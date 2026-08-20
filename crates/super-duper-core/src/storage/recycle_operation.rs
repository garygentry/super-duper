use std::collections::{BTreeMap, HashSet};
use std::hash::Hasher;

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, OptionalExtension, Row, Transaction, TransactionBehavior};
use thiserror::Error;
use twox_hash::XxHash64;

use super::models::{
    RecycleEligibilityObservation, RecycleItemResultObservation, RecycleOperation,
    RecycleOperationBatch, RecycleOperationItem, RecycleOperationItemPage,
    RecycleOperationMutationResult, RecycleOperationSummary, RecycleOperationView,
};
use super::Database;

const MAXIMUM_OPERATION_ID_CHARACTERS: usize = 128;
const MAXIMUM_BATCH_ITEMS: usize = 32;
const PREPARATION_FRESHNESS_SECONDS: i64 = 300;
const CONFIRMATION_FRESHNESS_SECONDS: i64 = 60;
const SUBMISSION_FRESHNESS_SECONDS: i64 = 30;

#[derive(Debug, Error)]
pub enum RecycleOperationError {
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error("invalid recycle operation request: {message}")]
    InvalidRequest { message: String },
    #[error("run {run_id} was not found")]
    RunNotFound { run_id: i64 },
    #[error("preflight {preflight_id} was not found")]
    PreflightNotFound { preflight_id: i64 },
    #[error("preflight {preflight_id} is {status}; a completed generation is required")]
    PreflightNotCompleted { preflight_id: i64, status: String },
    #[error("preflight {preflight_id} is not the latest generation for run {run_id}")]
    LatestPreflightRequired { preflight_id: i64, run_id: i64 },
    #[error("review revision {expected} is stale; current revision is {current}")]
    StaleReviewRevision { expected: i64, current: i64 },
    #[error("preflight {preflight_id} is outside the preparation freshness lease")]
    PreflightExpired { preflight_id: i64 },
    #[error("preflight {preflight_id} is not eligible: {reason}")]
    IneligiblePreflight { preflight_id: i64, reason: String },
    #[error("operation id {operation_id} was already used with another payload")]
    IdempotencyConflict { operation_id: String },
    #[error("recycle operation {operation_id} was not found")]
    NotFound { operation_id: i64 },
    #[error("recycle operation {operation_id} cannot transition from {status}")]
    InvalidState { operation_id: i64, status: String },
    #[error("run {run_id} is locked by recycle operation {operation_id}")]
    OperationLocked { run_id: i64, operation_id: i64 },
    #[error("confirmation for recycle operation {operation_id} expired")]
    ConfirmationExpired { operation_id: i64 },
    #[error(
        "submission lease for recycle operation {operation_id} expired before batch admission"
    )]
    SubmissionExpired { operation_id: i64 },
    #[error("item {item_id} does not belong to recycle operation {operation_id}")]
    ItemNotFound { operation_id: i64, item_id: i64 },
    #[error("batch {batch_id} does not belong to recycle operation {operation_id}")]
    BatchNotFound { operation_id: i64, batch_id: i64 },
}

#[derive(Clone)]
struct CandidateItem {
    preflight_item_id: i64,
    preflight_source_id: Option<i64>,
    target_kind: String,
    physical_key: String,
    snapshot_path: String,
    group_id: Option<i64>,
    folder_group_id: Option<i64>,
    folder_member_id: Option<i64>,
    snapshot_file_id: Option<i64>,
    snapshot_directory_id: Option<i64>,
    planned_bytes: i64,
}

