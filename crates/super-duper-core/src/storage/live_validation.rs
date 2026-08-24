use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::time::UNIX_EPOCH;

use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use thiserror::Error;

use crate::platform::{self, PathSafety};

use super::models::{
    CloudPolicy, ReviewDecisionKind, ReviewLiveValidationItem, ReviewLiveValidationRequest,
    ReviewLiveValidationResult, RunParameters,
};
use super::Database;

const MAXIMUM_VALIDATION_ITEMS: usize = 200;

#[derive(Debug, Error)]
pub enum ReviewLiveValidationError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("invalid live validation request: {message}")]
    InvalidRequest { message: String },
    #[error("scan run {run_id} was not found")]
    RunNotFound { run_id: i64 },
    #[error("scan run {run_id} is {status}; validation requires a completed run")]
    RunNotCompleted { run_id: i64, status: String },
    #[error("duplicate group {group_id} was not found in run {run_id}")]
    GroupNotFound { run_id: i64, group_id: i64 },
    #[error("file {file_id} is not a member of duplicate group {group_id} in run {run_id}")]
    MemberNotFound {
        run_id: i64,
        group_id: i64,
        file_id: i64,
    },
    #[error("review revision {expected} is stale; current revision is {actual}")]
    StaleRevision { expected: i64, actual: i64 },
    #[error("operation id {operation_id} was already used for a different validation request")]
    IdempotencyConflict { operation_id: String },
    #[error("run {run_id} has an invalid parameter snapshot")]
    InvalidRunParameters { run_id: i64 },
}

#[derive(Clone)]
pub(super) struct ValidationSnapshot {
    pub(super) file_id: i64,
    pub(super) path: String,
    pub(super) identity: Option<String>,
    pub(super) size: i64,
    pub(super) modified: i64,
    pub(super) recorded_decision: Option<ReviewDecisionKind>,
    pub(super) prior_invalidated: bool,
    pub(super) prior_invalidated_decision: Option<ReviewDecisionKind>,
}

#[derive(Clone)]
pub(super) struct Observation {
    pub(super) state: &'static str,
    pub(super) reason_code: &'static str,
    pub(super) observed_identity: Option<String>,
    pub(super) observed_size: Option<i64>,
    pub(super) observed_modified: Option<i64>,
    pub(super) os_error: Option<i64>,
}

impl Database {
    pub fn validate_review_files(
        &self,
        request: &ReviewLiveValidationRequest,
    ) -> Result<ReviewLiveValidationResult, ReviewLiveValidationError> {
        self.validate_review_files_with(request, validate_path)
    }

