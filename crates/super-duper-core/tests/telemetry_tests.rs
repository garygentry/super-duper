use rusqlite::Connection;
use super_duper_core::telemetry::{
    CounterKind, DeviceDescriptor, DeviceSample, HostSample, ScanCounters, StatusDatabase,
    StatusRetentionPolicy, StatusRunStart, StatusRunTerminal, TelemetryFlush, TelemetryPhase,
    TelemetryPhaseState, TelemetryRunState, WriteDisposition, CURRENT_STATUS_SCHEMA_VERSION,
    MAX_STATUS_RUN_PAGE, MAX_STATUS_SAMPLE_PAGE, METRICS_CONTRACT_VERSION,
};
use tempfile::tempdir;

fn run_start(operation_id: &str) -> StatusRunStart {
    StatusRunStart {
        operation_id: operation_id.to_owned(),
        product_run_id: Some(42),
        engine_version: "engine-test".to_owned(),
        worker_version: Some("worker-test".to_owned()),
        app_version: Some("app-test".to_owned()),
        product_schema_version: Some(14),
        input_signature: "fixture-signature".to_owned(),
        started_unix_millis: 1_700_000_000_000,
    }
}

fn flush(sequence: u64, discovered_files: u64) -> TelemetryFlush {
    let observed_unix_millis = 1_700_000_000_000 + i64::try_from(sequence).unwrap() * 5_000;
    let monotonic_nanos = sequence * 5_000_000_000;
    TelemetryFlush {
        sequence,
        observed_unix_millis,
        monotonic_nanos,
        phase: TelemetryPhase::Discovering,
        phase_state: TelemetryPhaseState::Running,
        phase_started_monotonic_nanos: Some(0),
        phase_completed_monotonic_nanos: None,
        phase_active_nanos: monotonic_nanos,
        counters: ScanCounters {
            discovered_files,
            discovered_bytes: discovered_files * 100,
            size_buckets: discovered_files,
            singleton_size_buckets: discovered_files,
            singleton_size_files: discovered_files,
            singleton_size_bytes: discovered_files * 100,
            unavailable_counters: 3,
            ..Default::default()
        },
        host_sample: Some(HostSample {
            sequence,
            observed_unix_millis,
            monotonic_nanos,
            phase: Some(TelemetryPhase::Discovering),
            process_cpu_nanos: Some(sequence * 10_000_000),
            process_private_bytes: None,
            unavailable_counter_count: 3,
            ..Default::default()
        }),
        devices: vec![DeviceDescriptor {
            device_key: "physical:0".to_owned(),
            volume_key: "volume:d".to_owned(),
            filesystem: Some("NTFS".to_owned()),
            capacity_bytes: Some(10_000),
            free_bytes_at_start: None,
            bus_type: Some("SATA".to_owned()),
            media_type: Some("rotational".to_owned()),
            model: Some("fixture disk".to_owned()),
        }],
        device_samples: vec![DeviceSample {
            sequence,
            device_key: "physical:0".to_owned(),
            read_bytes_per_second: Some(25_000_000),
            read_iops_millis: None,
            average_read_latency_micros: Some(30_000),
            active_millis_per_second: Some(990),
            queue_depth_millis: Some(8_000),
            unavailable_counter_count: 1,
        }],
    }
}

#[test]
fn metric_contract_has_stable_unique_identifiers() {
    assert_eq!(METRICS_CONTRACT_VERSION, 2);
    let mut identifiers = CounterKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>();
    identifiers.sort_unstable();
    identifiers.dedup();
    assert_eq!(identifiers.len(), CounterKind::ALL.len());
    assert_eq!(
        TelemetryPhase::CandidateScreening.as_str(),
        "candidate_screening"
    );
    assert_eq!(TelemetryRunState::Interrupted.as_str(), "interrupted");
}