impl Database {
    pub fn prepare_recycle_operation(
        &self,
        operation_id: &str,
        run_id: i64,
        preflight_id: i64,
        expected_review_revision: i64,
    ) -> Result<RecycleOperationMutationResult, RecycleOperationError> {
        validate_operation_id(operation_id)?;
        if run_id <= 0 || preflight_id <= 0 || expected_review_revision < 0 {
            return Err(RecycleOperationError::InvalidRequest {
                message: "runId and preflightId must be positive and expectedReviewRevision must be non-negative".to_owned(),
            });
        }
        let now = Utc::now();
        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        if let Some((existing_id, saved_run_id, saved_preflight_id, saved_revision)) = tx
            .query_row(
                "SELECT id, run_id, preflight_id, review_revision
                 FROM recycle_operation WHERE operation_id = ?1",
                params![operation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?
        {
            if saved_run_id != run_id
                || saved_preflight_id != preflight_id
                || saved_revision != expected_review_revision
            {
                return Err(RecycleOperationError::IdempotencyConflict {
                    operation_id: operation_id.to_owned(),
                });
            }
            tx.commit()?;
            return Ok(RecycleOperationMutationResult {
                view: self.get_recycle_operation(existing_id)?,
                replayed: true,
            });
        }

        let snapshot = tx
            .query_row(
                "SELECT preflight.run_id, preflight.plan_id, preflight.review_revision,
                        preflight.snapshot_signature, preflight.status, preflight.completed_at,
                        preflight.logical_removal_count, preflight.physical_removal_count,
                        preflight.folder_removal_count, preflight.affected_group_count,
                        preflight.planned_removal_bytes, plan.revision, plan.state, run.status,
                        (SELECT id FROM preflight newest WHERE newest.run_id = preflight.run_id
                         ORDER BY newest.id DESC LIMIT 1), run.excluded_subtree_count
                 FROM preflight
                 JOIN review_plan plan ON plan.id = preflight.plan_id
                 JOIN scan_run run ON run.id = preflight.run_id
                 WHERE preflight.id = ?1",
                params![preflight_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, String>(13)?,
                        row.get::<_, i64>(14)?,
                        row.get::<_, i64>(15)?,
                    ))
                },
            )
            .optional()?
            .ok_or(RecycleOperationError::PreflightNotFound { preflight_id })?;
        if snapshot.0 != run_id {
            return Err(RecycleOperationError::PreflightNotFound { preflight_id });
        }
        if snapshot.4 != "completed" {
            return Err(RecycleOperationError::PreflightNotCompleted {
                preflight_id,
                status: snapshot.4,
            });
        }
        if snapshot.13 != "completed" {
            return Err(RecycleOperationError::IneligiblePreflight {
                preflight_id,
                reason: format!("run is {}", snapshot.13),
            });
        }
        if snapshot.14 != preflight_id {
            return Err(RecycleOperationError::LatestPreflightRequired {
                preflight_id,
                run_id,
            });
        }
        if snapshot.12 != "active"
            || snapshot.2 != expected_review_revision
            || snapshot.11 != expected_review_revision
        {
            return Err(RecycleOperationError::StaleReviewRevision {
                expected: expected_review_revision,
                current: snapshot.11,
            });
        }
        let completed_at = snapshot
            .5
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
            .ok_or(RecycleOperationError::PreflightExpired { preflight_id })?;
        let age = now.signed_duration_since(completed_at).num_seconds();
        if !(0..=PREPARATION_FRESHNESS_SECONDS).contains(&age) {
            return Err(RecycleOperationError::PreflightExpired { preflight_id });
        }
        if let Some(locked_id) = locked_operation_id(&tx, run_id)? {
            return Err(RecycleOperationError::OperationLocked {
                run_id,
                operation_id: locked_id,
            });
        }
        let non_ready_removals: i64 = tx.query_row(
            "SELECT COUNT(*) FROM preflight_item
             WHERE preflight_id = ?1 AND target_role = 'remove' AND outcome <> 'ready'",
            params![preflight_id],
            |row| row.get(0),
        )?;
        if non_ready_removals != 0 {
            return Err(RecycleOperationError::IneligiblePreflight {
                preflight_id,
                reason: format!("{non_ready_removals} removal item(s) are not ready"),
            });
        }

        let mut candidates = BTreeMap::<String, CandidateItem>::new();
        {
            let mut statement = tx.prepare(
                "SELECT item.id, source.id, item.physical_key, source.snapshot_path,
                        source.group_id, source.folder_group_id, source.folder_member_id,
                        source.file_id, item.snapshot_file_size
                 FROM preflight_item item
                 JOIN preflight_item_source source ON source.item_id = item.id
                 WHERE item.preflight_id = ?1 AND item.target_role = 'remove'
                   AND item.target_kind = 'file' AND item.outcome = 'ready'
                   AND source.source_kind = 'file_decision'
                 ORDER BY source.snapshot_path COLLATE UNICODE_NOCASE, source.id",
            )?;
            let rows = statement.query_map(params![preflight_id], |row| {
                Ok(CandidateItem {
                    preflight_item_id: row.get(0)?,
                    preflight_source_id: Some(row.get(1)?),
                    target_kind: "file".to_owned(),
                    physical_key: row.get(2)?,
                    snapshot_path: row.get(3)?,
                    group_id: row.get(4)?,
                    folder_group_id: row.get(5)?,
                    folder_member_id: row.get(6)?,
                    snapshot_file_id: row.get(7)?,
                    snapshot_directory_id: None,
                    planned_bytes: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
                })
            })?;
            for candidate in rows {
                let candidate = candidate?;
                candidates.insert(
                    format!("file:{}", candidate.snapshot_path.to_lowercase()),
                    candidate,
                );
            }
        }
        {
            let mut statement = tx.prepare(
                "SELECT item.id, item.physical_key, item.snapshot_path, item.group_id,
                        item.folder_group_id, item.folder_member_id, item.snapshot_directory_id
                 FROM preflight_item item
                 WHERE item.preflight_id = ?1 AND item.target_role = 'remove'
                   AND item.target_kind = 'folder' AND item.outcome = 'ready'
                 ORDER BY item.snapshot_path COLLATE UNICODE_NOCASE, item.id",
            )?;
            let rows = statement.query_map(params![preflight_id], |row| {
                Ok(CandidateItem {
                    preflight_item_id: row.get(0)?,
                    preflight_source_id: None,
                    target_kind: "folder".to_owned(),
                    physical_key: row.get(1)?,
                    snapshot_path: row.get(2)?,
                    group_id: row.get(3)?,
                    folder_group_id: row.get(4)?,
                    folder_member_id: row.get(5)?,
                    snapshot_file_id: None,
                    snapshot_directory_id: row.get(6)?,
                    planned_bytes: 0,
                })
            })?;
            for candidate in rows {
                let candidate = candidate?;
                candidates.insert(
                    format!("folder:{}", candidate.snapshot_path.to_lowercase()),
                    candidate,
                );
            }
        }
        if candidates.is_empty() {
            return Err(RecycleOperationError::IneligiblePreflight {
                preflight_id,
                reason: "no top-level Shell items were produced".to_owned(),
            });
        }
        let candidates = candidates.into_values().collect::<Vec<_>>();
        let intent_signature = candidate_signature(
            run_id,
            snapshot.1,
            expected_review_revision,
            &snapshot.3,
            &candidates,
        );
        let affected_location_count: i64 = tx.query_row(
            "SELECT COUNT(DISTINCT file.root_path)
             FROM preflight_item item
             JOIN preflight_item_source source ON source.item_id = item.id
             JOIN scanned_file file ON file.id = source.file_id
             WHERE item.preflight_id = ?1 AND item.target_role = 'remove'",
            params![preflight_id],
            |row| row.get(0),
        )?;
        let prepared_at = now.to_rfc3339();
        tx.execute(
            "INSERT INTO recycle_operation
                (operation_id, run_id, plan_id, preflight_id, review_revision,
                 preflight_snapshot_signature, intent_signature, policy_version, status,
                 logical_removal_count, shell_item_count, physical_item_count, folder_item_count,
                 affected_group_count, planned_removal_bytes, affected_location_count,
                 exclusion_count, prepared_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 'prepared', ?8, ?9, ?10, ?11,
                     ?12, ?13, ?14, ?15, ?16)",
            params![
                operation_id,
                run_id,
                snapshot.1,
                preflight_id,
                expected_review_revision,
                snapshot.3,
                intent_signature,
                snapshot.6,
                candidates.len() as i64,
                snapshot.7,
                snapshot.8,
                snapshot.9,
                snapshot.10,
                affected_location_count,
                snapshot.15,
                prepared_at,
            ],
        )?;
        let recycle_operation_id = tx.last_insert_rowid();
        let files = candidates
            .iter()
            .filter(|item| item.target_kind == "file")
            .cloned()
            .collect::<Vec<_>>();
        let folders = candidates
            .iter()
            .filter(|item| item.target_kind == "folder")
            .cloned()
            .collect::<Vec<_>>();
        let mut batches = files
            .chunks(MAXIMUM_BATCH_ITEMS)
            .map(|chunk| chunk.to_vec())
            .collect::<Vec<_>>();
        batches.extend(folders.into_iter().map(|folder| vec![folder]));
        let mut item_ordinal = 0i64;
        for (batch_ordinal, batch_items) in batches.into_iter().enumerate() {
            let batch_signature = batch_signature(&batch_items);
            tx.execute(
                "INSERT INTO recycle_operation_batch
                    (recycle_operation_id, ordinal, item_signature, status)
                 VALUES (?1, ?2, ?3, 'pending')",
                params![recycle_operation_id, batch_ordinal as i64, batch_signature],
            )?;
            let batch_id = tx.last_insert_rowid();
            for item in batch_items {
                tx.execute(
                    "INSERT INTO recycle_operation_item
                        (recycle_operation_id, batch_id, ordinal, preflight_item_id,
                         preflight_source_id, target_kind, physical_key, snapshot_path,
                         group_id, folder_group_id, folder_member_id, snapshot_file_id,
                         snapshot_directory_id, planned_bytes)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![
                        recycle_operation_id,
                        batch_id,
                        item_ordinal,
                        item.preflight_item_id,
                        item.preflight_source_id,
                        item.target_kind,
                        item.physical_key,
                        item.snapshot_path,
                        item.group_id,
                        item.folder_group_id,
                        item.folder_member_id,
                        item.snapshot_file_id,
                        item.snapshot_directory_id,
                        item.planned_bytes,
                    ],
                )?;
                item_ordinal += 1;
            }
        }
        tx.commit()?;
        Ok(RecycleOperationMutationResult {
            view: self.get_recycle_operation(recycle_operation_id)?,
            replayed: false,
        })
    }

