use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use thiserror::Error;

use super::live_validation::{
    count_state, normalize_path, path_is_within, validate_path, Observation, ValidationSnapshot,
};
use super::models::{
    CloudPolicy, ReviewDecisionKind, ReviewLiveRootOverflowRequest, ReviewLiveRootOverflowResult,
    ReviewLiveRootReconciliationRequest, ReviewLiveRootReconciliationResult,
    ReviewLiveRootReconciliationSummary, ReviewLiveRootState, ReviewLiveValidationItem,
    RunParameters,
};
use super::Database;

const MAXIMUM_RECONCILIATION_ITEMS: i64 = 200;

#[derive(Debug, Error)]
pub enum ReviewLiveRootError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("invalid dirty-root request: {message}")]
    InvalidRequest { message: String },
    #[error("scan run {run_id} was not found")]
    RunNotFound { run_id: i64 },
    #[error("scan run {run_id} is {status}; dirty-root operations require a completed run")]
    RunNotCompleted { run_id: i64, status: String },
    #[error("root {root_path} is not an immutable selected root for run {run_id}")]
    RootNotFound { run_id: i64, root_path: String },
    #[error("root {root_path} in run {run_id} is not dirty")]
    RootNotDirty { run_id: i64, root_path: String },
    #[error("dirty revision {expected} is stale; current revision is {actual}")]
    StaleDirtyRevision { expected: i64, actual: i64 },
    #[error("review revision {expected} is stale; current revision is {actual}")]
    StaleReviewRevision { expected: i64, actual: i64 },
    #[error("the dirty-root reconciliation cursor changed before this bounded request committed")]
    StaleReconciliationCursor,
    #[error("operation id {operation_id} was already used for a different dirty-root request")]
    IdempotencyConflict { operation_id: String },
    #[error("run {run_id} has an invalid parameter snapshot")]
    InvalidRunParameters { run_id: i64 },
}