#[test]
fn counter_invariants_accept_partial_progress_and_reject_semantic_mixing() {
    let valid = ScanCounters {
        discovered_files: 12,
        discovered_bytes: 1_200,
        zero_byte_files: 1,
        hard_link_alias_files: 1,
        hard_link_alias_bytes: 100,
        size_buckets: 4,
        singleton_size_buckets: 2,
        singleton_size_files: 2,
        singleton_size_bytes: 200,
        candidate_size_buckets: 2,
        candidate_files: 8,
        candidate_bytes: 800,
        duplicate_candidate_size_buckets: 2,
        duplicate_candidate_files: 8,
        duplicate_candidate_bytes: 800,
        partial_hashes_attempted: 6,
        partial_hashes_succeeded: 5,
        partial_hashes_failed: 1,
        partial_collision_buckets: 1,
        partial_collision_files: 3,
        partial_collision_bytes: 300,
        full_hash_requests: 3,
        full_hash_cache_hits: 1,
        full_hash_cache_misses: 1,
        full_hash_cache_errors: 1,
        full_hash_content_reads_started: 2,
        full_hash_content_reads_completed: 1,
        confirmed_duplicate_groups: 1,
        confirmed_logical_copies: 2,
        confirmed_physical_items: 2,
        ..Default::default()
    };
    valid.validate().unwrap();

    let mut invalid = valid.clone();
    invalid.singleton_size_files = 3;
    assert_eq!(
        invalid.validate().unwrap_err().0,
        "every singleton size bucket contains exactly one file"
    );

    let mut invalid = valid;
    invalid.full_hash_content_reads_started = 3;
    assert_eq!(
        invalid.validate().unwrap_err().0,
        "content reads require a cache miss or cache error"
    );

    let overflow = ScanCounters {
        discovered_files: u64::MAX,
        zero_byte_files: u64::MAX,
        hard_link_alias_files: 1,
        ..Default::default()
    };
    assert_eq!(
        overflow.validate().unwrap_err().0,
        "excluded logical files cannot exceed discovered files"
    );
}

#[test]
fn status_schema_creates_reopens_and_stays_separate_from_product_tables() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("scan-status.db");
    let database = StatusDatabase::open(path.to_str().unwrap()).unwrap();
    assert_eq!(
        database.schema_version().unwrap(),
        CURRENT_STATUS_SCHEMA_VERSION
    );
    for table in [
        "status_run",
        "status_phase",
        "status_counter",
        "status_device",
        "status_host_sample",
        "status_device_sample",
        "status_flush",
    ] {
        let exists: bool = database
            .connection()
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
                 )",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "missing status table {table}");
    }
    let has_product_run: bool = database
        .connection()
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'scan_run'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!has_product_run);
    drop(database);

    let reopened = StatusDatabase::open(path.to_str().unwrap()).unwrap();
    assert_eq!(
        reopened.schema_version().unwrap(),
        CURRENT_STATUS_SCHEMA_VERSION
    );
}

#[test]
fn status_store_replays_exact_start_and_flush_but_rejects_conflicts_and_regressions() {
    let mut database = StatusDatabase::open_in_memory().unwrap();
    let start = run_start("run-op-1");
    let (run, disposition) = database.begin_run(&start).unwrap();
    assert_eq!(disposition, WriteDisposition::Applied);
    let (replayed, disposition) = database.begin_run(&start).unwrap();
    assert_eq!(disposition, WriteDisposition::Replayed);
    assert_eq!(replayed, run);

    let mut conflicting_start = start.clone();
    conflicting_start.input_signature = "different".to_owned();
    assert!(database
        .begin_run(&conflicting_start)
        .unwrap_err()
        .to_string()
        .contains("conflicts"));

    let first = flush(1, 10);
    assert_eq!(
        database.flush(run.id, &first).unwrap(),
        WriteDisposition::Applied
    );
    assert_eq!(
        database.flush(run.id, &first).unwrap(),
        WriteDisposition::Replayed
    );

    let mut conflicting_flush = first.clone();
    conflicting_flush.counters.warnings = 1;
    assert!(database
        .flush(run.id, &conflicting_flush)
        .unwrap_err()
        .to_string()
        .contains("conflicts"));

    let mut phase_regressed = flush(2, 10);
    phase_regressed.phase_active_nanos = 1;
    assert!(database
        .flush(run.id, &phase_regressed)
        .unwrap_err()
        .to_string()
        .contains("phase_active_nanos regressed"));

    let regressed = flush(2, 9);
    assert!(database
        .flush(run.id, &regressed)
        .unwrap_err()
        .to_string()
        .contains("regressed"));
    let committed = database.get_run(run.id).unwrap();
    assert_eq!(committed.last_sequence, 1);
    let second_flush_exists: bool = database
        .connection()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM status_flush WHERE run_id = ?1 AND sequence = 2)",
            [run.id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!second_flush_exists);
}

