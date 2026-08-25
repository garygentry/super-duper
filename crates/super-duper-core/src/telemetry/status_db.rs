use std::collections::HashSet;

use chrono::Utc;
use rusqlite::{params, Connection, Error as SqlError, OptionalExtension, TransactionBehavior};
use thiserror::Error;

use super::models::{
    CounterKind, DeviceDescriptor, DeviceSample, HostSample, MetricInvariantError,
    StatusCounterSummary, StatusPhaseSummary, StatusRetentionPolicy, StatusRetentionResult,
    StatusRunRecord, StatusRunStart, StatusRunTerminal, TelemetryFlush, TelemetryPhase,
    TelemetryPhaseState, TelemetryRunState, WriteDisposition, METRICS_CONTRACT_VERSION,
};

pub const CURRENT_STATUS_SCHEMA_VERSION: i64 = 2;
pub const MAX_STATUS_RUN_PAGE: usize = 100;
pub const MAX_STATUS_SAMPLE_PAGE: usize = 500;
pub const MAX_STATUS_DEVICES_PER_RUN: usize = 64;
const MAX_RETAINED_TERMINAL_RUNS: u32 = 10_000;
const MAX_RETAINED_SAMPLES_PER_RUN: u32 = 1_000_000;

#[derive(Debug, Error)]
pub enum StatusStoreError {
    #[error("status database error: {0}")]
    Database(#[from] SqlError),
    #[error("status payload serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error(transparent)]
    MetricInvariant(#[from] MetricInvariantError),
    #[error("invalid status input: {0}")]
    InvalidInput(String),
    #[error("status operation replay payload conflicts with the existing record")]
    OperationConflict,
    #[error("status run {0} was not found")]
    RunNotFound(i64),
    #[error("status run {run_id} is in state {state}, which does not accept telemetry writes")]
    RunStateConflict { run_id: i64, state: String },
    #[error("status flush sequence {sequence} is not newer than committed sequence {committed}")]
    SequenceConflict { sequence: u64, committed: u64 },
    #[error("status counter {metric} regressed from {committed} to {proposed}")]
    CounterRegression {
        metric: &'static str,
        committed: u64,
        proposed: u64,
    },
    #[error("status value {field}={value} exceeds SQLite's signed integer range")]
    NumericOverflow { field: &'static str, value: u64 },
}

/// Worker-owned connection to the separate local scan status database.
///
/// This database never owns immutable scan results, review state, preflight state, or operation
/// evidence. Higher-level worker code must keep it on the worker side of the process boundary.
pub struct StatusDatabase {
    conn: Connection,
}

impl StatusDatabase {
    pub fn open(path: &str) -> Result<Self, StatusStoreError> {
        let mut database = Self::open_connection(path)?;
        database.reconcile_interrupted_runs()?;
        database.apply_retention(StatusRetentionPolicy::default())?;
        Ok(database)
    }

    /// Opens the worker's one live status connection without reconciling active work. Startup owns
    /// [`open`](Self::open); tests and future explicitly coordinated secondary readers may use this
    /// entry point without marking the writer's current run interrupted.
    pub fn open_connection(path: &str) -> Result<Self, StatusStoreError> {
        let database = Self {
            conn: Connection::open(path)?,
        };
        database.configure_pragmas()?;
        database.migrate_schema()?;
        Ok(database)
    }

    pub fn open_in_memory() -> Result<Self, StatusStoreError> {
        let database = Self {
            conn: Connection::open_in_memory()?,
        };
        database.configure_pragmas()?;
        database.migrate_schema()?;
        Ok(database)
    }

    fn configure_pragmas(&self) -> Result<(), StatusStoreError> {
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             PRAGMA wal_autocheckpoint = 1000;",
        )?;
        Ok(())
    }

    pub fn schema_version(&self) -> Result<i64, StatusStoreError> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn begin_run(
        &mut self,
        input: &StatusRunStart,
    ) -> Result<(StatusRunRecord, WriteDisposition), StatusStoreError> {
        validate_run_start(input)?;
        let transaction = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = run_by_operation_id(&transaction, &input.operation_id)? {
            if !start_matches_record(input, &existing) {
                return Err(StatusStoreError::OperationConflict);
            }
            transaction.commit()?;
            return Ok((existing, WriteDisposition::Replayed));
        }
        transaction.execute(
            "INSERT INTO status_run
                (operation_id, product_run_id, metrics_contract_version, engine_version,
                 worker_version, app_version, product_schema_version, input_signature, state,
                 started_unix_millis, completed_unix_millis, last_monotonic_nanos, last_sequence,
                 error_code, error_message, created_unix_millis, updated_unix_millis)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'running', ?9, NULL, 0, 0,
                     NULL, NULL, ?9, ?9)",
            params![
                input.operation_id,
                input.product_run_id,
                i64::from(METRICS_CONTRACT_VERSION),
                input.engine_version,
                input.worker_version,
                input.app_version,
                input.product_schema_version,
                input.input_signature,
                input.started_unix_millis,
            ],
        )?;
        let id = transaction.last_insert_rowid();
        let record = run_by_id(&transaction, id)?.ok_or(StatusStoreError::RunNotFound(id))?;
        transaction.commit()?;
        Ok((record, WriteDisposition::Applied))
    }

    pub fn flush(
        &mut self,
        run_id: i64,
        flush: &TelemetryFlush,
    ) -> Result<WriteDisposition, StatusStoreError> {
        validate_flush(flush)?;
        let payload_json = serde_json::to_string(flush)?;
        if payload_json.len() > 1_048_576 {
            return Err(StatusStoreError::InvalidInput(
                "status flush payload exceeds 1 MiB".to_owned(),
            ));
        }

        let sequence = sqlite_u64("sequence", flush.sequence)?;
        let monotonic_nanos = sqlite_u64("monotonic_nanos", flush.monotonic_nanos)?;
        let phase_started = optional_sqlite_u64(
            "phase_started_monotonic_nanos",
            flush.phase_started_monotonic_nanos,
        )?;
        let phase_completed = optional_sqlite_u64(
            "phase_completed_monotonic_nanos",
            flush.phase_completed_monotonic_nanos,
        )?;
        let phase_active = sqlite_u64("phase_active_nanos", flush.phase_active_nanos)?;

        let transaction = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = run_by_id(&transaction, run_id)?.ok_or(StatusStoreError::RunNotFound(run_id))?;

        if let Some(existing_payload) = transaction
            .query_row(
                "SELECT payload_json FROM status_flush WHERE run_id = ?1 AND sequence = ?2",
                params![run_id, sequence],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            if existing_payload == payload_json {
                transaction.commit()?;
                return Ok(WriteDisposition::Replayed);
            }
            return Err(StatusStoreError::OperationConflict);
        }

        if !matches!(
            run.state,
            TelemetryRunState::Running | TelemetryRunState::Cancelling
        ) {
            return Err(StatusStoreError::RunStateConflict {
                run_id,
                state: run.state.as_str().to_owned(),
            });
        }
        if flush.sequence <= run.last_sequence {
            return Err(StatusStoreError::SequenceConflict {
                sequence: flush.sequence,
                committed: run.last_sequence,
            });
        }
        if flush.monotonic_nanos < run.last_monotonic_nanos {
            return Err(StatusStoreError::InvalidInput(format!(
                "monotonic timestamp {} precedes committed timestamp {}",
                flush.monotonic_nanos, run.last_monotonic_nanos
            )));
        }

        if let Some((committed_state, committed_start, committed_active)) = transaction
            .query_row(
                "SELECT state, started_monotonic_nanos, active_nanos
                 FROM status_phase WHERE run_id = ?1 AND phase = ?2",
                params![run_id, flush.phase.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
        {
            if !matches!(committed_state.as_str(), "pending" | "running") {
                return Err(StatusStoreError::InvalidInput(format!(
                    "phase {} is already terminal in state {committed_state}",
                    flush.phase.as_str()
                )));
            }
            if let (Some(committed), Some(proposed)) = (committed_start, phase_started) {
                if committed != proposed {
                    return Err(StatusStoreError::InvalidInput(format!(
                        "phase {} start timestamp changed from {committed} to {proposed}",
                        flush.phase.as_str()
                    )));
                }
            }
            let committed_active = sqlite_counter(committed_active)?;
            if flush.phase_active_nanos < committed_active {
                return Err(StatusStoreError::CounterRegression {
                    metric: "phase_active_nanos",
                    committed: committed_active,
                    proposed: flush.phase_active_nanos,
                });
            }
        }

        for kind in CounterKind::ALL {
            let proposed = flush.counters.value(kind);
            let committed = transaction
                .query_row(
                    "SELECT value FROM status_counter
                     WHERE run_id = ?1 AND phase = 'overall' AND metric = ?2",
                    params![run_id, kind.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
                .map(sqlite_counter)
                .transpose()?
                .unwrap_or(0);
            if proposed < committed {
                return Err(StatusStoreError::CounterRegression {
                    metric: kind.as_str(),
                    committed,
                    proposed,
                });
            }
            transaction.execute(
                "INSERT INTO status_counter(run_id, phase, metric, value, updated_sequence)
                 VALUES (?1, 'overall', ?2, ?3, ?4)
                 ON CONFLICT(run_id, phase, metric) DO UPDATE SET
                    value = excluded.value,
                    updated_sequence = excluded.updated_sequence",
                params![
                    run_id,
                    kind.as_str(),
                    sqlite_u64(kind.as_str(), proposed)?,
                    sequence,
                ],
            )?;
        }

        transaction.execute(
            "INSERT INTO status_phase
                (run_id, phase, state, started_monotonic_nanos, completed_monotonic_nanos,
                 active_nanos)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(run_id, phase) DO UPDATE SET
                state = excluded.state,
                started_monotonic_nanos = COALESCE(status_phase.started_monotonic_nanos,
                                                   excluded.started_monotonic_nanos),
                completed_monotonic_nanos = excluded.completed_monotonic_nanos,
                active_nanos = excluded.active_nanos",
            params![
                run_id,
                flush.phase.as_str(),
                flush.phase_state.as_str(),
                phase_started,
                phase_completed,
                phase_active,
            ],
        )?;

        if !flush.devices.is_empty() {
            let existing_devices: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM status_device WHERE run_id = ?1",
                [run_id],
                |row| row.get(0),
            )?;
            let mut new_devices = 0_i64;
            for device in &flush.devices {
                let exists: bool = transaction.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM status_device
                        WHERE run_id = ?1 AND device_key = ?2 AND volume_key = ?3
                     )",
                    params![run_id, device.device_key, device.volume_key],
                    |row| row.get(0),
                )?;
                if !exists {
                    new_devices += 1;
                }
            }
            if existing_devices + new_devices > MAX_STATUS_DEVICES_PER_RUN as i64 {
                return Err(StatusStoreError::InvalidInput(format!(
                    "status run cannot contain more than {MAX_STATUS_DEVICES_PER_RUN} devices"
                )));
            }
        }
        for device in &flush.devices {
            persist_device(&transaction, run_id, device)?;
        }
        if let Some(host) = &flush.host_sample {
            persist_host_sample(&transaction, run_id, host)?;
            for sample in &flush.device_samples {
                let known_device: bool = transaction.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM status_device WHERE run_id = ?1 AND device_key = ?2
                     )",
                    params![run_id, sample.device_key],
                    |row| row.get(0),
                )?;
                if !known_device {
                    return Err(StatusStoreError::InvalidInput(format!(
                        "device sample references unknown device key {}",
                        sample.device_key
                    )));
                }
                persist_device_sample(&transaction, run_id, sample)?;
            }
        }

        transaction.execute(
            "INSERT INTO status_flush(run_id, sequence, payload_json, applied_unix_millis)
             VALUES (?1, ?2, ?3, ?4)",
            params![run_id, sequence, payload_json, flush.observed_unix_millis],
        )?;
        transaction.execute(
            "UPDATE status_run
             SET last_monotonic_nanos = ?1, last_sequence = ?2, updated_unix_millis = ?3
             WHERE id = ?4",
            params![
                monotonic_nanos,
                sequence,
                flush.observed_unix_millis,
                run_id,
            ],
        )?;
        transaction.commit()?;
        Ok(WriteDisposition::Applied)
    }