    pub fn get_recycle_operation(
        &self,
        operation_id: i64,
    ) -> Result<RecycleOperationView, RecycleOperationError> {
        let operation = recycle_operation_by_id(self.connection(), operation_id)?
            .ok_or(RecycleOperationError::NotFound { operation_id })?;
        let current_review_revision = self
            .connection()
            .query_row(
                "SELECT revision FROM review_plan WHERE id = ?1 AND state = 'active'",
                params![operation.plan_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(-1);
        Ok(RecycleOperationView {
            is_current: current_review_revision == operation.review_revision,
            current_review_revision,
            operation,
        })
    }

    pub fn latest_recycle_operation_for_run(
        &self,
        run_id: i64,
    ) -> Result<Option<RecycleOperationView>, RecycleOperationError> {
        let id = self
            .connection()
            .query_row(
                "SELECT id FROM recycle_operation WHERE run_id = ?1 ORDER BY id DESC LIMIT 1",
                params![run_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        id.map(|id| self.get_recycle_operation(id)).transpose()
    }

    pub fn page_recycle_operation_items(
        &self,
        operation_id: i64,
        offset: i64,
        limit: i64,
        result_status: Option<&str>,
    ) -> Result<RecycleOperationItemPage, RecycleOperationError> {
        if offset < 0 || !(1..=200).contains(&limit) {
            return Err(RecycleOperationError::InvalidRequest {
                message: "offset must be non-negative and limit must be between 1 and 200"
                    .to_owned(),
            });
        }
        if result_status.is_some_and(|value| {
            !matches!(
                value,
                "pending" | "recycled" | "failed" | "cancelled" | "unknown"
            )
        }) {
            return Err(RecycleOperationError::InvalidRequest {
                message: "resultStatus must be pending, recycled, failed, cancelled, or unknown"
                    .to_owned(),
            });
        }
        if recycle_operation_by_id(self.connection(), operation_id)?.is_none() {
            return Err(RecycleOperationError::NotFound { operation_id });
        }
        let total = self.connection().query_row(
            "SELECT COUNT(*) FROM recycle_operation_item
             WHERE recycle_operation_id = ?1 AND (?2 IS NULL OR result_status = ?2)",
            params![operation_id, result_status],
            |row| row.get::<_, i64>(0),
        )?;
        let mut statement = self.connection().prepare(
            "SELECT id, recycle_operation_id, batch_id, ordinal, preflight_item_id,
                    preflight_source_id, target_kind, physical_key, snapshot_path, group_id,
                    folder_group_id, folder_member_id, snapshot_file_id, snapshot_directory_id,
                    planned_bytes, eligibility_status, eligibility_code, result_status,
                    result_code, shell_hresult, recycled_item_present, result_at
             FROM recycle_operation_item
             WHERE recycle_operation_id = ?1 AND (?2 IS NULL OR result_status = ?2)
             ORDER BY CASE result_status WHEN 'unknown' THEN 0 WHEN 'failed' THEN 1
                        WHEN 'cancelled' THEN 2 WHEN 'pending' THEN 3 ELSE 4 END,
                      target_kind, snapshot_path COLLATE UNICODE_NOCASE, id
             LIMIT ?3 OFFSET ?4",
        )?;
        let items = statement
            .query_map(
                params![operation_id, result_status, limit, offset],
                recycle_item_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RecycleOperationItemPage {
            has_more: offset + (items.len() as i64) < total,
            items,
            total,
        })
    }

    pub fn report_recycle_eligibility(
        &self,
        report_operation_id: &str,
        operation_id: i64,
        observations: &[RecycleEligibilityObservation],
    ) -> Result<RecycleOperationMutationResult, RecycleOperationError> {
        validate_operation_id(report_operation_id)?;
        if observations.is_empty() || observations.len() > 200 {
            return Err(RecycleOperationError::InvalidRequest {
                message: "eligibility reports require 1 to 200 items".to_owned(),
            });
        }
        let mut seen = HashSet::new();
        for observation in observations {
            if observation.item_id <= 0
                || !seen.insert(observation.item_id)
                || !matches!(observation.status.as_str(), "eligible" | "non_recyclable")
            {
                return Err(RecycleOperationError::InvalidRequest {
                    message: "eligibility items must be unique positive IDs with eligible or non_recyclable status".to_owned(),
                });
            }
        }
        let payload_signature = eligibility_signature(operation_id, observations);
        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        if report_replay(
            &tx,
            report_operation_id,
            operation_id,
            "eligibility",
            &payload_signature,
        )? {
            tx.commit()?;
            return Ok(RecycleOperationMutationResult {
                view: self.get_recycle_operation(operation_id)?,
                replayed: true,
            });
        }
        let status = operation_status(&tx, operation_id)?;
        if status != "prepared" {
            return Err(RecycleOperationError::InvalidState {
                operation_id,
                status,
            });
        }
        for observation in observations {
            let changed = tx.execute(
                "UPDATE recycle_operation_item
                 SET eligibility_status = ?1, eligibility_code = ?2
                 WHERE id = ?3 AND recycle_operation_id = ?4 AND eligibility_status = 'pending'",
                params![
                    observation.status,
                    observation.reason_code,
                    observation.item_id,
                    operation_id
                ],
            )?;
            if changed == 0 {
                let existing = tx
                    .query_row(
                        "SELECT eligibility_status, eligibility_code FROM recycle_operation_item
                         WHERE id = ?1 AND recycle_operation_id = ?2",
                        params![observation.item_id, operation_id],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                    )
                    .optional()?
                    .ok_or(RecycleOperationError::ItemNotFound {
                        operation_id,
                        item_id: observation.item_id,
                    })?;
                if existing.0 != observation.status || existing.1 != observation.reason_code {
                    return Err(RecycleOperationError::InvalidState {
                        operation_id,
                        status: "eligibility_already_reported".to_owned(),
                    });
                }
            }
        }
        insert_report(
            &tx,
            operation_id,
            None,
            report_operation_id,
            "eligibility",
            &payload_signature,
        )?;
        let (pending, blocked) = tx.query_row(
            "SELECT SUM(CASE WHEN eligibility_status = 'pending' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN eligibility_status = 'non_recyclable' THEN 1 ELSE 0 END)
             FROM recycle_operation_item WHERE recycle_operation_id = ?1",
            params![operation_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        if pending == 0 {
            let now = Utc::now();
            if blocked > 0 {
                tx.execute(
                    "UPDATE recycle_operation SET status = 'failed', completed_at = ?1,
                            error_code = 'non_recyclable_target',
                            error_detail = ?2 WHERE id = ?3 AND status = 'prepared'",
                    params![
                        now.to_rfc3339(),
                        format!("{blocked} target(s) are not positively recyclable"),
                        operation_id
                    ],
                )?;
            } else {
                let intent: String = tx.query_row(
                    "SELECT intent_signature FROM recycle_operation WHERE id = ?1",
                    params![operation_id],
                    |row| row.get(0),
                )?;
                let confirmation_signature = confirmation_signature(&intent, operation_id);
                tx.execute(
                    "UPDATE recycle_operation SET status = 'awaiting_confirmation',
                            confirmation_signature = ?1, confirmation_expires_at = ?2
                     WHERE id = ?3 AND status = 'prepared'",
                    params![
                        confirmation_signature,
                        (now + Duration::seconds(CONFIRMATION_FRESHNESS_SECONDS)).to_rfc3339(),
                        operation_id,
                    ],
                )?;
            }
        }
        tx.commit()?;
        Ok(RecycleOperationMutationResult {
            view: self.get_recycle_operation(operation_id)?,
            replayed: false,
        })
    }

    pub fn confirm_recycle_operation(
        &self,
        report_operation_id: &str,
        operation_id: i64,
        confirmation_signature_value: &str,
    ) -> Result<RecycleOperationMutationResult, RecycleOperationError> {
        validate_operation_id(report_operation_id)?;
        if confirmation_signature_value.is_empty() {
            return Err(RecycleOperationError::InvalidRequest {
                message: "confirmationSignature must not be empty".to_owned(),
            });
        }
        let payload_signature = text_signature(&format!(
            "confirm|{operation_id}|{confirmation_signature_value}"
        ));
        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        if report_replay(
            &tx,
            report_operation_id,
            operation_id,
            "confirmation",
            &payload_signature,
        )? {
            tx.commit()?;
            return Ok(RecycleOperationMutationResult {
                view: self.get_recycle_operation(operation_id)?,
                replayed: true,
            });
        }
        let (status, signature, expires_at, run_id, review_revision, preflight_id) = tx
            .query_row(
                "SELECT status, confirmation_signature, confirmation_expires_at, run_id,
                        review_revision, preflight_id FROM recycle_operation WHERE id = ?1",
                params![operation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?
            .ok_or(RecycleOperationError::NotFound { operation_id })?;
        if status != "awaiting_confirmation" {
            return Err(RecycleOperationError::InvalidState {
                operation_id,
                status,
            });
        }
        if signature.as_deref() != Some(confirmation_signature_value) {
            return Err(RecycleOperationError::IdempotencyConflict {
                operation_id: report_operation_id.to_owned(),
            });
        }
        let expires_at = expires_at
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc));
        if expires_at.map_or(true, |expires| Utc::now() > expires) {
            tx.execute(
                "UPDATE recycle_operation SET status = 'expired', completed_at = ?1,
                        error_code = 'confirmation_expired' WHERE id = ?2",
                params![Utc::now().to_rfc3339(), operation_id],
            )?;
            tx.commit()?;
            return Err(RecycleOperationError::ConfirmationExpired { operation_id });
        }
        let current = tx.query_row(
            "SELECT plan.revision,
                    (SELECT id FROM preflight WHERE run_id = ?1 ORDER BY id DESC LIMIT 1)
             FROM review_plan plan
             JOIN recycle_operation operation ON operation.plan_id = plan.id
             WHERE operation.id = ?2 AND plan.state = 'active'",
            params![run_id, operation_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        if current.0 != review_revision || current.1 != preflight_id {
            return Err(RecycleOperationError::StaleReviewRevision {
                expected: review_revision,
                current: current.0,
            });
        }
        insert_report(
            &tx,
            operation_id,
            None,
            report_operation_id,
            "confirmation",
            &payload_signature,
        )?;
        let submitted_at = Utc::now();
        let admission_expires_at =
            (submitted_at + Duration::seconds(SUBMISSION_FRESHNESS_SECONDS)).to_rfc3339();
        tx.execute(
            "UPDATE recycle_operation SET status = 'submitted', submitted_at = ?1
             WHERE id = ?2 AND status = 'awaiting_confirmation'",
            params![submitted_at.to_rfc3339(), operation_id],
        )?;
        tx.execute(
            "UPDATE recycle_operation_batch SET admission_expires_at = ?1
             WHERE recycle_operation_id = ?2 AND status = 'pending'",
            params![admission_expires_at, operation_id],
        )?;
        tx.commit()?;
        Ok(RecycleOperationMutationResult {
            view: self.get_recycle_operation(operation_id)?,
            replayed: false,
        })
    }

    pub fn cancel_recycle_operation(
        &self,
        operation_id: i64,
    ) -> Result<RecycleOperationView, RecycleOperationError> {
        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        let status = operation_status(&tx, operation_id)?;
        let now = Utc::now().to_rfc3339();
        match status.as_str() {
            "prepared" | "awaiting_confirmation" | "submitted" => {
                tx.execute(
                    "UPDATE recycle_operation SET status = 'cancelled',
                            cancellation_requested = 1, completed_at = ?1 WHERE id = ?2",
                    params![now, operation_id],
                )?;
                tx.execute(
                    "UPDATE recycle_operation_batch SET status = 'skipped'
                     WHERE recycle_operation_id = ?1 AND status IN ('pending', 'admitted')",
                    params![operation_id],
                )?;
                tx.execute(
                    "UPDATE recycle_operation_item SET result_status = 'cancelled',
                            result_code = 'cancelled_before_shell', result_at = ?1
                     WHERE recycle_operation_id = ?2 AND result_status = 'pending'",
                    params![now, operation_id],
                )?;
            }
            "executing" | "cancelling" => {
                tx.execute(
                    "UPDATE recycle_operation SET status = 'cancelling', cancellation_requested = 1
                     WHERE id = ?1",
                    params![operation_id],
                )?;
            }
            "expired"
            | "cancelled"
            | "completed"
            | "partially_completed"
            | "failed"
            | "recovery_required" => {}
            _ => {
                return Err(RecycleOperationError::InvalidState {
                    operation_id,
                    status,
                })
            }
        }
        tx.commit()?;
        self.get_recycle_operation(operation_id)
    }

    pub fn next_recycle_operation_batch(
        &self,
        operation_id: i64,
    ) -> Result<Option<RecycleOperationBatch>, RecycleOperationError> {
        let status = operation_status(self.connection(), operation_id)?;
        if !matches!(status.as_str(), "submitted" | "executing" | "cancelling") {
            return Err(RecycleOperationError::InvalidState {
                operation_id,
                status,
            });
        }
        if status == "cancelling" {
            return Ok(None);
        }
        let batch_id = self
            .connection()
            .query_row(
                "SELECT id FROM recycle_operation_batch
                 WHERE recycle_operation_id = ?1 AND status = 'pending'
                 ORDER BY ordinal LIMIT 1",
                params![operation_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        batch_id
            .map(|batch_id| recycle_batch_by_id(self.connection(), operation_id, batch_id))
            .transpose()
    }

    pub fn begin_recycle_operation_batch(
        &self,
        report_operation_id: &str,
        operation_id: i64,
        batch_id: i64,
        shell_attempt_id: &str,
    ) -> Result<RecycleOperationMutationResult, RecycleOperationError> {
        validate_operation_id(report_operation_id)?;
        validate_operation_id(shell_attempt_id)?;
        let payload_signature = text_signature(&format!(
            "begin|{operation_id}|{batch_id}|{shell_attempt_id}"
        ));
        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        if report_replay(
            &tx,
            report_operation_id,
            operation_id,
            "batch_begin",
            &payload_signature,
        )? {
            tx.commit()?;
            return Ok(RecycleOperationMutationResult {
                view: self.get_recycle_operation(operation_id)?,
                replayed: true,
            });
        }
        let status = operation_status(&tx, operation_id)?;
        if !matches!(status.as_str(), "submitted" | "executing") {
            return Err(RecycleOperationError::InvalidState {
                operation_id,
                status,
            });
        }
        let admission = tx
            .query_row(
                "SELECT batch.admission_expires_at, operation.review_revision,
                        operation.preflight_id, plan.revision,
                        (SELECT id FROM preflight newest WHERE newest.run_id = operation.run_id
                         ORDER BY newest.id DESC LIMIT 1)
                 FROM recycle_operation_batch batch
                 JOIN recycle_operation operation ON operation.id = batch.recycle_operation_id
                 JOIN review_plan plan ON plan.id = operation.plan_id AND plan.state = 'active'
                 WHERE batch.id = ?1 AND operation.id = ?2",
                params![batch_id, operation_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()?
            .ok_or(RecycleOperationError::BatchNotFound {
                operation_id,
                batch_id,
            })?;
        if admission.1 != admission.3 || admission.2 != admission.4 {
            return Err(RecycleOperationError::StaleReviewRevision {
                expected: admission.1,
                current: admission.3,
            });
        }
        let admission_expires_at = admission
            .0
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc));
        if admission_expires_at.map_or(true, |expires| Utc::now() > expires) {
            tx.execute(
                "UPDATE recycle_operation SET status = 'expired', completed_at = ?1,
                        error_code = 'submission_lease_expired'
                 WHERE id = ?2 AND status = 'submitted'",
                params![Utc::now().to_rfc3339(), operation_id],
            )?;
            tx.commit()?;
            return Err(RecycleOperationError::SubmissionExpired { operation_id });
        }
        let eligible: bool = tx
            .query_row(
                "SELECT NOT EXISTS(SELECT 1 FROM recycle_operation_item
                    WHERE batch_id = ?1 AND eligibility_status <> 'eligible')",
                params![batch_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(RecycleOperationError::BatchNotFound {
                operation_id,
                batch_id,
            })?;
        if !eligible {
            return Err(RecycleOperationError::InvalidState {
                operation_id,
                status: "batch_not_eligible".to_owned(),
            });
        }
        let changed = tx.execute(
            "UPDATE recycle_operation_batch SET status = 'shell_started',
                    shell_attempt_id = ?1, started_at = ?2
             WHERE id = ?3 AND recycle_operation_id = ?4 AND status = 'pending'",
            params![
                shell_attempt_id,
                Utc::now().to_rfc3339(),
                batch_id,
                operation_id
            ],
        )?;
        if changed == 0 {
            return Err(RecycleOperationError::BatchNotFound {
                operation_id,
                batch_id,
            });
        }
        insert_report(
            &tx,
            operation_id,
            Some(batch_id),
            report_operation_id,
            "batch_begin",
            &payload_signature,
        )?;
        tx.execute(
            "UPDATE recycle_operation SET status = 'executing'
             WHERE id = ?1 AND status = 'submitted'",
            params![operation_id],
        )?;
        tx.commit()?;
        Ok(RecycleOperationMutationResult {
            view: self.get_recycle_operation(operation_id)?,
            replayed: false,
        })
    }

    pub fn report_recycle_operation_batch(
        &self,
        report_operation_id: &str,
        operation_id: i64,
        batch_id: i64,
        observations: &[RecycleItemResultObservation],
    ) -> Result<RecycleOperationMutationResult, RecycleOperationError> {
        validate_operation_id(report_operation_id)?;
        if observations.len() > MAXIMUM_BATCH_ITEMS {
            return Err(RecycleOperationError::InvalidRequest {
                message: "result reports may contain at most 32 items".to_owned(),
            });
        }
        let payload_signature = result_signature(operation_id, batch_id, observations);
        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        if report_replay(
            &tx,
            report_operation_id,
            operation_id,
            "result",
            &payload_signature,
        )? {
            tx.commit()?;
            return Ok(RecycleOperationMutationResult {
                view: self.get_recycle_operation(operation_id)?,
                replayed: true,
            });
        }
        let batch_status = tx
            .query_row(
                "SELECT status FROM recycle_operation_batch
                 WHERE id = ?1 AND recycle_operation_id = ?2",
                params![batch_id, operation_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(RecycleOperationError::BatchNotFound {
                operation_id,
                batch_id,
            })?;
        if batch_status != "shell_started" {
            return Err(RecycleOperationError::InvalidState {
                operation_id,
                status: format!("batch_{batch_status}"),
            });
        }
        let mut observations_by_id = BTreeMap::new();
        for observation in observations {
            if observation.item_id <= 0
                || !matches!(
                    observation.status.as_str(),
                    "recycled" | "failed" | "cancelled" | "unknown"
                )
                || observations_by_id
                    .insert(observation.item_id, observation)
                    .is_some()
            {
                return Err(RecycleOperationError::InvalidRequest {
                    message: "result items must be unique positive IDs with a terminal item status"
                        .to_owned(),
                });
            }
        }
        let item_ids = {
            let mut statement = tx.prepare(
                "SELECT id FROM recycle_operation_item WHERE batch_id = ?1 ORDER BY ordinal",
            )?;
            let ids = statement
                .query_map(params![batch_id], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            ids
        };
        let batch_item_ids = item_ids.iter().copied().collect::<HashSet<_>>();
        if let Some(item_id) = observations_by_id
            .keys()
            .find(|item_id| !batch_item_ids.contains(item_id))
        {
            return Err(RecycleOperationError::ItemNotFound {
                operation_id,
                item_id: *item_id,
            });
        }
        let now = Utc::now().to_rfc3339();
        for item_id in item_ids {
            let observation = observations_by_id.get(&item_id);
            let (status, reason, hresult, recycled_present) = match observation {
                Some(value)
                    if value.status == "recycled" && value.recycled_item_present == Some(true) =>
                {
                    (
                        "recycled",
                        value
                            .reason_code
                            .clone()
                            .or_else(|| Some("recycled".to_owned())),
                        value.shell_hresult,
                        Some(true),
                    )
                }
                Some(value) if value.status == "recycled" => (
                    "unknown",
                    Some("missing_recycled_shell_item".to_owned()),
                    value.shell_hresult,
                    value.recycled_item_present,
                ),
                Some(value) => (
                    value.status.as_str(),
                    value.reason_code.clone(),
                    value.shell_hresult,
                    value.recycled_item_present,
                ),
                None => (
                    "unknown",
                    Some("missing_shell_callback".to_owned()),
                    None,
                    None,
                ),
            };
            tx.execute(
                "UPDATE recycle_operation_item SET result_status = ?1, result_code = ?2,
                        shell_hresult = ?3, recycled_item_present = ?4, result_at = ?5
                 WHERE id = ?6 AND batch_id = ?7",
                params![
                    status,
                    reason,
                    hresult,
                    recycled_present,
                    now,
                    item_id,
                    batch_id
                ],
            )?;
            if status == "unknown" {
                tx.execute(
                    "INSERT INTO recycle_operation_recovery
                        (recycle_operation_id, batch_id, item_id, reason_code, detail, created_at)
                     VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
                    params![
                        operation_id,
                        batch_id,
                        item_id,
                        reason.unwrap_or_else(|| "unknown_shell_result".to_owned()),
                        now
                    ],
                )?;
            }
        }
        tx.execute(
            "UPDATE recycle_operation_batch SET status = 'reported', reported_at = ?1
             WHERE id = ?2 AND recycle_operation_id = ?3",
            params![now, batch_id, operation_id],
        )?;
        insert_report(
            &tx,
            operation_id,
            Some(batch_id),
            report_operation_id,
            "result",
            &payload_signature,
        )?;
        refresh_operation_terminal_state(&tx, operation_id, &now)?;
        tx.commit()?;
        Ok(RecycleOperationMutationResult {
            view: self.get_recycle_operation(operation_id)?,
            replayed: false,
        })
    }

    pub fn locked_recycle_operation_id(
        &self,
        run_id: i64,
    ) -> Result<Option<i64>, RecycleOperationError> {
        Ok(locked_operation_id(self.connection(), run_id)?)
    }
}

fn validate_operation_id(operation_id: &str) -> Result<(), RecycleOperationError> {
    if operation_id.is_empty() || operation_id.chars().count() > MAXIMUM_OPERATION_ID_CHARACTERS {
        return Err(RecycleOperationError::InvalidRequest {
            message: "operation IDs must contain 1 to 128 characters".to_owned(),
        });
    }
    Ok(())
}

fn locked_operation_id(
    connection: &rusqlite::Connection,
    run_id: i64,
) -> rusqlite::Result<Option<i64>> {
    connection
        .query_row(
            "SELECT id FROM recycle_operation
             WHERE run_id = ?1 AND status IN
                ('prepared', 'awaiting_confirmation', 'submitted', 'executing', 'cancelling', 'recovery_required')
             ORDER BY id DESC LIMIT 1",
            params![run_id],
            |row| row.get(0),
        )
        .optional()
}

fn operation_status(
    connection: &rusqlite::Connection,
    operation_id: i64,
) -> Result<String, RecycleOperationError> {
    connection
        .query_row(
            "SELECT status FROM recycle_operation WHERE id = ?1",
            params![operation_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(RecycleOperationError::NotFound { operation_id })
}

fn recycle_operation_by_id(
    connection: &rusqlite::Connection,
    operation_id: i64,
) -> rusqlite::Result<Option<RecycleOperation>> {
    connection
        .query_row(
            "SELECT operation.id, operation.operation_id, operation.run_id, operation.plan_id,
                    operation.preflight_id, operation.review_revision,
                    operation.preflight_snapshot_signature, operation.intent_signature,
                    operation.policy_version, operation.status, operation.logical_removal_count,
                    operation.shell_item_count, operation.physical_item_count,
                    operation.folder_item_count, operation.affected_group_count,
                    operation.planned_removal_bytes, operation.affected_location_count,
                    operation.exclusion_count, operation.prepared_at,
                    operation.confirmation_signature, operation.confirmation_expires_at,
                    operation.submitted_at, operation.completed_at,
                    operation.cancellation_requested, operation.error_code, operation.error_detail,
                    SUM(CASE WHEN item.eligibility_status = 'eligible' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN item.eligibility_status = 'non_recyclable' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN item.eligibility_status = 'pending' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN item.result_status = 'recycled' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN item.result_status = 'failed' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN item.result_status = 'cancelled' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN item.result_status = 'unknown' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN item.result_status = 'pending' THEN 1 ELSE 0 END)
             FROM recycle_operation operation
             LEFT JOIN recycle_operation_item item ON item.recycle_operation_id = operation.id
             WHERE operation.id = ?1 GROUP BY operation.id",
            params![operation_id],
            recycle_operation_from_row,
        )
        .optional()
}

fn recycle_operation_from_row(row: &Row<'_>) -> rusqlite::Result<RecycleOperation> {
    Ok(RecycleOperation {
        id: row.get(0)?,
        operation_id: row.get(1)?,
        run_id: row.get(2)?,
        plan_id: row.get(3)?,
        preflight_id: row.get(4)?,
        review_revision: row.get(5)?,
        preflight_snapshot_signature: row.get(6)?,
        intent_signature: row.get(7)?,
        policy_version: row.get(8)?,
        status: row.get(9)?,
        summary: RecycleOperationSummary {
            logical_removal_count: row.get(10)?,
            shell_item_count: row.get(11)?,
            physical_item_count: row.get(12)?,
            folder_item_count: row.get(13)?,
            affected_group_count: row.get(14)?,
            planned_removal_bytes: row.get(15)?,
            affected_location_count: row.get(16)?,
            exclusion_count: row.get(17)?,
            eligible_count: row.get::<_, Option<i64>>(26)?.unwrap_or(0),
            non_recyclable_count: row.get::<_, Option<i64>>(27)?.unwrap_or(0),
            pending_eligibility_count: row.get::<_, Option<i64>>(28)?.unwrap_or(0),
            recycled_count: row.get::<_, Option<i64>>(29)?.unwrap_or(0),
            failed_count: row.get::<_, Option<i64>>(30)?.unwrap_or(0),
            cancelled_count: row.get::<_, Option<i64>>(31)?.unwrap_or(0),
            unknown_count: row.get::<_, Option<i64>>(32)?.unwrap_or(0),
            pending_result_count: row.get::<_, Option<i64>>(33)?.unwrap_or(0),
        },
        prepared_at: row.get(18)?,
        confirmation_signature: row.get(19)?,
        confirmation_expires_at: row.get(20)?,
        submitted_at: row.get(21)?,
        completed_at: row.get(22)?,
        cancellation_requested: row.get(23)?,
        error_code: row.get(24)?,
        error_detail: row.get(25)?,
    })
}

fn recycle_item_from_row(row: &Row<'_>) -> rusqlite::Result<RecycleOperationItem> {
    Ok(RecycleOperationItem {
        id: row.get(0)?,
        recycle_operation_id: row.get(1)?,
        batch_id: row.get(2)?,
        ordinal: row.get(3)?,
        preflight_item_id: row.get(4)?,
        preflight_source_id: row.get(5)?,
        target_kind: row.get(6)?,
        physical_key: row.get(7)?,
        snapshot_path: row.get(8)?,
        group_id: row.get(9)?,
        folder_group_id: row.get(10)?,
        folder_member_id: row.get(11)?,
        snapshot_file_id: row.get(12)?,
        snapshot_directory_id: row.get(13)?,
        planned_bytes: row.get(14)?,
        eligibility_status: row.get(15)?,
        eligibility_code: row.get(16)?,
        result_status: row.get(17)?,
        result_code: row.get(18)?,
        shell_hresult: row.get(19)?,
        recycled_item_present: row.get(20)?,
        result_at: row.get(21)?,
    })
}

fn recycle_batch_by_id(
    connection: &rusqlite::Connection,
    operation_id: i64,
    batch_id: i64,
) -> Result<RecycleOperationBatch, RecycleOperationError> {
    let mut batch = connection
        .query_row(
            "SELECT id, recycle_operation_id, ordinal, item_signature, status,
                    admission_expires_at, shell_attempt_id, started_at, reported_at
             FROM recycle_operation_batch WHERE id = ?1 AND recycle_operation_id = ?2",
            params![batch_id, operation_id],
            |row| {
                Ok(RecycleOperationBatch {
                    id: row.get(0)?,
                    recycle_operation_id: row.get(1)?,
                    ordinal: row.get(2)?,
                    item_signature: row.get(3)?,
                    status: row.get(4)?,
                    admission_expires_at: row.get(5)?,
                    shell_attempt_id: row.get(6)?,
                    started_at: row.get(7)?,
                    reported_at: row.get(8)?,
                    items: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or(RecycleOperationError::BatchNotFound {
            operation_id,
            batch_id,
        })?;
    let mut statement = connection.prepare(
        "SELECT id, recycle_operation_id, batch_id, ordinal, preflight_item_id,
                preflight_source_id, target_kind, physical_key, snapshot_path, group_id,
                folder_group_id, folder_member_id, snapshot_file_id, snapshot_directory_id,
                planned_bytes, eligibility_status, eligibility_code, result_status,
                result_code, shell_hresult, recycled_item_present, result_at
         FROM recycle_operation_item WHERE batch_id = ?1 ORDER BY ordinal",
    )?;
    batch.items = statement
        .query_map(params![batch_id], recycle_item_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(batch)
}

fn report_replay(
    connection: &rusqlite::Connection,
    report_operation_id: &str,
    operation_id: i64,
    kind: &str,
    signature: &str,
) -> Result<bool, RecycleOperationError> {
    let existing = connection
        .query_row(
            "SELECT recycle_operation_id, report_kind, payload_signature
             FROM recycle_operation_report WHERE report_operation_id = ?1",
            params![report_operation_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    if let Some(existing) = existing {
        if existing.0 != operation_id || existing.1 != kind || existing.2 != signature {
            return Err(RecycleOperationError::IdempotencyConflict {
                operation_id: report_operation_id.to_owned(),
            });
        }
        return Ok(true);
    }
    Ok(false)
}

fn insert_report(
    connection: &rusqlite::Connection,
    operation_id: i64,
    batch_id: Option<i64>,
    report_operation_id: &str,
    kind: &str,
    signature: &str,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO recycle_operation_report
            (recycle_operation_id, batch_id, report_operation_id, report_kind,
             payload_signature, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            operation_id,
            batch_id,
            report_operation_id,
            kind,
            signature,
            Utc::now().to_rfc3339()
        ],
    )?;
    Ok(())
}

fn refresh_operation_terminal_state(
    connection: &rusqlite::Connection,
    operation_id: i64,
    now: &str,
) -> rusqlite::Result<()> {
    let (pending_batches, recycled, failed, cancelled, unknown, pending_results, cancellation_requested) = connection.query_row(
        "SELECT
             (SELECT COUNT(*) FROM recycle_operation_batch WHERE recycle_operation_id = ?1 AND status = 'pending'),
             SUM(CASE WHEN result_status = 'recycled' THEN 1 ELSE 0 END),
             SUM(CASE WHEN result_status = 'failed' THEN 1 ELSE 0 END),
             SUM(CASE WHEN result_status = 'cancelled' THEN 1 ELSE 0 END),
             SUM(CASE WHEN result_status = 'unknown' THEN 1 ELSE 0 END),
             SUM(CASE WHEN result_status = 'pending' THEN 1 ELSE 0 END),
             (SELECT cancellation_requested FROM recycle_operation WHERE id = ?1)
         FROM recycle_operation_item WHERE recycle_operation_id = ?1",
        params![operation_id],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?, row.get::<_, i64>(4)?, row.get::<_, i64>(5)?, row.get::<_, bool>(6)?)),
    )?;
    let status = if unknown > 0 {
        "recovery_required"
    } else if pending_batches > 0 && !cancellation_requested {
        "executing"
    } else if pending_results == 0 {
        if recycled > 0 && failed == 0 && cancelled == 0 {
            "completed"
        } else if recycled > 0 {
            "partially_completed"
        } else if cancellation_requested && failed == 0 {
            "cancelled"
        } else {
            "failed"
        }
    } else if cancellation_requested {
        "cancelling"
    } else {
        "executing"
    };
    if matches!(
        status,
        "completed" | "partially_completed" | "failed" | "cancelled" | "recovery_required"
    ) {
        if cancellation_requested {
            connection.execute(
                "UPDATE recycle_operation_batch SET status = 'skipped'
                 WHERE recycle_operation_id = ?1 AND status = 'pending'",
                params![operation_id],
            )?;
            connection.execute(
                "UPDATE recycle_operation_item SET result_status = 'cancelled',
                        result_code = 'cancelled_before_shell', result_at = ?1
                 WHERE recycle_operation_id = ?2 AND result_status = 'pending'",
                params![now, operation_id],
            )?;
        }
        connection.execute(
            "UPDATE recycle_operation SET status = ?1, completed_at = ?2 WHERE id = ?3",
            params![status, now, operation_id],
        )?;
    } else {
        connection.execute(
            "UPDATE recycle_operation SET status = ?1 WHERE id = ?2",
            params![status, operation_id],
        )?;
    }
    Ok(())
}

fn candidate_signature(
    run_id: i64,
    plan_id: i64,
    revision: i64,
    preflight_signature: &str,
    candidates: &[CandidateItem],
) -> String {
    let mut hasher = XxHash64::with_seed(0x72656379636c6531);
    hasher.write_i64(run_id);
    hasher.write_i64(plan_id);
    hasher.write_i64(revision);
    hasher.write(preflight_signature.as_bytes());
    for candidate in candidates {
        hasher.write(candidate.target_kind.as_bytes());
        hasher.write(candidate.physical_key.as_bytes());
        hasher.write(candidate.snapshot_path.as_bytes());
    }
    format!("{:016x}", hasher.finish())
}

fn batch_signature(candidates: &[CandidateItem]) -> String {
    let mut hasher = XxHash64::with_seed(0x626174636831);
    for candidate in candidates {
        hasher.write_i64(candidate.preflight_item_id);
        hasher.write(candidate.target_kind.as_bytes());
        hasher.write(candidate.snapshot_path.as_bytes());
    }
    format!("{:016x}", hasher.finish())
}

fn eligibility_signature(
    operation_id: i64,
    observations: &[RecycleEligibilityObservation],
) -> String {
    let mut entries = observations.to_vec();
    entries.sort_by_key(|value| value.item_id);
    let mut value = format!("eligibility|{operation_id}");
    for entry in entries {
        value.push_str(&format!(
            "|{}|{}|{}",
            entry.item_id,
            entry.status,
            entry.reason_code.unwrap_or_default()
        ));
    }
    text_signature(&value)
}

fn result_signature(
    operation_id: i64,
    batch_id: i64,
    observations: &[RecycleItemResultObservation],
) -> String {
    let mut entries = observations.to_vec();
    entries.sort_by_key(|value| value.item_id);
    let mut value = format!("result|{operation_id}|{batch_id}");
    for entry in entries {
        value.push_str(&format!(
            "|{}|{}|{}|{:?}|{:?}",
            entry.item_id,
            entry.status,
            entry.reason_code.unwrap_or_default(),
            entry.shell_hresult,
            entry.recycled_item_present
        ));
    }
    text_signature(&value)
}

fn confirmation_signature(intent_signature: &str, operation_id: i64) -> String {
    text_signature(&format!("confirmation|{operation_id}|{intent_signature}"))
}

fn text_signature(value: &str) -> String {
    let mut hasher = XxHash64::with_seed(0x6f7065726174696f);
    hasher.write(value.as_bytes());
    format!("{:016x}", hasher.finish())
}