#[test]
fn status_store_persists_atomic_counters_devices_and_explicit_unavailable_gauges() {
    let mut database = StatusDatabase::open_in_memory().unwrap();
    let (run, _) = database.begin_run(&run_start("run-op-2")).unwrap();
    database.flush(run.id, &flush(1, 10)).unwrap();

    let counter_count: i64 = database
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM status_counter WHERE run_id = ?1 AND phase = 'overall'",
            [run.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(counter_count as usize, CounterKind::ALL.len());
    let (private_bytes, unavailable): (Option<i64>, i64) = database
        .connection()
        .query_row(
            "SELECT process_private_bytes, unavailable_counter_count
             FROM status_host_sample WHERE run_id = ?1 AND sequence = 1",
            [run.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(private_bytes, None);
    assert_eq!(unavailable, 3);
    let (device_count, device_sample_count): (i64, i64) = database
        .connection()
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM status_device WHERE run_id = ?1),
                (SELECT COUNT(*) FROM status_device_sample WHERE run_id = ?1)",
            [run.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((device_count, device_sample_count), (1, 1));

    let terminal = StatusRunTerminal {
        state: TelemetryRunState::Completed,
        completed_unix_millis: 1_700_000_010_000,
        monotonic_nanos: 10_000_000_000,
        error_code: None,
        error_message: None,
    };
    assert_eq!(
        database.finish_run(run.id, &terminal).unwrap(),
        WriteDisposition::Applied
    );
    assert_eq!(
        database.finish_run(run.id, &terminal).unwrap(),
        WriteDisposition::Replayed
    );
    let finished = database.get_run(run.id).unwrap();
    assert_eq!(finished.state, TelemetryRunState::Completed);
    database
        .apply_retention(StatusRetentionPolicy::default())
        .unwrap();
    let retained_samples: i64 = database
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM status_host_sample WHERE run_id = ?1",
            [run.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(retained_samples, 1);
}

#[test]
fn status_store_batches_counter_snapshots_and_skips_unchanged_counter_writes() {
    let mut database = StatusDatabase::open_in_memory().unwrap();
    let (run, _) = database
        .begin_run(&run_start("run-op-counter-batch"))
        .unwrap();
    database.flush(run.id, &flush(1, 10)).unwrap();

    let mut second = flush(2, 10);
    second.counters.warnings = 1;
    database.flush(run.id, &second).unwrap();

    let (counter_count, first_sequence_count, second_sequence_count): (i64, i64, i64) = database
        .connection()
        .query_row(
            "SELECT COUNT(*),
                    SUM(CASE WHEN updated_sequence = 1 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN updated_sequence = 2 THEN 1 ELSE 0 END)
             FROM status_counter WHERE run_id = ?1 AND phase = 'overall'",
            [run.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(counter_count as usize, CounterKind::ALL.len());
    assert_eq!(first_sequence_count as usize, CounterKind::ALL.len() - 1);
    assert_eq!(second_sequence_count, 1);

    let warning = database
        .get_run_counters(run.id)
        .unwrap()
        .into_iter()
        .find(|counter| counter.metric == CounterKind::Warnings.as_str())
        .unwrap();
    assert_eq!(warning.value, 1);
    assert_eq!(warning.updated_sequence, 2);
}

#[test]
fn status_store_reconciles_interrupted_runs_and_preserves_product_database() {
    let temp = tempdir().unwrap();
    let status_path = temp.path().join("status.db");
    let product_path = temp.path().join("product.db");
    let product = Connection::open(&product_path).unwrap();
    product
        .execute_batch(
            "CREATE TABLE immutable_result(id INTEGER PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO immutable_result(value) VALUES ('preserve me');",
        )
        .unwrap();
    drop(product);

    let run_id = {
        let mut database = StatusDatabase::open_connection(status_path.to_str().unwrap()).unwrap();
        let (run, _) = database.begin_run(&run_start("run-op-3")).unwrap();
        database.flush(run.id, &flush(1, 10)).unwrap();
        run.id
    };
    let database = StatusDatabase::open(status_path.to_str().unwrap()).unwrap();
    let interrupted = database.get_run(run_id).unwrap();
    assert_eq!(interrupted.state, TelemetryRunState::Interrupted);
    assert_eq!(interrupted.error_code.as_deref(), Some("worker_restarted"));
    let phase_state: String = database
        .connection()
        .query_row(
            "SELECT state FROM status_phase WHERE run_id = ?1 AND phase = 'discovering'",
            [run_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(phase_state, "interrupted");

    let product = Connection::open(&product_path).unwrap();
    let value: String = product
        .query_row("SELECT value FROM immutable_result", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, "preserve me");
    let has_status_table: bool = product
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'status_run'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!has_status_table);
}

#[test]
fn version_one_status_schema_migrates_transactionally() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("status-v1.db");
    let database = StatusDatabase::open_connection(path.to_str().unwrap()).unwrap();
    database
        .connection()
        .execute_batch("DROP TABLE status_flush; PRAGMA user_version = 1;")
        .unwrap();
    drop(database);

    let migrated = StatusDatabase::open_connection(path.to_str().unwrap()).unwrap();
    assert_eq!(
        migrated.schema_version().unwrap(),
        CURRENT_STATUS_SCHEMA_VERSION
    );
    let exists: bool = migrated
        .connection()
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'status_flush'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(exists);
}

#[test]
fn failed_version_one_status_migration_rolls_back_without_reinterpreting_state() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("status-v1-conflict.db");
    let database = StatusDatabase::open_connection(path.to_str().unwrap()).unwrap();
    database
        .connection()
        .execute_batch(
            "DROP TABLE status_flush;
             CREATE TABLE status_flush(unrelated_value TEXT NOT NULL);
             INSERT INTO status_flush(unrelated_value) VALUES ('preserve me');
             PRAGMA user_version = 1;",
        )
        .unwrap();
    drop(database);

    let error = StatusDatabase::open_connection(path.to_str().unwrap())
        .err()
        .expect("conflicting migration must fail");
    assert!(error.to_string().contains("already exists"));
    let raw = Connection::open(&path).unwrap();
    let version: i64 = raw
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 1);
    let preserved: String = raw
        .query_row("SELECT unrelated_value FROM status_flush", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(preserved, "preserve me");
}

#[test]
fn status_schema_rejects_newer_or_unversioned_nonempty_databases_without_changes() {
    let temp = tempdir().unwrap();
    let newer_path = temp.path().join("newer-status.db");
    let newer = Connection::open(&newer_path).unwrap();
    newer
        .execute_batch("CREATE TABLE future_evidence(id INTEGER); PRAGMA user_version = 99;")
        .unwrap();
    drop(newer);

    let error = StatusDatabase::open(newer_path.to_str().unwrap())
        .err()
        .expect("newer schema must fail");
    assert!(error.to_string().contains("newer than supported"));
    let newer = Connection::open(&newer_path).unwrap();
    let version: i64 = newer
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 99);
    let row_count: i64 = newer
        .query_row("SELECT COUNT(*) FROM future_evidence", [], |row| row.get(0))
        .unwrap();
    assert_eq!(row_count, 0);

    let legacy_path = temp.path().join("unversioned-status.db");
    let legacy = Connection::open(&legacy_path).unwrap();
    legacy
        .execute_batch("CREATE TABLE unknown_local_state(id INTEGER PRIMARY KEY);")
        .unwrap();
    drop(legacy);
    let error = StatusDatabase::open(legacy_path.to_str().unwrap())
        .err()
        .expect("unversioned non-empty schema must fail");
    assert!(error.to_string().contains("was not modified"));
    let legacy = Connection::open(&legacy_path).unwrap();
    let exists: bool = legacy
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'unknown_local_state'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(exists);
}

#[test]
fn status_queries_use_stable_bounded_cursors_and_fixed_summaries() {
    let mut database = StatusDatabase::open_in_memory().unwrap();
    let mut run_ids = Vec::new();
    for index in 1..=3 {
        let (run, _) = database
            .begin_run(&run_start(&format!("query-run-{index}")))
            .unwrap();
        database.flush(run.id, &flush(1, index * 10)).unwrap();
        database.flush(run.id, &flush(2, index * 10 + 1)).unwrap();
        database
            .finish_run(
                run.id,
                &StatusRunTerminal {
                    state: TelemetryRunState::Completed,
                    completed_unix_millis: 1_700_000_020_000 + index as i64,
                    monotonic_nanos: 20_000_000_000,
                    error_code: None,
                    error_message: None,
                },
            )
            .unwrap();
        run_ids.push(run.id);
    }

    let first = database.list_runs(None, 2).unwrap();
    assert_eq!(
        first.iter().map(|run| run.id).collect::<Vec<_>>(),
        vec![run_ids[2], run_ids[1]]
    );
    let second = database.list_runs(Some(first[1].id), 2).unwrap();
    assert_eq!(
        second.iter().map(|run| run.id).collect::<Vec<_>>(),
        vec![run_ids[0]]
    );
    assert!(database.list_runs(None, 0).is_err());
    assert!(database.list_runs(None, MAX_STATUS_RUN_PAGE + 1).is_err());

    let counters = database.get_run_counters(run_ids[2]).unwrap();
    assert_eq!(counters.len(), CounterKind::ALL.len());
    assert!(counters
        .windows(2)
        .all(|pair| pair[0].metric < pair[1].metric));
    let phases = database.get_run_phases(run_ids[2]).unwrap();
    assert_eq!(phases.len(), 1);
    assert_eq!(phases[0].phase, TelemetryPhase::Discovering);
    assert_eq!(phases[0].state, TelemetryPhaseState::Completed);
    let devices = database.get_run_devices(run_ids[2]).unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device_key, "physical:0");

    let host_first = database.list_host_samples(run_ids[2], 0, 1).unwrap();
    assert_eq!(host_first[0].sequence, 1);
    let host_second = database
        .list_host_samples(run_ids[2], host_first[0].sequence, 2)
        .unwrap();
    assert_eq!(
        host_second
            .iter()
            .map(|sample| sample.sequence)
            .collect::<Vec<_>>(),
        vec![2]
    );
    let device_samples = database
        .list_device_samples(run_ids[2], "physical:0", 0, 2)
        .unwrap();
    assert_eq!(device_samples.len(), 2);
    assert!(database
        .list_host_samples(run_ids[2], 0, MAX_STATUS_SAMPLE_PAGE + 1)
        .is_err());
    assert!(database.list_host_samples(999_999, 0, 1).is_err());
}

#[test]
fn status_device_summary_cardinality_is_bounded_atomically() {
    let mut database = StatusDatabase::open_in_memory().unwrap();
    let (run, _) = database.begin_run(&run_start("device-bound")).unwrap();
    let descriptors = |start: usize, count: usize| {
        (start..start + count)
            .map(|index| DeviceDescriptor {
                device_key: format!("physical:{index}"),
                volume_key: format!("volume:{index}"),
                ..Default::default()
            })
            .collect::<Vec<_>>()
    };
    let mut first = flush(1, 10);
    first.devices = descriptors(0, 40);
    first.device_samples.clear();
    database.flush(run.id, &first).unwrap();

    let mut overflow = flush(2, 10);
    overflow.devices = descriptors(40, 30);
    overflow.device_samples.clear();
    let error = database.flush(run.id, &overflow).unwrap_err();
    assert!(error.to_string().contains("more than 64 devices"));
    assert_eq!(database.get_run_devices(run.id).unwrap().len(), 40);
    assert_eq!(database.get_run(run.id).unwrap().last_sequence, 1);
}

#[test]
fn retention_is_idempotent_bounded_and_isolated_from_product_results() {
    assert_eq!(
        StatusRetentionPolicy::default(),
        StatusRetentionPolicy {
            max_terminal_runs: 50,
            max_samples_per_run: 100_000,
        }
    );
    let temp = tempdir().unwrap();
    let status_path = temp.path().join("retention-status.db");
    let product_path = temp.path().join("retention-product.db");
    let product = Connection::open(&product_path).unwrap();
    product
        .execute_batch(
            "CREATE TABLE immutable_result(id INTEGER PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO immutable_result(value) VALUES ('preserve me');",
        )
        .unwrap();
    drop(product);

    let mut database = StatusDatabase::open_connection(status_path.to_str().unwrap()).unwrap();
    let autocheckpoint: i64 = database
        .connection()
        .query_row("PRAGMA wal_autocheckpoint", [], |row| row.get(0))
        .unwrap();
    assert_eq!(autocheckpoint, 1000);
    let mut terminal_ids = Vec::new();
    for index in 1..=4 {
        let (run, _) = database
            .begin_run(&run_start(&format!("retention-terminal-{index}")))
            .unwrap();
        for sequence in 1..=4 {
            database
                .flush(run.id, &flush(sequence, index * 10 + sequence))
                .unwrap();
        }
        database
            .finish_run(
                run.id,
                &StatusRunTerminal {
                    state: TelemetryRunState::Completed,
                    completed_unix_millis: 1_700_000_030_000 + index as i64,
                    monotonic_nanos: 30_000_000_000,
                    error_code: None,
                    error_message: None,
                },
            )
            .unwrap();
        terminal_ids.push(run.id);
    }
    let (active, _) = database.begin_run(&run_start("retention-active")).unwrap();
    for sequence in 1..=4 {
        database
            .flush(active.id, &flush(sequence, 100 + sequence))
            .unwrap();
    }

    let result = database
        .apply_retention(StatusRetentionPolicy {
            max_terminal_runs: 2,
            max_samples_per_run: 2,
        })
        .unwrap();
    assert_eq!(result.terminal_runs_deleted, 2);
    assert_eq!(result.host_samples_deleted, 14);
    assert_eq!(result.device_samples_deleted, 14);
    assert_eq!(result.replay_flushes_deleted, 8);
    assert!(database.get_run(terminal_ids[0]).is_err());
    assert!(database.get_run(terminal_ids[1]).is_err());
    assert_eq!(
        database.get_run(active.id).unwrap().state,
        TelemetryRunState::Running
    );
    let oversized_sample_sets: i64 = database
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM (
                 SELECT run_id FROM status_host_sample GROUP BY run_id HAVING COUNT(*) > 2
             )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(oversized_sample_sets, 0);
    drop(database);

    let mut reopened = StatusDatabase::open_connection(status_path.to_str().unwrap()).unwrap();
    let repeated = reopened
        .apply_retention(StatusRetentionPolicy {
            max_terminal_runs: 2,
            max_samples_per_run: 2,
        })
        .unwrap();
    assert_eq!(repeated.terminal_runs_deleted, 0);
    assert_eq!(repeated.host_samples_deleted, 0);
    assert_eq!(repeated.device_samples_deleted, 0);
    assert_eq!(repeated.replay_flushes_deleted, 0);
    assert_eq!(reopened.delete_terminal_history().unwrap(), 2);
    assert_eq!(reopened.list_runs(None, 10).unwrap().len(), 1);
    assert_eq!(
        reopened.get_run(active.id).unwrap().state,
        TelemetryRunState::Running
    );

    let product = Connection::open(&product_path).unwrap();
    let value: String = product
        .query_row("SELECT value FROM immutable_result", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, "preserve me");
    let has_status_table: bool = product
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'status_run'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!has_status_table);
}