    pub fn finish_run(
        &mut self,
        run_id: i64,
        terminal: &StatusRunTerminal,
    ) -> Result<WriteDisposition, StatusStoreError> {
        validate_terminal(terminal)?;
        let monotonic_nanos = sqlite_u64("monotonic_nanos", terminal.monotonic_nanos)?;
        let transaction = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = run_by_id(&transaction, run_id)?.ok_or(StatusStoreError::RunNotFound(run_id))?;
        if run.state.is_terminal() {
            if terminal_matches_record(terminal, &run) {
                transaction.commit()?;
                return Ok(WriteDisposition::Replayed);
            }
            return Err(StatusStoreError::OperationConflict);
        }
        if terminal.monotonic_nanos < run.last_monotonic_nanos {
            return Err(StatusStoreError::InvalidInput(format!(
                "terminal monotonic timestamp {} precedes committed timestamp {}",
                terminal.monotonic_nanos, run.last_monotonic_nanos
            )));
        }
        transaction.execute(
            "UPDATE status_phase
             SET state = CASE WHEN state = 'running' THEN ?1 ELSE state END,
                 completed_monotonic_nanos = CASE
                     WHEN state = 'running' THEN COALESCE(completed_monotonic_nanos, ?2)
                     ELSE completed_monotonic_nanos END
             WHERE run_id = ?3",
            params![
                phase_terminal_state(terminal.state),
                monotonic_nanos,
                run_id
            ],
        )?;
        transaction.execute(
            "UPDATE status_run
             SET state = ?1, completed_unix_millis = ?2, last_monotonic_nanos = ?3,
                 error_code = ?4, error_message = ?5, updated_unix_millis = ?2
             WHERE id = ?6",
            params![
                terminal.state.as_str(),
                terminal.completed_unix_millis,
                monotonic_nanos,
                terminal.error_code,
                terminal.error_message,
                run_id,
            ],
        )?;
        transaction.commit()?;
        Ok(WriteDisposition::Applied)
    }