impl Database {
    pub fn mark_review_root_overflow(
        &self,
        request: &ReviewLiveRootOverflowRequest,
    ) -> Result<ReviewLiveRootOverflowResult, ReviewLiveRootError> {
        validate_operation_id(&request.operation_id)?;
        validate_run_and_root(request.run_id, &request.root_path)?;
        let (_, parameters, _) = self.load_root_context(request.run_id)?;
        let root_path = owned_root(&parameters, &request.root_path).ok_or_else(|| {
            ReviewLiveRootError::RootNotFound {
                run_id: request.run_id,
                root_path: request.root_path.clone(),
            }
        })?;
        let signature = overflow_signature(request.run_id, &root_path);
        if let Some(stored_signature) = self
            .connection()
            .query_row(
                "SELECT request_signature FROM review_live_root_overflow WHERE operation_id = ?1",
                params![request.operation_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            if stored_signature != signature {
                return Err(ReviewLiveRootError::IdempotencyConflict {
                    operation_id: request.operation_id.clone(),
                });
            }
            let root = self
                .get_review_live_root(request.run_id, &root_path)?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            return Ok(ReviewLiveRootOverflowResult {
                root,
                replayed: true,
            });
        }

        let now = Utc::now().to_rfc3339();
        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        ensure_completed_root_context(&tx, request.run_id, &root_path, None)?;
        if let Some(stored_signature) = tx
            .query_row(
                "SELECT request_signature FROM review_live_root_overflow WHERE operation_id = ?1",
                params![request.operation_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            drop(tx);
            if stored_signature != signature {
                return Err(ReviewLiveRootError::IdempotencyConflict {
                    operation_id: request.operation_id.clone(),
                });
            }
            let root = self
                .get_review_live_root(request.run_id, &root_path)?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            return Ok(ReviewLiveRootOverflowResult {
                root,
                replayed: true,
            });
        }
        let dirty_revision = tx.query_row(
            "SELECT COALESCE(MAX(dirty_revision), 0) + 1
             FROM review_live_root_state WHERE run_id = ?1 AND root_path = ?2",
            params![request.run_id, root_path],
            |row| row.get::<_, i64>(0),
        )?;
        tx.execute(
            "INSERT INTO review_live_root_overflow
                (operation_id, request_signature, run_id, root_path, dirty_revision, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                request.operation_id,
                signature,
                request.run_id,
                root_path,
                dirty_revision,
                now,
            ],
        )?;
        tx.execute(
            "INSERT INTO review_live_root_state
                (run_id, root_path, state, dirty_revision, reason_code, dirty_at,
                 reconciliation_cursor_file_id, reconciled_item_count, updated_at)
             VALUES (?1, ?2, 'dirty', ?3, 'watcher_overflow', ?4, NULL, 0, ?4)
             ON CONFLICT(run_id, root_path) DO UPDATE SET
                 state = 'dirty', dirty_revision = excluded.dirty_revision,
                 reason_code = excluded.reason_code, dirty_at = excluded.dirty_at,
                 reconciliation_cursor_file_id = NULL, reconciled_item_count = 0,
                 updated_at = excluded.updated_at",
            params![request.run_id, root_path, dirty_revision, now],
        )?;
        tx.commit()?;
        Ok(ReviewLiveRootOverflowResult {
            root: self
                .get_review_live_root(request.run_id, &root_path)?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?,
            replayed: false,
        })
    }

    pub fn list_dirty_review_roots(
        &self,
        run_id: i64,
    ) -> Result<Vec<ReviewLiveRootState>, ReviewLiveRootError> {
        if run_id <= 0 {
            return Err(ReviewLiveRootError::InvalidRequest {
                message: "runId must be positive".to_owned(),
            });
        }
        self.load_root_context(run_id)?;
        let mut statement = self.connection().prepare(
            "SELECT run_id, root_path, state, dirty_revision, reason_code, dirty_at,
                    reconciliation_cursor_file_id, reconciled_item_count, updated_at
             FROM review_live_root_state
             WHERE run_id = ?1 AND state = 'dirty'
             ORDER BY root_path COLLATE UNICODE_NOCASE LIMIT 64",
        )?;
        let rows = statement.query_map(params![run_id], root_state_from_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn reconcile_review_root(
        &self,
        request: &ReviewLiveRootReconciliationRequest,
    ) -> Result<ReviewLiveRootReconciliationResult, ReviewLiveRootError> {
        self.reconcile_review_root_with(request, validate_path)
    }

    fn reconcile_review_root_with<F>(
        &self,
        request: &ReviewLiveRootReconciliationRequest,
        mut validator: F,
    ) -> Result<ReviewLiveRootReconciliationResult, ReviewLiveRootError>
    where
        F: FnMut(&ValidationSnapshot) -> Observation,
    {
        validate_reconciliation_request(request)?;
        let (_, parameters, current_review_revision) = self.load_root_context(request.run_id)?;
        let root_path = owned_root(&parameters, &request.root_path).ok_or_else(|| {
            ReviewLiveRootError::RootNotFound {
                run_id: request.run_id,
                root_path: request.root_path.clone(),
            }
        })?;
        if current_review_revision != request.expected_review_revision {
            return Err(ReviewLiveRootError::StaleReviewRevision {
                expected: request.expected_review_revision,
                actual: current_review_revision,
            });
        }
        let signature = reconciliation_signature(request, &root_path);
        if let Some(replayed) = self.load_root_reconciliation(&request.operation_id)? {
            if self.root_reconciliation_signature(replayed.reconciliation_id)? != signature {
                return Err(ReviewLiveRootError::IdempotencyConflict {
                    operation_id: request.operation_id.clone(),
                });
            }
            return Ok(ReviewLiveRootReconciliationResult {
                replayed: true,
                ..replayed
            });
        }
        let root = self
            .get_review_live_root(request.run_id, &root_path)?
            .ok_or_else(|| ReviewLiveRootError::RootNotDirty {
                run_id: request.run_id,
                root_path: root_path.clone(),
            })?;
        if root.state != "dirty" {
            return Err(ReviewLiveRootError::RootNotDirty {
                run_id: request.run_id,
                root_path,
            });
        }
        if root.dirty_revision != request.expected_dirty_revision {
            return Err(ReviewLiveRootError::StaleDirtyRevision {
                expected: request.expected_dirty_revision,
                actual: root.dirty_revision,
            });
        }
        let start_after_file_id = root.reconciliation_cursor_file_id;
        let snapshots = self.load_root_snapshots(
            request.run_id,
            &root.root_path,
            start_after_file_id,
            request.page_size,
        )?;
        let last_file_id = snapshots.last().map(|snapshot| snapshot.file_id);
        let has_more = match last_file_id {
            Some(last) => self.root_has_members_after(request.run_id, &root.root_path, last)?,
            None => false,
        };
        let exclusions = validation_exclusions(parameters);
        let observed_at = Utc::now().to_rfc3339();
        let observations = snapshots
            .iter()
            .map(|snapshot| {
                if exclusions
                    .iter()
                    .any(|excluded| path_is_within(&snapshot.path, excluded))
                {
                    Observation {
                        state: "unavailable",
                        reason_code: "excluded_location",
                        observed_identity: None,
                        observed_size: None,
                        observed_modified: None,
                        os_error: None,
                    }
                } else {
                    validator(snapshot)
                }
            })
            .collect::<Vec<_>>();
        let items = snapshots
            .iter()
            .zip(observations.iter())
            .map(|(snapshot, observation)| validation_item(snapshot, observation, &observed_at))
            .collect::<Vec<_>>();
        let summary = reconciliation_summary(&items);
        let next_cursor = has_more.then_some(last_file_id).flatten();
        let reconciled_item_count = root.reconciled_item_count + items.len() as i64;

        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM review_live_root_reconciliation WHERE operation_id = ?1)",
            params![request.operation_id],
            |row| row.get::<_, bool>(0),
        )? {
            drop(tx);
            let replayed = self
                .load_root_reconciliation(&request.operation_id)?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            if self.root_reconciliation_signature(replayed.reconciliation_id)? != signature {
                return Err(ReviewLiveRootError::IdempotencyConflict {
                    operation_id: request.operation_id.clone(),
                });
            }
            return Ok(ReviewLiveRootReconciliationResult {
                replayed: true,
                ..replayed
            });
        }
        ensure_completed_root_context(
            &tx,
            request.run_id,
            &root.root_path,
            Some(request.expected_review_revision),
        )?;
        let current_root =
            load_root_state_tx(&tx, request.run_id, &root.root_path)?.ok_or_else(|| {
                ReviewLiveRootError::RootNotDirty {
                    run_id: request.run_id,
                    root_path: root.root_path.clone(),
                }
            })?;
        if current_root.state != "dirty" {
            return Err(ReviewLiveRootError::RootNotDirty {
                run_id: request.run_id,
                root_path: root.root_path.clone(),
            });
        }
        if current_root.dirty_revision != request.expected_dirty_revision {
            return Err(ReviewLiveRootError::StaleDirtyRevision {
                expected: request.expected_dirty_revision,
                actual: current_root.dirty_revision,
            });
        }
        if current_root.reconciliation_cursor_file_id != start_after_file_id {
            return Err(ReviewLiveRootError::StaleReconciliationCursor);
        }
        tx.execute(
            "INSERT INTO review_live_root_reconciliation
                (operation_id, request_signature, run_id, root_path, dirty_revision,
                 expected_review_revision, start_after_file_id, item_count, present_count,
                 changed_count, missing_count, unavailable_count, invalidated_decision_count,
                 next_cursor_file_id, reconciliation_required, reconciled_item_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                request.operation_id,
                signature,
                request.run_id,
                root.root_path,
                request.expected_dirty_revision,
                request.expected_review_revision,
                start_after_file_id,
                summary.item_count,
                summary.present_count,
                summary.changed_count,
                summary.missing_count,
                summary.unavailable_count,
                summary.invalidated_decision_count,
                next_cursor,
                has_more,
                reconciled_item_count,
                observed_at,
            ],
        )?;
        let reconciliation_id = tx.last_insert_rowid();
        for (ordinal, item) in items.iter().enumerate() {
            insert_reconciliation_item(&tx, reconciliation_id, ordinal as i64, item)?;
            upsert_reconciled_live_state(&tx, reconciliation_id, request.run_id, item)?;
        }
        tx.execute(
            "UPDATE review_live_root_state
             SET state = ?1, reconciliation_cursor_file_id = ?2,
                 reconciled_item_count = ?3, updated_at = ?4
             WHERE run_id = ?5 AND root_path = ?6 AND dirty_revision = ?7",
            params![
                if has_more { "dirty" } else { "clean" },
                next_cursor,
                reconciled_item_count,
                observed_at,
                request.run_id,
                root.root_path,
                request.expected_dirty_revision,
            ],
        )?;
        tx.commit()?;
        Ok(ReviewLiveRootReconciliationResult {
            reconciliation_id,
            run_id: request.run_id,
            root_path: root.root_path.clone(),
            dirty_revision: request.expected_dirty_revision,
            review_revision: request.expected_review_revision,
            replayed: false,
            summary,
            items,
            root: self
                .get_review_live_root(request.run_id, &root.root_path)?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?,
        })
    }

    fn load_root_context(
        &self,
        run_id: i64,
    ) -> Result<(String, RunParameters, i64), ReviewLiveRootError> {
        let (status, parameters_json, review_revision) = self
            .connection()
            .query_row(
                "SELECT status, parameters_json,
                        COALESCE((SELECT revision FROM review_plan
                                  WHERE run_id = scan_run.id AND state = 'active'), 0)
                 FROM scan_run WHERE id = ?1",
                params![run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(ReviewLiveRootError::RunNotFound { run_id })?;
        if status != "completed" {
            return Err(ReviewLiveRootError::RunNotCompleted { run_id, status });
        }
        let parameters = RunParameters::from_json(&parameters_json)
            .ok_or(ReviewLiveRootError::InvalidRunParameters { run_id })?;
        Ok((status, parameters, review_revision))
    }

    fn get_review_live_root(
        &self,
        run_id: i64,
        root_path: &str,
    ) -> Result<Option<ReviewLiveRootState>, ReviewLiveRootError> {
        Ok(self
            .connection()
            .query_row(
                "SELECT run_id, root_path, state, dirty_revision, reason_code, dirty_at,
                        reconciliation_cursor_file_id, reconciled_item_count, updated_at
                 FROM review_live_root_state WHERE run_id = ?1 AND root_path = ?2",
                params![run_id, root_path],
                root_state_from_row,
            )
            .optional()?)
    }

    fn load_root_snapshots(
        &self,
        run_id: i64,
        root_path: &str,
        start_after_file_id: Option<i64>,
        page_size: i64,
    ) -> Result<Vec<ValidationSnapshot>, ReviewLiveRootError> {
        let mut statement = self.connection().prepare(
            "SELECT file.id, file.canonical_path, file.file_identity, file.file_size,
                    file.last_modified, recorded.decision, live.decision_invalidated,
                    live.invalidated_decision
             FROM scanned_file file
             JOIN duplicate_group_member member ON member.file_id = file.id
             JOIN duplicate_group duplicate_set
               ON duplicate_set.id = member.group_id AND duplicate_set.run_id = file.run_id
             LEFT JOIN review_plan plan ON plan.run_id = file.run_id AND plan.state = 'active'
             LEFT JOIN recorded_review_decision recorded
               ON recorded.plan_id = plan.id AND recorded.group_id = duplicate_set.id
              AND recorded.file_id = file.id
             LEFT JOIN review_live_file_state live
               ON live.run_id = file.run_id AND live.file_id = file.id
             WHERE file.run_id = ?1 AND file.root_path = ?2 COLLATE UNICODE_NOCASE
               AND file.id > COALESCE(?3, 0)
             ORDER BY file.id LIMIT ?4",
        )?;
        let rows = statement.query_map(
            params![run_id, root_path, start_after_file_id, page_size],
            |row| {
                Ok(ValidationSnapshot {
                    file_id: row.get(0)?,
                    path: row.get(1)?,
                    identity: row.get(2)?,
                    size: row.get(3)?,
                    modified: row.get(4)?,
                    recorded_decision: row
                        .get::<_, Option<String>>(5)?
                        .as_deref()
                        .and_then(ReviewDecisionKind::parse),
                    prior_invalidated: row.get::<_, Option<bool>>(6)?.unwrap_or(false),
                    prior_invalidated_decision: row
                        .get::<_, Option<String>>(7)?
                        .as_deref()
                        .and_then(ReviewDecisionKind::parse),
                })
            },
        )?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn root_has_members_after(
        &self,
        run_id: i64,
        root_path: &str,
        file_id: i64,
    ) -> Result<bool, ReviewLiveRootError> {
        Ok(self.connection().query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM scanned_file file
                 JOIN duplicate_group_member member ON member.file_id = file.id
                 JOIN duplicate_group duplicate_set
                   ON duplicate_set.id = member.group_id AND duplicate_set.run_id = file.run_id
                 WHERE file.run_id = ?1 AND file.root_path = ?2 COLLATE UNICODE_NOCASE
                   AND file.id > ?3)",
            params![run_id, root_path, file_id],
            |row| row.get::<_, bool>(0),
        )?)
    }

    fn load_root_reconciliation(
        &self,
        operation_id: &str,
    ) -> Result<Option<ReviewLiveRootReconciliationResult>, ReviewLiveRootError> {
        let header = self
            .connection()
            .query_row(
                "SELECT id, run_id, root_path, dirty_revision, expected_review_revision,
                        item_count, present_count, changed_count, missing_count, unavailable_count,
                        invalidated_decision_count
                 FROM review_live_root_reconciliation WHERE operation_id = ?1",
                params![operation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        ReviewLiveRootReconciliationSummary {
                            item_count: row.get(5)?,
                            present_count: row.get(6)?,
                            changed_count: row.get(7)?,
                            missing_count: row.get(8)?,
                            unavailable_count: row.get(9)?,
                            invalidated_decision_count: row.get(10)?,
                        },
                    ))
                },
            )
            .optional()?;
        let Some((id, run_id, root_path, dirty_revision, review_revision, summary)) = header else {
            return Ok(None);
        };
        let mut statement = self.connection().prepare(
            "SELECT file_id, state, reason_code, observed_file_identity, observed_file_size,
                    observed_last_modified, os_error, decision_invalidated,
                    invalidated_decision, observed_at
             FROM review_live_root_reconciliation_item
             WHERE reconciliation_id = ?1 ORDER BY ordinal",
        )?;
        let items = statement
            .query_map(params![id], validation_item_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        let root = self
            .get_review_live_root(run_id, &root_path)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        Ok(Some(ReviewLiveRootReconciliationResult {
            reconciliation_id: id,
            run_id,
            root_path,
            dirty_revision,
            review_revision,
            replayed: false,
            summary,
            items,
            root,
        }))
    }

    fn root_reconciliation_signature(
        &self,
        reconciliation_id: i64,
    ) -> Result<String, ReviewLiveRootError> {
        Ok(self.connection().query_row(
            "SELECT request_signature FROM review_live_root_reconciliation WHERE id = ?1",
            params![reconciliation_id],
            |row| row.get(0),
        )?)
    }
}

fn validate_operation_id(operation_id: &str) -> Result<(), ReviewLiveRootError> {
    if operation_id.trim().is_empty() || operation_id.chars().count() > 128 {
        return Err(ReviewLiveRootError::InvalidRequest {
            message: "operationId must contain 1..=128 characters".to_owned(),
        });
    }
    Ok(())
}

fn validate_run_and_root(run_id: i64, root_path: &str) -> Result<(), ReviewLiveRootError> {
    if run_id <= 0 || root_path.trim().is_empty() || root_path.chars().count() > 32_767 {
        return Err(ReviewLiveRootError::InvalidRequest {
            message: "runId must be positive and rootPath must contain 1..=32767 characters"
                .to_owned(),
        });
    }
    Ok(())
}

fn validate_reconciliation_request(
    request: &ReviewLiveRootReconciliationRequest,
) -> Result<(), ReviewLiveRootError> {
    validate_operation_id(&request.operation_id)?;
    validate_run_and_root(request.run_id, &request.root_path)?;
    if request.expected_dirty_revision <= 0
        || request.expected_review_revision < 0
        || !(1..=MAXIMUM_RECONCILIATION_ITEMS).contains(&request.page_size)
    {
        return Err(ReviewLiveRootError::InvalidRequest {
            message: "expectedDirtyRevision must be positive, expectedReviewRevision non-negative, and pageSize 1..=200".to_owned(),
        });
    }
    Ok(())
}

fn owned_root(parameters: &RunParameters, requested: &str) -> Option<String> {
    let requested = normalize_path(requested).trim_end_matches('/').to_owned();
    parameters.roots.iter().find_map(|root| {
        (normalize_path(root).trim_end_matches('/') == requested).then(|| root.clone())
    })
}

fn validation_exclusions(parameters: RunParameters) -> Vec<String> {
    let mut exclusions = parameters.manual_location_exclusions;
    if parameters.cloud_policy == CloudPolicy::ExcludeRegisteredRoots {
        exclusions.extend(
            parameters
                .registered_cloud_locations
                .into_iter()
                .map(|location| location.path),
        );
    }
    exclusions
}

fn ensure_completed_root_context(
    tx: &Transaction<'_>,
    run_id: i64,
    root_path: &str,
    expected_review_revision: Option<i64>,
) -> Result<(), ReviewLiveRootError> {
    let (status, parameters_json, review_revision) = tx
        .query_row(
            "SELECT status, parameters_json,
                    COALESCE((SELECT revision FROM review_plan
                              WHERE run_id = scan_run.id AND state = 'active'), 0)
             FROM scan_run WHERE id = ?1",
            params![run_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or(ReviewLiveRootError::RunNotFound { run_id })?;
    if status != "completed" {
        return Err(ReviewLiveRootError::RunNotCompleted { run_id, status });
    }
    let parameters = RunParameters::from_json(&parameters_json)
        .ok_or(ReviewLiveRootError::InvalidRunParameters { run_id })?;
    if owned_root(&parameters, root_path).is_none() {
        return Err(ReviewLiveRootError::RootNotFound {
            run_id,
            root_path: root_path.to_owned(),
        });
    }
    if let Some(expected) = expected_review_revision {
        if review_revision != expected {
            return Err(ReviewLiveRootError::StaleReviewRevision {
                expected,
                actual: review_revision,
            });
        }
    }
    Ok(())
}

fn load_root_state_tx(
    tx: &Transaction<'_>,
    run_id: i64,
    root_path: &str,
) -> Result<Option<ReviewLiveRootState>, ReviewLiveRootError> {
    Ok(tx
        .query_row(
            "SELECT run_id, root_path, state, dirty_revision, reason_code, dirty_at,
                    reconciliation_cursor_file_id, reconciled_item_count, updated_at
             FROM review_live_root_state WHERE run_id = ?1 AND root_path = ?2",
            params![run_id, root_path],
            root_state_from_row,
        )
        .optional()?)
}

fn root_state_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewLiveRootState> {
    Ok(ReviewLiveRootState {
        run_id: row.get(0)?,
        root_path: row.get(1)?,
        state: row.get(2)?,
        dirty_revision: row.get(3)?,
        reason_code: row.get(4)?,
        dirty_at: row.get(5)?,
        reconciliation_cursor_file_id: row.get(6)?,
        reconciled_item_count: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn validation_item(
    snapshot: &ValidationSnapshot,
    observation: &Observation,
    observed_at: &str,
) -> ReviewLiveValidationItem {
    let newly_invalidated = matches!(observation.state, "changed" | "missing")
        && matches!(
            snapshot.recorded_decision,
            Some(ReviewDecisionKind::Keep | ReviewDecisionKind::Remove)
        );
    ReviewLiveValidationItem {
        file_id: snapshot.file_id,
        state: observation.state.to_owned(),
        reason_code: observation.reason_code.to_owned(),
        observed_file_identity: observation.observed_identity.clone(),
        observed_file_size: observation.observed_size,
        observed_last_modified: observation.observed_modified,
        os_error: observation.os_error,
        decision_invalidated: snapshot.prior_invalidated || newly_invalidated,
        invalidated_decision: if newly_invalidated {
            snapshot.recorded_decision
        } else {
            snapshot.prior_invalidated_decision
        },
        observed_at: observed_at.to_owned(),
    }
}

fn reconciliation_summary(
    items: &[ReviewLiveValidationItem],
) -> ReviewLiveRootReconciliationSummary {
    ReviewLiveRootReconciliationSummary {
        item_count: items.len() as i64,
        present_count: count_state(items, "present"),
        changed_count: count_state(items, "changed"),
        missing_count: count_state(items, "missing"),
        unavailable_count: count_state(items, "unavailable"),
        invalidated_decision_count: items
            .iter()
            .filter(|item| item.decision_invalidated)
            .count() as i64,
    }
}

fn insert_reconciliation_item(
    tx: &Transaction<'_>,
    reconciliation_id: i64,
    ordinal: i64,
    item: &ReviewLiveValidationItem,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO review_live_root_reconciliation_item
            (reconciliation_id, ordinal, file_id, state, reason_code,
             observed_file_identity, observed_file_size, observed_last_modified, os_error,
             decision_invalidated, invalidated_decision, observed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            reconciliation_id,
            ordinal,
            item.file_id,
            item.state,
            item.reason_code,
            item.observed_file_identity,
            item.observed_file_size,
            item.observed_last_modified,
            item.os_error,
            item.decision_invalidated,
            item.invalidated_decision.map(ReviewDecisionKind::as_str),
            item.observed_at,
        ],
    )?;
    Ok(())
}

fn upsert_reconciled_live_state(
    tx: &Transaction<'_>,
    reconciliation_id: i64,
    run_id: i64,
    item: &ReviewLiveValidationItem,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO review_live_file_state
            (run_id, file_id, validation_id, reconciliation_id, state, reason_code,
             observed_file_identity, observed_file_size, observed_last_modified, os_error,
             decision_invalidated, invalidated_decision, observed_at)
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(run_id, file_id) DO UPDATE SET
             validation_id = NULL, reconciliation_id = excluded.reconciliation_id,
             state = excluded.state, reason_code = excluded.reason_code,
             observed_file_identity = excluded.observed_file_identity,
             observed_file_size = excluded.observed_file_size,
             observed_last_modified = excluded.observed_last_modified,
             os_error = excluded.os_error,
             decision_invalidated = excluded.decision_invalidated,
             invalidated_decision = excluded.invalidated_decision,
             observed_at = excluded.observed_at",
        params![
            run_id,
            item.file_id,
            reconciliation_id,
            item.state,
            item.reason_code,
            item.observed_file_identity,
            item.observed_file_size,
            item.observed_last_modified,
            item.os_error,
            item.decision_invalidated,
            item.invalidated_decision.map(ReviewDecisionKind::as_str),
            item.observed_at,
        ],
    )?;
    Ok(())
}

fn validation_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewLiveValidationItem> {
    Ok(ReviewLiveValidationItem {
        file_id: row.get(0)?,
        state: row.get(1)?,
        reason_code: row.get(2)?,
        observed_file_identity: row.get(3)?,
        observed_file_size: row.get(4)?,
        observed_last_modified: row.get(5)?,
        os_error: row.get(6)?,
        decision_invalidated: row.get(7)?,
        invalidated_decision: row
            .get::<_, Option<String>>(8)?
            .as_deref()
            .and_then(ReviewDecisionKind::parse),
        observed_at: row.get(9)?,
    })
}

fn overflow_signature(run_id: i64, root_path: &str) -> String {
    format!(
        "review-live-root-overflow-v1|{run_id}|{}",
        normalize_path(root_path)
    )
}

fn reconciliation_signature(
    request: &ReviewLiveRootReconciliationRequest,
    root_path: &str,
) -> String {
    format!(
        "review-live-root-reconcile-v1|{}|{}|{}|{}|{}",
        request.run_id,
        normalize_path(root_path),
        request.expected_dirty_revision,
        request.expected_review_revision,
        request.page_size
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform;
    use crate::storage::models::{CloudDetectionStatus, ScannedFile};
    use std::fs;
    use std::time::UNIX_EPOCH;
    use tempfile::TempDir;

    #[test]
    fn overflow_is_durable_and_reconciliation_advances_only_one_bounded_batch() {
        let fixture = Fixture::new();
        fixture
            .db
            .set_review_decision(
                "keep-dirty",
                fixture.run_id,
                fixture.group_id,
                fixture.ids[0],
                ReviewDecisionKind::Keep,
                0,
            )
            .unwrap();
        fixture
            .db
            .set_review_decision(
                "remove-dirty",
                fixture.run_id,
                fixture.group_id,
                fixture.ids[1],
                ReviewDecisionKind::Remove,
                1,
            )
            .unwrap();
        let immutable_before = immutable_rows(&fixture.db, fixture.run_id);
        fs::remove_file(&fixture.paths[0]).unwrap();
        fs::write(&fixture.paths[1], b"changed size for dirty reconciliation").unwrap();

        let overflow = ReviewLiveRootOverflowRequest {
            operation_id: "overflow-1".to_owned(),
            run_id: fixture.run_id,
            root_path: fixture.root.clone(),
        };
        let dirty = fixture.db.mark_review_root_overflow(&overflow).unwrap();
        assert!(!dirty.replayed);
        assert_eq!(dirty.root.state, "dirty");
        assert_eq!(dirty.root.dirty_revision, 1);
        assert_eq!(
            fixture
                .db
                .list_dirty_review_roots(fixture.run_id)
                .unwrap()
                .len(),
            1
        );
        let replay = fixture.db.mark_review_root_overflow(&overflow).unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.root.dirty_revision, 1);

        let first = fixture
            .db
            .reconcile_review_root(&ReviewLiveRootReconciliationRequest {
                operation_id: "reconcile-1".to_owned(),
                run_id: fixture.run_id,
                root_path: fixture.root.clone(),
                expected_dirty_revision: 1,
                expected_review_revision: 2,
                page_size: 2,
            })
            .unwrap();
        assert_eq!(first.summary.item_count, 2);
        assert_eq!(first.summary.missing_count, 1);
        assert_eq!(first.summary.changed_count, 1);
        assert_eq!(first.summary.invalidated_decision_count, 2);
        assert_eq!(first.root.state, "dirty");
        assert_eq!(first.root.reconciled_item_count, 2);
        assert!(first.root.reconciliation_cursor_file_id.is_some());
        assert_eq!(
            immutable_rows(&fixture.db, fixture.run_id),
            immutable_before
        );
        assert_eq!(count_rows(&fixture.db, "review_decision"), 2);
        assert_eq!(count_rows(&fixture.db, "recorded_review_decision"), 2);
        assert_eq!(count_rows(&fixture.db, "effective_review_decision"), 0);

        let second_request = ReviewLiveRootReconciliationRequest {
            operation_id: "reconcile-2".to_owned(),
            run_id: fixture.run_id,
            root_path: fixture.root.clone(),
            expected_dirty_revision: 1,
            expected_review_revision: 2,
            page_size: 2,
        };
        let second = fixture.db.reconcile_review_root(&second_request).unwrap();
        assert_eq!(second.summary.item_count, 1);
        assert_eq!(second.summary.present_count, 1);
        assert_eq!(second.root.state, "clean");
        assert_eq!(second.root.reconciled_item_count, 3);
        assert!(fixture
            .db
            .list_dirty_review_roots(fixture.run_id)
            .unwrap()
            .is_empty());
        let second_replay = fixture.db.reconcile_review_root(&second_request).unwrap();
        assert!(second_replay.replayed);
        assert_eq!(second_replay.reconciliation_id, second.reconciliation_id);

        drop(fixture.db);
        let reopened = Database::open(fixture.database_path.to_str().unwrap()).unwrap();
        assert!(reopened
            .list_dirty_review_roots(fixture.run_id)
            .unwrap()
            .is_empty());
        assert_eq!(count_rows(&reopened, "review_live_file_state"), 3);
        assert_eq!(immutable_rows(&reopened, fixture.run_id), immutable_before);
        let next = reopened
            .mark_review_root_overflow(&ReviewLiveRootOverflowRequest {
                operation_id: "overflow-2".to_owned(),
                run_id: fixture.run_id,
                root_path: fixture.root.clone(),
            })
            .unwrap();
        assert_eq!(next.root.dirty_revision, 2);
        let stale = reopened.reconcile_review_root(&ReviewLiveRootReconciliationRequest {
            operation_id: "stale-root".to_owned(),
            expected_dirty_revision: 1,
            ..second_request
        });
        assert!(matches!(
            stale,
            Err(ReviewLiveRootError::StaleDirtyRevision { actual: 2, .. })
        ));
        assert_eq!(count_rows(&reopened, "review_live_root_reconciliation"), 2);
    }

    struct Fixture {
        _temp: TempDir,
        database_path: std::path::PathBuf,
        db: Database,
        run_id: i64,
        group_id: i64,
        root: String,
        ids: Vec<i64>,
        paths: Vec<std::path::PathBuf>,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();
            let database_path = temp.path().join("dirty-root.db");
            let paths = (0..3)
                .map(|index| temp.path().join(format!("copy-{index}.bin")))
                .collect::<Vec<_>>();
            for path in &paths {
                fs::write(path, b"same dirty root bytes").unwrap();
            }
            let db = Database::open(database_path.to_str().unwrap()).unwrap();
            let root = temp.path().to_string_lossy().into_owned();
            let session_id = db
                .create_session("dirty root", std::slice::from_ref(&root), &[])
                .unwrap();
            let run_id = db
                .create_scan_run(
                    session_id,
                    &RunParameters {
                        roots: vec![root.clone()],
                        ignore_patterns: Vec::new(),
                        directory_similarity_threshold_millis: 500,
                        cloud_policy: CloudPolicy::ExcludeRegisteredRoots,
                        manual_location_exclusions: Vec::new(),
                        registered_cloud_locations: Vec::new(),
                        cloud_detection_status: CloudDetectionStatus::Complete,
                    },
                    "test",
                )
                .unwrap();
            db.start_scan_run(run_id).unwrap();
            let files = paths
                .iter()
                .map(|path| {
                    let metadata = fs::metadata(path).unwrap();
                    ScannedFile {
                        id: 0,
                        run_id,
                        root_path: root.clone(),
                        canonical_path: path.to_string_lossy().into_owned(),
                        relative_path: path.file_name().unwrap().to_string_lossy().into_owned(),
                        file_name: path.file_name().unwrap().to_string_lossy().into_owned(),
                        parent_dir: root.clone(),
                        drive_letter: String::new(),
                        file_size: metadata.len() as i64,
                        last_modified: metadata
                            .modified()
                            .unwrap()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_nanos()
                            .min(i64::MAX as u128) as i64,
                        partial_hash: None,
                        content_hash: Some(99),
                        file_identity: platform::file_identity(path).unwrap(),
                        warning_message: None,
                        marked_deleted: false,
                    }
                })
                .collect::<Vec<_>>();
            db.insert_scanned_files(&files).unwrap();
            db.insert_duplicate_groups(
                run_id,
                &[(
                    99,
                    files[0].file_size,
                    files
                        .iter()
                        .map(|file| file.canonical_path.clone())
                        .collect(),
                )],
            )
            .unwrap();
            db.complete_scan_run(
                run_id,
                3,
                files[0].file_size * 3,
                3,
                1,
                0,
                files[0].file_size * 2,
                0,
            )
            .unwrap();
            let group_id = db
                .connection()
                .query_row(
                    "SELECT id FROM duplicate_group WHERE run_id = ?1",
                    params![run_id],
                    |row| row.get(0),
                )
                .unwrap();
            let ids = db
                .connection()
                .prepare("SELECT id FROM scanned_file WHERE run_id = ?1 ORDER BY id")
                .unwrap()
                .query_map(params![run_id], |row| row.get::<_, i64>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            Self {
                _temp: temp,
                database_path,
                db,
                run_id,
                group_id,
                root,
                ids,
                paths,
            }
        }
    }

    fn count_rows(db: &Database, table: &str) -> i64 {
        db.connection()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn immutable_rows(db: &Database, run_id: i64) -> Vec<(i64, String, i64, i64)> {
        db.connection()
            .prepare(
                "SELECT id, canonical_path, file_size, last_modified
                 FROM scanned_file WHERE run_id = ?1 ORDER BY id",
            )
            .unwrap()
            .query_map(params![run_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }
}