    fn validate_review_files_with<F>(
        &self,
        request: &ReviewLiveValidationRequest,
        mut validator: F,
    ) -> Result<ReviewLiveValidationResult, ReviewLiveValidationError>
    where
        F: FnMut(&ValidationSnapshot) -> Observation,
    {
        validate_request(request)?;
        let signature = request_signature(request);
        if let Some(replayed) = self.load_validation_by_operation(&request.operation_id)? {
            if replayed.run_id != request.run_id
                || replayed.group_id != request.group_id
                || replayed.review_revision != request.expected_review_revision
                || replayed.scope != request.scope
                || self.validation_signature(replayed.validation_id)? != signature
            {
                return Err(ReviewLiveValidationError::IdempotencyConflict {
                    operation_id: request.operation_id.clone(),
                });
            }
            return Ok(ReviewLiveValidationResult {
                replayed: true,
                ..replayed
            });
        }

        let (snapshots, exclusions) = self.load_validation_snapshots(request)?;
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

        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM review_live_validation WHERE operation_id = ?1)",
            params![request.operation_id],
            |row| row.get::<_, bool>(0),
        )? {
            drop(tx);
            let replayed = self
                .load_validation_by_operation(&request.operation_id)?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            if self.validation_signature(replayed.validation_id)? != signature {
                return Err(ReviewLiveValidationError::IdempotencyConflict {
                    operation_id: request.operation_id.clone(),
                });
            }
            return Ok(ReviewLiveValidationResult {
                replayed: true,
                ..replayed
            });
        }
        ensure_current_context(&tx, request)?;

        let mut items = Vec::with_capacity(snapshots.len());
        for (snapshot, observation) in snapshots.iter().zip(observations.iter()) {
            let newly_invalidated = matches!(observation.state, "changed" | "missing")
                && matches!(
                    snapshot.recorded_decision,
                    Some(ReviewDecisionKind::Keep | ReviewDecisionKind::Remove)
                );
            let decision_invalidated = snapshot.prior_invalidated || newly_invalidated;
            let invalidated_decision = if newly_invalidated {
                snapshot.recorded_decision
            } else {
                snapshot.prior_invalidated_decision
            };
            items.push(ReviewLiveValidationItem {
                file_id: snapshot.file_id,
                state: observation.state.to_owned(),
                reason_code: observation.reason_code.to_owned(),
                observed_file_identity: observation.observed_identity.clone(),
                observed_file_size: observation.observed_size,
                observed_last_modified: observation.observed_modified,
                os_error: observation.os_error,
                decision_invalidated,
                invalidated_decision,
                observed_at: observed_at.clone(),
            });
        }
        let present_count = count_state(&items, "present");
        let changed_count = count_state(&items, "changed");
        let missing_count = count_state(&items, "missing");
        let unavailable_count = count_state(&items, "unavailable");
        let invalidated_count = items
            .iter()
            .filter(|item| item.decision_invalidated)
            .count() as i64;
        tx.execute(
            "INSERT INTO review_live_validation
                (operation_id, request_signature, run_id, group_id, expected_review_revision,
                 scope, item_count, present_count, changed_count, missing_count,
                 unavailable_count, invalidated_decision_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                request.operation_id,
                signature,
                request.run_id,
                request.group_id,
                request.expected_review_revision,
                request.scope,
                items.len() as i64,
                present_count,
                changed_count,
                missing_count,
                unavailable_count,
                invalidated_count,
                observed_at,
            ],
        )?;
        let validation_id = tx.last_insert_rowid();
        for (ordinal, item) in items.iter().enumerate() {
            tx.execute(
                "INSERT INTO review_live_validation_item
                    (validation_id, ordinal, file_id, state, reason_code,
                     observed_file_identity, observed_file_size, observed_last_modified, os_error,
                     decision_invalidated, invalidated_decision, observed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    validation_id,
                    ordinal as i64,
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
            tx.execute(
                "INSERT INTO review_live_file_state
                    (run_id, file_id, validation_id, reconciliation_id, state, reason_code,
                     observed_file_identity, observed_file_size, observed_last_modified, os_error,
                     decision_invalidated, invalidated_decision, observed_at)
                 VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(run_id, file_id) DO UPDATE SET
                     validation_id = excluded.validation_id,
                     reconciliation_id = NULL,
                     state = excluded.state,
                     reason_code = excluded.reason_code,
                     observed_file_identity = excluded.observed_file_identity,
                     observed_file_size = excluded.observed_file_size,
                     observed_last_modified = excluded.observed_last_modified,
                     os_error = excluded.os_error,
                     decision_invalidated = excluded.decision_invalidated,
                     invalidated_decision = excluded.invalidated_decision,
                     observed_at = excluded.observed_at",
                params![
                    request.run_id,
                    item.file_id,
                    validation_id,
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
        }
        tx.commit()?;
        Ok(ReviewLiveValidationResult {
            validation_id,
            run_id: request.run_id,
            group_id: request.group_id,
            review_revision: request.expected_review_revision,
            scope: request.scope.clone(),
            replayed: false,
            items,
        })
    }

    fn load_validation_snapshots(
        &self,
        request: &ReviewLiveValidationRequest,
    ) -> Result<(Vec<ValidationSnapshot>, Vec<String>), ReviewLiveValidationError> {
        let (status, parameters_json) = self
            .connection()
            .query_row(
                "SELECT status, parameters_json FROM scan_run WHERE id = ?1",
                params![request.run_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(ReviewLiveValidationError::RunNotFound {
                run_id: request.run_id,
            })?;
        if status != "completed" {
            return Err(ReviewLiveValidationError::RunNotCompleted {
                run_id: request.run_id,
                status,
            });
        }
        if !self.connection().query_row(
            "SELECT EXISTS(SELECT 1 FROM duplicate_group WHERE id = ?1 AND run_id = ?2)",
            params![request.group_id, request.run_id],
            |row| row.get::<_, bool>(0),
        )? {
            return Err(ReviewLiveValidationError::GroupNotFound {
                run_id: request.run_id,
                group_id: request.group_id,
            });
        }
        let current_revision = self.connection().query_row(
            "SELECT COALESCE((SELECT revision FROM review_plan
                              WHERE run_id = ?1 AND state = 'active'), 0)",
            params![request.run_id],
            |row| row.get::<_, i64>(0),
        )?;
        if current_revision != request.expected_review_revision {
            return Err(ReviewLiveValidationError::StaleRevision {
                expected: request.expected_review_revision,
                actual: current_revision,
            });
        }
        let mut statement = self.connection().prepare(
            "SELECT file.canonical_path, file.file_identity, file.file_size, file.last_modified,
                    recorded.decision, live.decision_invalidated, live.invalidated_decision
             FROM duplicate_group_member member
             JOIN duplicate_group duplicate_set ON duplicate_set.id = member.group_id
             JOIN scanned_file file ON file.id = member.file_id AND file.run_id = duplicate_set.run_id
             LEFT JOIN review_plan plan ON plan.run_id = duplicate_set.run_id AND plan.state = 'active'
             LEFT JOIN recorded_review_decision recorded
               ON recorded.plan_id = plan.id AND recorded.group_id = duplicate_set.id
              AND recorded.file_id = file.id
             LEFT JOIN review_live_file_state live
               ON live.run_id = duplicate_set.run_id AND live.file_id = file.id
             WHERE duplicate_set.id = ?1 AND duplicate_set.run_id = ?2 AND file.id = ?3",
        )?;
        let mut snapshots = Vec::with_capacity(request.file_ids.len());
        for file_id in &request.file_ids {
            let snapshot = statement
                .query_row(params![request.group_id, request.run_id, file_id], |row| {
                    Ok(ValidationSnapshot {
                        file_id: *file_id,
                        path: row.get(0)?,
                        identity: row.get(1)?,
                        size: row.get(2)?,
                        modified: row.get(3)?,
                        recorded_decision: row
                            .get::<_, Option<String>>(4)?
                            .as_deref()
                            .and_then(ReviewDecisionKind::parse),
                        prior_invalidated: row.get::<_, Option<bool>>(5)?.unwrap_or(false),
                        prior_invalidated_decision: row
                            .get::<_, Option<String>>(6)?
                            .as_deref()
                            .and_then(ReviewDecisionKind::parse),
                    })
                })
                .optional()?
                .ok_or(ReviewLiveValidationError::MemberNotFound {
                    run_id: request.run_id,
                    group_id: request.group_id,
                    file_id: *file_id,
                })?;
            snapshots.push(snapshot);
        }
        let parameters = RunParameters::from_json(&parameters_json).ok_or(
            ReviewLiveValidationError::InvalidRunParameters {
                run_id: request.run_id,
            },
        )?;
        let mut exclusions = parameters.manual_location_exclusions;
        if parameters.cloud_policy == CloudPolicy::ExcludeRegisteredRoots {
            exclusions.extend(
                parameters
                    .registered_cloud_locations
                    .into_iter()
                    .map(|location| location.path),
            );
        }
        Ok((snapshots, exclusions))
    }

    fn load_validation_by_operation(
        &self,
        operation_id: &str,
    ) -> Result<Option<ReviewLiveValidationResult>, ReviewLiveValidationError> {
        let header = self
            .connection()
            .query_row(
                "SELECT id, run_id, group_id, expected_review_revision, scope
                 FROM review_live_validation WHERE operation_id = ?1",
                params![operation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((validation_id, run_id, group_id, review_revision, scope)) = header else {
            return Ok(None);
        };
        let mut statement = self.connection().prepare(
            "SELECT file_id, state, reason_code, observed_file_identity, observed_file_size,
                    observed_last_modified, os_error, decision_invalidated,
                    invalidated_decision, observed_at
             FROM review_live_validation_item WHERE validation_id = ?1 ORDER BY ordinal",
        )?;
        let rows = statement.query_map(params![validation_id], |row| {
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
        })?;
        Ok(Some(ReviewLiveValidationResult {
            validation_id,
            run_id,
            group_id,
            review_revision,
            scope,
            replayed: false,
            items: rows.collect::<Result<Vec<_>, _>>()?,
        }))
    }

    fn validation_signature(&self, validation_id: i64) -> rusqlite::Result<String> {
        self.connection().query_row(
            "SELECT request_signature FROM review_live_validation WHERE id = ?1",
            params![validation_id],
            |row| row.get(0),
        )
    }
}

fn validate_request(
    request: &ReviewLiveValidationRequest,
) -> Result<(), ReviewLiveValidationError> {
    if request.operation_id.trim().is_empty()
        || request.operation_id.chars().count() > 128
        || request.run_id <= 0
        || request.group_id <= 0
        || request.expected_review_revision < 0
        || !matches!(request.scope.as_str(), "selection" | "visible_page")
        || request.file_ids.is_empty()
        || request.file_ids.len() > MAXIMUM_VALIDATION_ITEMS
        || request.file_ids.iter().any(|id| *id <= 0)
        || request
            .file_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len()
            != request.file_ids.len()
    {
        return Err(ReviewLiveValidationError::InvalidRequest {
            message: "operationId must contain 1..=128 characters; runId/groupId/fileIds must be positive; expectedReviewRevision must be non-negative; scope must be selection or visible_page; and fileIds must contain 1..=200 distinct items".to_owned(),
        });
    }
    Ok(())
}

fn ensure_current_context(
    tx: &Transaction<'_>,
    request: &ReviewLiveValidationRequest,
) -> Result<(), ReviewLiveValidationError> {
    let (status, current_revision) = tx
        .query_row(
            "SELECT run.status,
                    COALESCE((SELECT revision FROM review_plan
                              WHERE run_id = run.id AND state = 'active'), 0)
             FROM scan_run run WHERE run.id = ?1",
            params![request.run_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
        .ok_or(ReviewLiveValidationError::RunNotFound {
            run_id: request.run_id,
        })?;
    if status != "completed" {
        return Err(ReviewLiveValidationError::RunNotCompleted {
            run_id: request.run_id,
            status,
        });
    }
    if current_revision != request.expected_review_revision {
        return Err(ReviewLiveValidationError::StaleRevision {
            expected: request.expected_review_revision,
            actual: current_revision,
        });
    }
    let group_exists = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM duplicate_group WHERE id = ?1 AND run_id = ?2)",
        params![request.group_id, request.run_id],
        |row| row.get::<_, bool>(0),
    )?;
    if !group_exists {
        return Err(ReviewLiveValidationError::GroupNotFound {
            run_id: request.run_id,
            group_id: request.group_id,
        });
    }
    for file_id in &request.file_ids {
        let member_exists = tx.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM duplicate_group_member member
                 JOIN duplicate_group duplicate_set ON duplicate_set.id = member.group_id
                 JOIN scanned_file file ON file.id = member.file_id
                 WHERE duplicate_set.id = ?1 AND duplicate_set.run_id = ?2
                   AND file.run_id = duplicate_set.run_id AND file.id = ?3
             )",
            params![request.group_id, request.run_id, file_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !member_exists {
            return Err(ReviewLiveValidationError::MemberNotFound {
                run_id: request.run_id,
                group_id: request.group_id,
                file_id: *file_id,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_path(snapshot: &ValidationSnapshot) -> Observation {
    let path = Path::new(&snapshot.path);
    match platform::classify_path_without_open(path) {
        Ok(PathSafety::Missing) => observation("missing", "path_missing"),
        Ok(PathSafety::CloudPlaceholder) => observation("unavailable", "cloud_placeholder"),
        Ok(PathSafety::ReparsePoint) => observation("unavailable", "reparse_point"),
        Ok(PathSafety::Directory) => observation("changed", "wrong_type_directory"),
        Ok(PathSafety::Other) => observation("changed", "wrong_type_other"),
        Err(error) => unavailable("attributes_unavailable", &error),
        Ok(PathSafety::File) => match file_observation(path, snapshot) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                observation("missing", "path_missing")
            }
            Err(error) => unavailable("metadata_unavailable", &error),
        },
    }
}

fn file_observation(path: &Path, snapshot: &ValidationSnapshot) -> io::Result<Observation> {
    let metadata = fs::metadata(path)?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let size = metadata.len().min(i64::MAX as u64) as i64;
    let identity = platform::file_identity(path)?;
    let mut value = Observation {
        state: "present",
        reason_code: "matched_snapshot",
        observed_identity: identity.clone(),
        observed_size: Some(size),
        observed_modified: Some(modified),
        os_error: None,
    };
    if snapshot
        .identity
        .as_ref()
        .is_some_and(|expected| identity.as_ref() != Some(expected))
    {
        value.state = "changed";
        value.reason_code = "identity_changed";
    } else if size != snapshot.size {
        value.state = "changed";
        value.reason_code = "size_changed";
    } else if modified != snapshot.modified {
        value.state = "changed";
        value.reason_code = "timestamp_changed";
    }
    Ok(value)
}

fn observation(state: &'static str, reason_code: &'static str) -> Observation {
    Observation {
        state,
        reason_code,
        observed_identity: None,
        observed_size: None,
        observed_modified: None,
        os_error: None,
    }
}

fn unavailable(reason_code: &'static str, error: &io::Error) -> Observation {
    Observation {
        os_error: error.raw_os_error().map(i64::from),
        ..observation("unavailable", reason_code)
    }
}

pub(super) fn count_state(items: &[ReviewLiveValidationItem], state: &str) -> i64 {
    items.iter().filter(|item| item.state == state).count() as i64
}

fn request_signature(request: &ReviewLiveValidationRequest) -> String {
    format!(
        "review-live-v1|{}|{}|{}|{}|{}",
        request.run_id,
        request.group_id,
        request.expected_review_revision,
        request.scope,
        request
            .file_ids
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(super) fn path_is_within(path: &str, root: &str) -> bool {
    let path = normalize_path(path);
    let root = normalize_path(root).trim_end_matches('/').to_owned();
    path == root
        || path
            .strip_prefix(&root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

pub(super) fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{CloudDetectionStatus, ScannedFile};
    use rusqlite::params;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    #[test]
    fn bounded_external_deletion_and_modification_invalidate_working_decisions_only() {
        let fixture = Fixture::new(Vec::new());
        fixture
            .db
            .set_review_decision(
                "keep",
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
                "remove",
                fixture.run_id,
                fixture.group_id,
                fixture.ids[1],
                ReviewDecisionKind::Remove,
                1,
            )
            .unwrap();
        let immutable_before = immutable_rows(&fixture.db, fixture.run_id);
        fs::remove_file(&fixture.paths[0]).unwrap();
        fs::write(
            &fixture.paths[1],
            b"externally modified content with a different size",
        )
        .unwrap();

        let request = ReviewLiveValidationRequest {
            operation_id: "external-change".to_owned(),
            run_id: fixture.run_id,
            group_id: fixture.group_id,
            expected_review_revision: 2,
            scope: "selection".to_owned(),
            file_ids: fixture.ids.clone(),
        };
        let result = fixture.db.validate_review_files(&request).unwrap();
        assert_eq!(result.items.len(), 3);
        assert_eq!(result.items[0].state, "missing");
        assert!(result.items[0].decision_invalidated);
        assert_eq!(
            result.items[0].invalidated_decision,
            Some(ReviewDecisionKind::Keep)
        );
        assert_eq!(result.items[1].state, "changed");
        assert!(result.items[1].decision_invalidated);
        assert_eq!(
            result.items[1].invalidated_decision,
            Some(ReviewDecisionKind::Remove)
        );
        assert_eq!(result.items[2].state, "present");
        assert!(!result.items[2].decision_invalidated);
        assert_eq!(
            immutable_rows(&fixture.db, fixture.run_id),
            immutable_before
        );
        assert_eq!(count_rows(&fixture.db, "review_decision"), 2);
        assert_eq!(count_rows(&fixture.db, "recorded_review_decision"), 2);
        assert_eq!(count_rows(&fixture.db, "effective_review_decision"), 0);

        let replay = fixture.db.validate_review_files(&request).unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.validation_id, result.validation_id);
        assert_eq!(count_rows(&fixture.db, "review_live_validation"), 1);
        drop(fixture.db);
        let reopened = Database::open(fixture.database_path.to_str().unwrap()).unwrap();
        assert_eq!(count_rows(&reopened, "review_decision"), 2);
        assert_eq!(count_rows(&reopened, "effective_review_decision"), 0);
        assert_eq!(count_rows(&reopened, "review_live_file_state"), 3);
        assert_eq!(immutable_rows(&reopened, fixture.run_id), immutable_before);
    }

    #[test]
    fn excluded_paths_are_not_accessed_and_stale_or_unbounded_requests_write_nothing() {
        let excluded = "excluded-root".to_owned();
        let fixture = Fixture::new(vec![excluded.clone()]);
        fixture
            .db
            .connection()
            .execute(
                "UPDATE scanned_file SET canonical_path = ?1 WHERE id = ?2",
                params![format!("{excluded}/placeholder.bin"), fixture.ids[0]],
            )
            .unwrap();
        let calls = AtomicUsize::new(0);
        let request = ReviewLiveValidationRequest {
            operation_id: "excluded".to_owned(),
            run_id: fixture.run_id,
            group_id: fixture.group_id,
            expected_review_revision: 0,
            scope: "visible_page".to_owned(),
            file_ids: fixture.ids.clone(),
        };
        let result = fixture
            .db
            .validate_review_files_with(&request, |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                observation("present", "injected")
            })
            .unwrap();
        assert_eq!(result.items[0].state, "unavailable");
        assert_eq!(result.items[0].reason_code, "excluded_location");
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        let mut stale = request.clone();
        stale.operation_id = "stale".to_owned();
        stale.expected_review_revision = 1;
        assert!(matches!(
            fixture.db.validate_review_files(&stale),
            Err(ReviewLiveValidationError::StaleRevision { .. })
        ));
        let mut duplicate = request.clone();
        duplicate.operation_id = "duplicate".to_owned();
        duplicate.file_ids = vec![fixture.ids[0], fixture.ids[0]];
        assert!(matches!(
            fixture.db.validate_review_files(&duplicate),
            Err(ReviewLiveValidationError::InvalidRequest { .. })
        ));
        let mut unbounded = request;
        unbounded.operation_id = "unbounded".to_owned();
        unbounded.file_ids = (1..=201).collect();
        assert!(matches!(
            fixture.db.validate_review_files(&unbounded),
            Err(ReviewLiveValidationError::InvalidRequest { .. })
        ));
        assert_eq!(count_rows(&fixture.db, "review_live_validation"), 1);
    }

    struct Fixture {
        _temp: TempDir,
        database_path: std::path::PathBuf,
        db: Database,
        run_id: i64,
        group_id: i64,
        ids: Vec<i64>,
        paths: Vec<std::path::PathBuf>,
    }

    impl Fixture {
        fn new(manual_location_exclusions: Vec<String>) -> Self {
            let temp = TempDir::new().unwrap();
            let database_path = temp.path().join("validation.db");
            let paths = (0..3)
                .map(|index| temp.path().join(format!("copy-{index}.bin")))
                .collect::<Vec<_>>();
            for path in &paths {
                fs::write(path, b"same validation bytes").unwrap();
            }
            let db = Database::open(database_path.to_str().unwrap()).unwrap();
            let root = temp.path().to_string_lossy().into_owned();
            let session_id = db
                .create_session("validation", std::slice::from_ref(&root), &[])
                .unwrap();
            let run_id = db
                .create_scan_run(
                    session_id,
                    &RunParameters {
                        roots: vec![root.clone()],
                        ignore_patterns: Vec::new(),
                        directory_similarity_threshold_millis: 500,
                        cloud_policy: CloudPolicy::ExcludeRegisteredRoots,
                        manual_location_exclusions,
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
                        content_hash: Some(7),
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
                    7,
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
                .prepare("SELECT id FROM scanned_file WHERE run_id = ?1 ORDER BY canonical_path")
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