    pub fn get_run(&self, run_id: i64) -> Result<StatusRunRecord, StatusStoreError> {
        run_by_id(&self.conn, run_id)?.ok_or(StatusStoreError::RunNotFound(run_id))
    }

    /// List newest status runs using a strict, stable `id < before_id` cursor.
    pub fn list_runs(
        &self,
        before_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<StatusRunRecord>, StatusStoreError> {
        validate_page_limit("run page", limit, MAX_STATUS_RUN_PAGE)?;
        let before_id = before_id.unwrap_or(i64::MAX);
        if before_id <= 0 {
            return Err(StatusStoreError::InvalidInput(
                "run cursor must be a positive row ID".to_owned(),
            ));
        }
        let mut statement = self.conn.prepare(
            "SELECT id, operation_id, product_run_id, metrics_contract_version, engine_version,
                    worker_version, app_version, product_schema_version, input_signature, state,
                    started_unix_millis, completed_unix_millis, last_monotonic_nanos,
                    last_sequence, error_code, error_message
             FROM status_run WHERE id < ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![before_id, limit as i64], status_run_from_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get_run_counters(
        &self,
        run_id: i64,
    ) -> Result<Vec<StatusCounterSummary>, StatusStoreError> {
        self.ensure_run_exists(run_id)?;
        let mut statement = self.conn.prepare(
            "SELECT metric, value, updated_sequence FROM status_counter
             WHERE run_id = ?1 AND phase = 'overall' ORDER BY metric",
        )?;
        let rows = statement.query_map([run_id], |row| {
            Ok(StatusCounterSummary {
                metric: row.get(0)?,
                value: sql_u64_from_row(row, 1)?,
                updated_sequence: sql_u64_from_row(row, 2)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get_run_phases(&self, run_id: i64) -> Result<Vec<StatusPhaseSummary>, StatusStoreError> {
        self.ensure_run_exists(run_id)?;
        let mut statement = self.conn.prepare(
            "SELECT phase, state, started_monotonic_nanos, completed_monotonic_nanos,
                    active_nanos
             FROM status_phase WHERE run_id = ?1 ORDER BY started_monotonic_nanos, phase",
        )?;
        let rows = statement.query_map([run_id], |row| {
            Ok(StatusPhaseSummary {
                phase: parse_phase(&row.get::<_, String>(0)?, 0)?,
                state: parse_phase_state(&row.get::<_, String>(1)?, 1)?,
                started_monotonic_nanos: optional_sql_u64_from_row(row, 2)?,
                completed_monotonic_nanos: optional_sql_u64_from_row(row, 3)?,
                active_nanos: sql_u64_from_row(row, 4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get_run_devices(&self, run_id: i64) -> Result<Vec<DeviceDescriptor>, StatusStoreError> {
        self.ensure_run_exists(run_id)?;
        let mut statement = self.conn.prepare(
            "SELECT device_key, volume_key, filesystem, capacity_bytes, free_bytes_at_start,
                    bus_type, media_type, model
             FROM status_device WHERE run_id = ?1 ORDER BY device_key, volume_key",
        )?;
        let rows = statement.query_map([run_id], |row| {
            Ok(DeviceDescriptor {
                device_key: row.get(0)?,
                volume_key: row.get(1)?,
                filesystem: row.get(2)?,
                capacity_bytes: optional_sql_u64_from_row(row, 3)?,
                free_bytes_at_start: optional_sql_u64_from_row(row, 4)?,
                bus_type: row.get(5)?,
                media_type: row.get(6)?,
                model: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn list_host_samples(
        &self,
        run_id: i64,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<HostSample>, StatusStoreError> {
        validate_page_limit("host sample page", limit, MAX_STATUS_SAMPLE_PAGE)?;
        self.ensure_run_exists(run_id)?;
        let mut statement = self.conn.prepare(
            "SELECT sequence, observed_unix_millis, monotonic_nanos, phase,
                    process_cpu_nanos, process_private_bytes, process_working_set_bytes,
                    process_peak_working_set_bytes, process_read_operations, process_read_bytes,
                    process_write_operations, process_write_bytes, system_cpu_basis_points,
                    system_available_memory_bytes, system_committed_memory_bytes,
                    unavailable_counter_count
             FROM status_host_sample
             WHERE run_id = ?1 AND sequence > ?2 ORDER BY sequence LIMIT ?3",
        )?;
        let rows = statement.query_map(
            params![
                run_id,
                sqlite_u64("after_sequence", after_sequence)?,
                limit as i64
            ],
            host_sample_from_row,
        )?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn list_device_samples(
        &self,
        run_id: i64,
        device_key: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<DeviceSample>, StatusStoreError> {
        validate_page_limit("device sample page", limit, MAX_STATUS_SAMPLE_PAGE)?;
        bounded_nonblank("device_key", device_key, 256)?;
        self.ensure_run_exists(run_id)?;
        let mut statement = self.conn.prepare(
            "SELECT sequence, device_key, read_bytes_per_second, read_iops_millis,
                    average_read_latency_micros, active_millis_per_second, queue_depth_millis,
                    unavailable_counter_count
             FROM status_device_sample
             WHERE run_id = ?1 AND device_key = ?2 AND sequence > ?3
             ORDER BY sequence LIMIT ?4",
        )?;
        let rows = statement.query_map(
            params![
                run_id,
                device_key,
                sqlite_u64("after_sequence", after_sequence)?,
                limit as i64
            ],
            device_sample_from_row,
        )?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Apply fixed-count retention and a non-blocking WAL checkpoint. Active runs are never
    /// deleted. Terminal replay payloads are discarded because terminal runs reject new flushes.
    pub fn apply_retention(
        &mut self,
        policy: StatusRetentionPolicy,
    ) -> Result<StatusRetentionResult, StatusStoreError> {
        validate_retention_policy(policy)?;
        let transaction = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (host_samples_before, device_samples_before): (i64, i64) = transaction.query_row(
            "SELECT
                (SELECT COUNT(*) FROM status_host_sample),
                (SELECT COUNT(*) FROM status_device_sample)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let terminal_runs_deleted = transaction.execute(
            "DELETE FROM status_run
             WHERE state IN ('completed', 'cancelled', 'failed', 'interrupted')
               AND id NOT IN (
                   SELECT id FROM status_run
                   WHERE state IN ('completed', 'cancelled', 'failed', 'interrupted')
                   ORDER BY id DESC LIMIT ?1
               )",
            [i64::from(policy.max_terminal_runs)],
        )?;
        let sample_offset = i64::from(policy.max_samples_per_run - 1);
        transaction.execute(
            "DELETE FROM status_host_sample
             WHERE sequence < (
                 SELECT cutoff.sequence FROM status_host_sample AS cutoff
                 WHERE cutoff.run_id = status_host_sample.run_id
                 ORDER BY cutoff.sequence DESC LIMIT 1 OFFSET ?1
             )",
            [sample_offset],
        )?;
        let replay_flushes_deleted = transaction.execute(
            "DELETE FROM status_flush
             WHERE run_id IN (
                 SELECT id FROM status_run
                 WHERE state IN ('completed', 'cancelled', 'failed', 'interrupted')
             )",
            [],
        )?;
        let (host_samples_after, device_samples_after): (i64, i64) = transaction.query_row(
            "SELECT
                (SELECT COUNT(*) FROM status_host_sample),
                (SELECT COUNT(*) FROM status_device_sample)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        transaction.commit()?;

        let (wal_busy, wal_frames, wal_frames_checkpointed) = self.checkpoint_wal()?;
        Ok(StatusRetentionResult {
            terminal_runs_deleted: terminal_runs_deleted as u64,
            host_samples_deleted: host_samples_before.saturating_sub(host_samples_after) as u64,
            device_samples_deleted: device_samples_before.saturating_sub(device_samples_after)
                as u64,
            replay_flushes_deleted: replay_flushes_deleted as u64,
            wal_busy,
            wal_frames,
            wal_frames_checkpointed,
        })
    }

    /// Delete all terminal status history without touching active status runs or product state.
    pub fn delete_terminal_history(&mut self) -> Result<u64, StatusStoreError> {
        let deleted = self.conn.execute(
            "DELETE FROM status_run
             WHERE state IN ('completed', 'cancelled', 'failed', 'interrupted')",
            [],
        )?;
        let _ = self.checkpoint_wal()?;
        Ok(deleted as u64)
    }

    fn ensure_run_exists(&self, run_id: i64) -> Result<(), StatusStoreError> {
        if run_by_id(&self.conn, run_id)?.is_none() {
            return Err(StatusStoreError::RunNotFound(run_id));
        }
        Ok(())
    }

    fn checkpoint_wal(&self) -> Result<(bool, Option<u64>, Option<u64>), StatusStoreError> {
        let (busy, frames, checkpointed): (i64, i64, i64) =
            self.conn
                .query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?;
        Ok((
            busy != 0,
            u64::try_from(frames).ok(),
            u64::try_from(checkpointed).ok(),
        ))
    }

    fn reconcile_interrupted_runs(&mut self) -> Result<(), StatusStoreError> {
        let now = Utc::now().timestamp_millis();
        let transaction = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "UPDATE status_phase
             SET state = 'interrupted',
                 completed_monotonic_nanos = COALESCE(
                    completed_monotonic_nanos,
                    (SELECT last_monotonic_nanos FROM status_run WHERE id = status_phase.run_id))
             WHERE state = 'running'
               AND run_id IN (SELECT id FROM status_run WHERE state IN ('running', 'cancelling'))",
            [],
        )?;
        transaction.execute(
            "UPDATE status_run
             SET state = 'interrupted', completed_unix_millis = ?1,
                 error_code = COALESCE(error_code, 'worker_restarted'),
                 error_message = COALESCE(error_message,
                     'The worker restarted before telemetry reached a terminal state.'),
                 updated_unix_millis = ?1
             WHERE state IN ('running', 'cancelling')",
            [now],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn has_user_tables(&self) -> Result<bool, StatusStoreError> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             )",
            [],
            |row| row.get(0),
        )?)
    }

    fn migrate_schema(&self) -> Result<(), StatusStoreError> {
        let version = self.schema_version()?;
        match version {
            CURRENT_STATUS_SCHEMA_VERSION => {
                self.conn.execute_batch(include_str!("status_schema.sql"))?;
            }
            1 => self.migrate_v1_to_v2()?,
            0 if !self.has_user_tables()? => {
                self.conn.execute_batch(include_str!("status_schema.sql"))?;
            }
            0 => {
                return Err(StatusStoreError::InvalidInput(
                    "unversioned non-empty status database was not modified".to_owned(),
                ));
            }
            newer if newer > CURRENT_STATUS_SCHEMA_VERSION => {
                return Err(StatusStoreError::InvalidInput(format!(
                    "status database schema version {newer} is newer than supported version {CURRENT_STATUS_SCHEMA_VERSION}"
                )));
            }
            older => {
                return Err(StatusStoreError::InvalidInput(format!(
                    "unsupported status database schema version {older}; database was not modified"
                )));
            }
        }
        self.conn.execute_batch(include_str!("status_schema.sql"))?;
        Ok(())
    }

    fn migrate_v1_to_v2(&self) -> Result<(), StatusStoreError> {
        let transaction = self.conn.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE status_flush (
                run_id INTEGER NOT NULL REFERENCES status_run(id) ON DELETE CASCADE,
                sequence INTEGER NOT NULL CHECK(sequence >= 0),
                payload_json TEXT NOT NULL CHECK(length(payload_json) BETWEEN 2 AND 1048576),
                applied_unix_millis INTEGER NOT NULL,
                PRIMARY KEY(run_id, sequence)
             );
             PRAGMA user_version = 2;",
        )?;
        transaction.commit()?;
        Ok(())
    }
}

fn validate_run_start(input: &StatusRunStart) -> Result<(), StatusStoreError> {
    bounded_nonblank("operation_id", &input.operation_id, 128)?;
    bounded_nonblank("engine_version", &input.engine_version, 128)?;
    bounded_nonblank("input_signature", &input.input_signature, 256)?;
    bounded_optional("worker_version", input.worker_version.as_deref(), 128)?;
    bounded_optional("app_version", input.app_version.as_deref(), 128)?;
    if input.product_run_id.is_some_and(|id| id <= 0) {
        return Err(StatusStoreError::InvalidInput(
            "product_run_id must be positive when present".to_owned(),
        ));
    }
    if input
        .product_schema_version
        .is_some_and(|version| version < 0)
    {
        return Err(StatusStoreError::InvalidInput(
            "product_schema_version cannot be negative".to_owned(),
        ));
    }
    Ok(())
}

fn validate_flush(flush: &TelemetryFlush) -> Result<(), StatusStoreError> {
    flush.counters.validate()?;
    if flush.phase == TelemetryPhase::Overall {
        return Err(StatusStoreError::InvalidInput(
            "overall is a counter scope, not an executable phase".to_owned(),
        ));
    }
    if flush.phase_state != super::models::TelemetryPhaseState::Pending
        && flush.phase_started_monotonic_nanos.is_none()
    {
        return Err(StatusStoreError::InvalidInput(
            "an active or terminal phase requires its start timestamp".to_owned(),
        ));
    }
    if flush
        .phase_completed_monotonic_nanos
        .is_some_and(|completed| {
            flush
                .phase_started_monotonic_nanos
                .map_or(true, |started| completed < started)
                || completed > flush.monotonic_nanos
        })
    {
        return Err(StatusStoreError::InvalidInput(
            "phase completion requires a start and cannot follow the flush timestamp".to_owned(),
        ));
    }
    if let Some(host) = &flush.host_sample {
        if host.sequence != flush.sequence
            || host.observed_unix_millis != flush.observed_unix_millis
            || host.monotonic_nanos != flush.monotonic_nanos
            || host.phase != Some(flush.phase)
        {
            return Err(StatusStoreError::InvalidInput(
                "host sample identity must match its flush envelope".to_owned(),
            ));
        }
        if host
            .system_cpu_basis_points
            .is_some_and(|value| value > 10_000)
        {
            return Err(StatusStoreError::InvalidInput(
                "system_cpu_basis_points cannot exceed 10000".to_owned(),
            ));
        }
    } else if !flush.device_samples.is_empty() {
        return Err(StatusStoreError::InvalidInput(
            "device samples require a matching host sample".to_owned(),
        ));
    }

    if flush.devices.len() > MAX_STATUS_DEVICES_PER_RUN
        || flush.device_samples.len() > MAX_STATUS_DEVICES_PER_RUN
    {
        return Err(StatusStoreError::InvalidInput(format!(
            "a status flush cannot contain more than {MAX_STATUS_DEVICES_PER_RUN} devices"
        )));
    }
    let mut descriptor_keys = HashSet::new();
    for device in &flush.devices {
        validate_device(device)?;
        if !descriptor_keys.insert((device.device_key.as_str(), device.volume_key.as_str())) {
            return Err(StatusStoreError::InvalidInput(
                "duplicate device descriptor in one flush".to_owned(),
            ));
        }
    }
    let mut sample_keys = HashSet::new();
    for sample in &flush.device_samples {
        validate_device_sample(sample, flush.sequence)?;
        if !sample_keys.insert(sample.device_key.as_str()) {
            return Err(StatusStoreError::InvalidInput(
                "duplicate device sample in one flush".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_terminal(terminal: &StatusRunTerminal) -> Result<(), StatusStoreError> {
    if !terminal.state.is_terminal() {
        return Err(StatusStoreError::InvalidInput(
            "terminal update requires a terminal run state".to_owned(),
        ));
    }
    bounded_optional("error_code", terminal.error_code.as_deref(), 128)?;
    bounded_optional("error_message", terminal.error_message.as_deref(), 4096)
}

fn validate_device(device: &DeviceDescriptor) -> Result<(), StatusStoreError> {
    bounded_nonblank("device_key", &device.device_key, 256)?;
    bounded_nonblank("volume_key", &device.volume_key, 256)?;
    bounded_optional("filesystem", device.filesystem.as_deref(), 64)?;
    bounded_optional("bus_type", device.bus_type.as_deref(), 64)?;
    bounded_optional("media_type", device.media_type.as_deref(), 64)?;
    bounded_optional("model", device.model.as_deref(), 256)?;
    optional_sqlite_u64("capacity_bytes", device.capacity_bytes)?;
    optional_sqlite_u64("free_bytes_at_start", device.free_bytes_at_start)?;
    Ok(())
}

fn validate_device_sample(sample: &DeviceSample, sequence: u64) -> Result<(), StatusStoreError> {
    if sample.sequence != sequence {
        return Err(StatusStoreError::InvalidInput(
            "device sample sequence must match its flush envelope".to_owned(),
        ));
    }
    bounded_nonblank("device_key", &sample.device_key, 256)?;
    if sample
        .active_millis_per_second
        .is_some_and(|value| value > 1000)
    {
        return Err(StatusStoreError::InvalidInput(
            "active_millis_per_second cannot exceed 1000".to_owned(),
        ));
    }
    Ok(())
}

fn bounded_nonblank(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), StatusStoreError> {
    if value.trim().is_empty() || value.chars().count() > maximum {
        return Err(StatusStoreError::InvalidInput(format!(
            "{field} must contain 1 to {maximum} characters"
        )));
    }
    Ok(())
}

fn bounded_optional(
    field: &'static str,
    value: Option<&str>,
    maximum: usize,
) -> Result<(), StatusStoreError> {
    if let Some(value) = value {
        bounded_nonblank(field, value, maximum)?;
    }
    Ok(())
}

fn persist_device(
    transaction: &rusqlite::Transaction<'_>,
    run_id: i64,
    device: &DeviceDescriptor,
) -> Result<(), StatusStoreError> {
    transaction.execute(
        "INSERT INTO status_device
            (run_id, device_key, volume_key, filesystem, capacity_bytes, free_bytes_at_start,
             bus_type, media_type, model)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(run_id, device_key, volume_key) DO UPDATE SET
            filesystem = excluded.filesystem,
            capacity_bytes = excluded.capacity_bytes,
            free_bytes_at_start = excluded.free_bytes_at_start,
            bus_type = excluded.bus_type,
            media_type = excluded.media_type,
            model = excluded.model",
        params![
            run_id,
            device.device_key,
            device.volume_key,
            device.filesystem,
            optional_sqlite_u64("capacity_bytes", device.capacity_bytes)?,
            optional_sqlite_u64("free_bytes_at_start", device.free_bytes_at_start)?,
            device.bus_type,
            device.media_type,
            device.model,
        ],
    )?;
    Ok(())
}

fn persist_host_sample(
    transaction: &rusqlite::Transaction<'_>,
    run_id: i64,
    sample: &HostSample,
) -> Result<(), StatusStoreError> {
    transaction.execute(
        "INSERT INTO status_host_sample
            (run_id, sequence, observed_unix_millis, monotonic_nanos, phase,
             process_cpu_nanos, process_private_bytes, process_working_set_bytes,
             process_peak_working_set_bytes, process_read_operations, process_read_bytes,
             process_write_operations, process_write_bytes, system_cpu_basis_points,
             system_available_memory_bytes, system_committed_memory_bytes,
             unavailable_counter_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            run_id,
            sqlite_u64("sequence", sample.sequence)?,
            sample.observed_unix_millis,
            sqlite_u64("monotonic_nanos", sample.monotonic_nanos)?,
            sample.phase.map(TelemetryPhase::as_str),
            optional_sqlite_u64("process_cpu_nanos", sample.process_cpu_nanos)?,
            optional_sqlite_u64("process_private_bytes", sample.process_private_bytes)?,
            optional_sqlite_u64(
                "process_working_set_bytes",
                sample.process_working_set_bytes
            )?,
            optional_sqlite_u64(
                "process_peak_working_set_bytes",
                sample.process_peak_working_set_bytes,
            )?,
            optional_sqlite_u64("process_read_operations", sample.process_read_operations)?,
            optional_sqlite_u64("process_read_bytes", sample.process_read_bytes)?,
            optional_sqlite_u64("process_write_operations", sample.process_write_operations)?,
            optional_sqlite_u64("process_write_bytes", sample.process_write_bytes)?,
            sample.system_cpu_basis_points.map(i64::from),
            optional_sqlite_u64(
                "system_available_memory_bytes",
                sample.system_available_memory_bytes,
            )?,
            optional_sqlite_u64(
                "system_committed_memory_bytes",
                sample.system_committed_memory_bytes,
            )?,
            i64::from(sample.unavailable_counter_count),
        ],
    )?;
    Ok(())
}

fn persist_device_sample(
    transaction: &rusqlite::Transaction<'_>,
    run_id: i64,
    sample: &DeviceSample,
) -> Result<(), StatusStoreError> {
    transaction.execute(
        "INSERT INTO status_device_sample
            (run_id, sequence, device_key, read_bytes_per_second, read_iops_millis,
             average_read_latency_micros, active_millis_per_second, queue_depth_millis,
             unavailable_counter_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            run_id,
            sqlite_u64("sequence", sample.sequence)?,
            sample.device_key,
            optional_sqlite_u64("read_bytes_per_second", sample.read_bytes_per_second)?,
            optional_sqlite_u64("read_iops_millis", sample.read_iops_millis)?,
            optional_sqlite_u64(
                "average_read_latency_micros",
                sample.average_read_latency_micros,
            )?,
            sample.active_millis_per_second.map(i64::from),
            optional_sqlite_u64("queue_depth_millis", sample.queue_depth_millis)?,
            i64::from(sample.unavailable_counter_count),
        ],
    )?;
    Ok(())
}

fn validate_page_limit(label: &str, limit: usize, maximum: usize) -> Result<(), StatusStoreError> {
    if limit == 0 || limit > maximum {
        return Err(StatusStoreError::InvalidInput(format!(
            "{label} limit must be between 1 and {maximum}"
        )));
    }
    Ok(())
}

fn validate_retention_policy(policy: StatusRetentionPolicy) -> Result<(), StatusStoreError> {
    if policy.max_terminal_runs == 0 || policy.max_terminal_runs > MAX_RETAINED_TERMINAL_RUNS {
        return Err(StatusStoreError::InvalidInput(format!(
            "max_terminal_runs must be between 1 and {MAX_RETAINED_TERMINAL_RUNS}"
        )));
    }
    if policy.max_samples_per_run == 0 || policy.max_samples_per_run > MAX_RETAINED_SAMPLES_PER_RUN
    {
        return Err(StatusStoreError::InvalidInput(format!(
            "max_samples_per_run must be between 1 and {MAX_RETAINED_SAMPLES_PER_RUN}"
        )));
    }
    Ok(())
}

fn sql_u64_from_row(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value = row.get::<_, i64>(index)?;
    u64::try_from(value).map_err(|error| {
        SqlError::FromSqlConversionFailure(index, rusqlite::types::Type::Integer, Box::new(error))
    })
}

fn optional_sql_u64_from_row(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| {
            u64::try_from(value).map_err(|error| {
                SqlError::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })
        })
        .transpose()
}

fn optional_sql_u32_from_row(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<u32>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| {
            u32::try_from(value).map_err(|error| {
                SqlError::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })
        })
        .transpose()
}

fn sql_u32_from_row(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u32> {
    let value = row.get::<_, i64>(index)?;
    u32::try_from(value).map_err(|error| {
        SqlError::FromSqlConversionFailure(index, rusqlite::types::Type::Integer, Box::new(error))
    })
}

fn host_sample_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HostSample> {
    Ok(HostSample {
        sequence: sql_u64_from_row(row, 0)?,
        observed_unix_millis: row.get(1)?,
        monotonic_nanos: sql_u64_from_row(row, 2)?,
        phase: row
            .get::<_, Option<String>>(3)?
            .map(|value| parse_phase(&value, 3))
            .transpose()?,
        process_cpu_nanos: optional_sql_u64_from_row(row, 4)?,
        process_private_bytes: optional_sql_u64_from_row(row, 5)?,
        process_working_set_bytes: optional_sql_u64_from_row(row, 6)?,
        process_peak_working_set_bytes: optional_sql_u64_from_row(row, 7)?,
        process_read_operations: optional_sql_u64_from_row(row, 8)?,
        process_read_bytes: optional_sql_u64_from_row(row, 9)?,
        process_write_operations: optional_sql_u64_from_row(row, 10)?,
        process_write_bytes: optional_sql_u64_from_row(row, 11)?,
        system_cpu_basis_points: optional_sql_u32_from_row(row, 12)?,
        system_available_memory_bytes: optional_sql_u64_from_row(row, 13)?,
        system_committed_memory_bytes: optional_sql_u64_from_row(row, 14)?,
        unavailable_counter_count: sql_u32_from_row(row, 15)?,
    })
}

fn device_sample_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DeviceSample> {
    Ok(DeviceSample {
        sequence: sql_u64_from_row(row, 0)?,
        device_key: row.get(1)?,
        read_bytes_per_second: optional_sql_u64_from_row(row, 2)?,
        read_iops_millis: optional_sql_u64_from_row(row, 3)?,
        average_read_latency_micros: optional_sql_u64_from_row(row, 4)?,
        active_millis_per_second: optional_sql_u32_from_row(row, 5)?,
        queue_depth_millis: optional_sql_u64_from_row(row, 6)?,
        unavailable_counter_count: sql_u32_from_row(row, 7)?,
    })
}

fn parse_phase(value: &str, index: usize) -> rusqlite::Result<TelemetryPhase> {
    match value {
        "overall" => Ok(TelemetryPhase::Overall),
        "discovering" => Ok(TelemetryPhase::Discovering),
        "candidate_screening" => Ok(TelemetryPhase::CandidateScreening),
        "full_hashing" => Ok(TelemetryPhase::FullHashing),
        "persisting" => Ok(TelemetryPhase::Persisting),
        "analyzing_folders" => Ok(TelemetryPhase::AnalyzingFolders),
        "finalizing" => Ok(TelemetryPhase::Finalizing),
        _ => Err(SqlError::InvalidColumnType(
            index,
            "phase".to_owned(),
            rusqlite::types::Type::Text,
        )),
    }
}

fn parse_phase_state(value: &str, index: usize) -> rusqlite::Result<TelemetryPhaseState> {
    match value {
        "pending" => Ok(TelemetryPhaseState::Pending),
        "running" => Ok(TelemetryPhaseState::Running),
        "completed" => Ok(TelemetryPhaseState::Completed),
        "cancelled" => Ok(TelemetryPhaseState::Cancelled),
        "failed" => Ok(TelemetryPhaseState::Failed),
        "interrupted" => Ok(TelemetryPhaseState::Interrupted),
        _ => Err(SqlError::InvalidColumnType(
            index,
            "state".to_owned(),
            rusqlite::types::Type::Text,
        )),
    }
}

fn run_by_operation_id(
    connection: &Connection,
    operation_id: &str,
) -> Result<Option<StatusRunRecord>, StatusStoreError> {
    Ok(connection
        .query_row(
            "SELECT id, operation_id, product_run_id, metrics_contract_version, engine_version,
                    worker_version, app_version, product_schema_version, input_signature, state,
                    started_unix_millis, completed_unix_millis, last_monotonic_nanos,
                    last_sequence, error_code, error_message
             FROM status_run WHERE operation_id = ?1",
            [operation_id],
            status_run_from_row,
        )
        .optional()?)
}

fn run_by_id(
    connection: &Connection,
    run_id: i64,
) -> Result<Option<StatusRunRecord>, StatusStoreError> {
    Ok(connection
        .query_row(
            "SELECT id, operation_id, product_run_id, metrics_contract_version, engine_version,
                    worker_version, app_version, product_schema_version, input_signature, state,
                    started_unix_millis, completed_unix_millis, last_monotonic_nanos,
                    last_sequence, error_code, error_message
             FROM status_run WHERE id = ?1",
            [run_id],
            status_run_from_row,
        )
        .optional()?)
}

fn status_run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StatusRunRecord> {
    let metrics_version = row.get::<_, i64>(3)?;
    let last_monotonic_nanos = row.get::<_, i64>(12)?;
    let last_sequence = row.get::<_, i64>(13)?;
    Ok(StatusRunRecord {
        id: row.get(0)?,
        operation_id: row.get(1)?,
        product_run_id: row.get(2)?,
        metrics_contract_version: u32::try_from(metrics_version).map_err(|error| {
            SqlError::FromSqlConversionFailure(3, rusqlite::types::Type::Integer, Box::new(error))
        })?,
        engine_version: row.get(4)?,
        worker_version: row.get(5)?,
        app_version: row.get(6)?,
        product_schema_version: row.get(7)?,
        input_signature: row.get(8)?,
        state: parse_run_state(&row.get::<_, String>(9)?)?,
        started_unix_millis: row.get(10)?,
        completed_unix_millis: row.get(11)?,
        last_monotonic_nanos: u64::try_from(last_monotonic_nanos).map_err(|error| {
            SqlError::FromSqlConversionFailure(12, rusqlite::types::Type::Integer, Box::new(error))
        })?,
        last_sequence: u64::try_from(last_sequence).map_err(|error| {
            SqlError::FromSqlConversionFailure(13, rusqlite::types::Type::Integer, Box::new(error))
        })?,
        error_code: row.get(14)?,
        error_message: row.get(15)?,
    })
}

fn parse_run_state(value: &str) -> rusqlite::Result<TelemetryRunState> {
    match value {
        "pending" => Ok(TelemetryRunState::Pending),
        "running" => Ok(TelemetryRunState::Running),
        "cancelling" => Ok(TelemetryRunState::Cancelling),
        "completed" => Ok(TelemetryRunState::Completed),
        "cancelled" => Ok(TelemetryRunState::Cancelled),
        "failed" => Ok(TelemetryRunState::Failed),
        "interrupted" => Ok(TelemetryRunState::Interrupted),
        _ => Err(SqlError::InvalidColumnType(
            9,
            "state".to_owned(),
            rusqlite::types::Type::Text,
        )),
    }
}

fn start_matches_record(input: &StatusRunStart, record: &StatusRunRecord) -> bool {
    record.product_run_id == input.product_run_id
        && record.metrics_contract_version == METRICS_CONTRACT_VERSION
        && record.engine_version == input.engine_version
        && record.worker_version == input.worker_version
        && record.app_version == input.app_version
        && record.product_schema_version == input.product_schema_version
        && record.input_signature == input.input_signature
        && record.started_unix_millis == Some(input.started_unix_millis)
}

fn terminal_matches_record(terminal: &StatusRunTerminal, record: &StatusRunRecord) -> bool {
    record.state == terminal.state
        && record.completed_unix_millis == Some(terminal.completed_unix_millis)
        && record.last_monotonic_nanos == terminal.monotonic_nanos
        && record.error_code == terminal.error_code
        && record.error_message == terminal.error_message
}

fn parse_counter(value: i64) -> Result<u64, StatusStoreError> {
    u64::try_from(value).map_err(|_| {
        StatusStoreError::InvalidInput("stored counter contains a negative value".to_owned())
    })
}

fn sqlite_counter(value: i64) -> Result<u64, StatusStoreError> {
    parse_counter(value)
}

fn sqlite_u64(field: &'static str, value: u64) -> Result<i64, StatusStoreError> {
    i64::try_from(value).map_err(|_| StatusStoreError::NumericOverflow { field, value })
}

fn optional_sqlite_u64(
    field: &'static str,
    value: Option<u64>,
) -> Result<Option<i64>, StatusStoreError> {
    value.map(|value| sqlite_u64(field, value)).transpose()
}

fn phase_terminal_state(state: TelemetryRunState) -> &'static str {
    match state {
        TelemetryRunState::Completed => "completed",
        TelemetryRunState::Cancelled => "cancelled",
        TelemetryRunState::Failed => "failed",
        TelemetryRunState::Interrupted => "interrupted",
        _ => "failed",
    }
}
