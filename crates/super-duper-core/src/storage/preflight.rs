use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::hash::Hasher;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use thiserror::Error;
use twox_hash::XxHash64;

use crate::hasher::xxhash::hash_file_streaming;
use crate::platform::{self, PathSafety};

use super::models::{
    CloudPolicy, Preflight, PreflightItem, PreflightItemPage, PreflightObservation,
    PreflightStartResult, PreflightSummary, PreflightView, RunParameters,
};
use super::review::{validate_review_state, ReviewError};
use super::Database;

const MAXIMUM_OPERATION_ID_CHARACTERS: usize = 128;

#[derive(Debug, Error)]
pub enum PreflightError {
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    Review(#[from] ReviewError),
    #[error("run {run_id} was not found")]
    RunNotFound { run_id: i64 },
    #[error("run {run_id} is {status}; preflight requires a completed run")]
    RunNotCompleted { run_id: i64, status: String },
    #[error("run {run_id} has no active review plan")]
    PlanNotFound { run_id: i64 },
    #[error("review revision {expected} is stale; current revision is {current}")]
    StaleReviewRevision { expected: i64, current: i64 },
    #[error("the reviewed plan has no removal targets")]
    EmptyPlan,
    #[error("operation id {operation_id} was already used for another preflight payload")]
    IdempotencyConflict { operation_id: String },
    #[error("preflight {preflight_id} was not found")]
    NotFound { preflight_id: i64 },
    #[error("preflight {preflight_id} cannot transition from {status}")]
    InvalidState { preflight_id: i64, status: String },
    #[error("the reviewed plan contains inconsistent immutable snapshots: {message}")]
    SnapshotConflict { message: String },
    #[error("invalid preflight request: {message}")]
    InvalidRequest { message: String },
    #[error("run {run_id} is locked by recycle operation {operation_id}")]
    OperationLocked { run_id: i64, operation_id: i64 },
}

#[derive(Clone)]
struct FileSnapshot {
    id: i64,
    group_id: Option<i64>,
    path: String,
    identity: Option<String>,
    size: i64,
    modified: i64,
    hash: Option<i64>,
}

#[derive(Clone)]
struct FolderSnapshot {
    group_id: i64,
    member_id: i64,
    directory_id: i64,
    path: String,
    structural_fingerprint: String,
    verified_fingerprint: String,
}

#[derive(Clone)]
struct SourceSnapshot {
    kind: &'static str,
    group_id: Option<i64>,
    folder_group_id: Option<i64>,
    folder_member_id: Option<i64>,
    file_id: Option<i64>,
    directory_id: Option<i64>,
    path: String,
}

struct PhysicalSnapshot {
    role: &'static str,
    file: FileSnapshot,
    sources: Vec<SourceSnapshot>,
}

struct FolderItemSnapshot {
    role: &'static str,
    folder: FolderSnapshot,
    sources: Vec<SourceSnapshot>,
}

impl Database {
    pub fn create_preflight(
        &self,
        operation_id: &str,
        run_id: i64,
        expected_review_revision: i64,
    ) -> Result<PreflightStartResult, PreflightError> {
        if operation_id.trim().is_empty()
            || operation_id.len() > MAXIMUM_OPERATION_ID_CHARACTERS
            || expected_review_revision < 0
        {
            return Err(PreflightError::InvalidRequest {
                message: "operationId must be 1 to 128 characters and expectedReviewRevision must be non-negative".to_owned(),
            });
        }

        let tx = self.connection().unchecked_transaction()?;
        if let Some((preflight_id, saved_run_id, saved_revision)) = tx
            .query_row(
                "SELECT id, run_id, review_revision FROM preflight WHERE operation_id = ?1",
                params![operation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
        {
            if saved_run_id != run_id || saved_revision != expected_review_revision {
                return Err(PreflightError::IdempotencyConflict {
                    operation_id: operation_id.to_owned(),
                });
            }
            tx.commit()?;
            return Ok(PreflightStartResult {
                view: self.get_preflight_view(preflight_id)?,
                replayed: true,
            });
        }

        if let Some((operation_id, status)) = tx
            .query_row(
                "SELECT id, status FROM recycle_operation
                 WHERE run_id = ?1 AND status IN
                    ('prepared', 'awaiting_confirmation', 'submitted', 'executing', 'cancelling', 'recovery_required')
                 ORDER BY id DESC LIMIT 1",
                params![run_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        {
            if status == "prepared" || status == "awaiting_confirmation" {
                tx.execute(
                    "UPDATE recycle_operation SET status = 'expired', completed_at = ?1,
                            error_code = 'preflight_superseded',
                            error_detail = 'A newer preflight generation invalidated the unsubmitted operation intent'
                     WHERE id = ?2",
                    params![Utc::now().to_rfc3339(), operation_id],
                )?;
            } else {
            return Err(PreflightError::OperationLocked {
                run_id,
                operation_id,
            });
            }
        }

        let run = tx
            .query_row(
                "SELECT status FROM scan_run WHERE id = ?1",
                params![run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(PreflightError::RunNotFound { run_id })?;
        if run != "completed" {
            return Err(PreflightError::RunNotCompleted {
                run_id,
                status: run,
            });
        }
        let (plan_id, current_revision) = tx
            .query_row(
                "SELECT id, revision FROM review_plan WHERE run_id = ?1 AND state = 'active'",
                params![run_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?
            .ok_or(PreflightError::PlanNotFound { run_id })?;
        if current_revision != expected_review_revision {
            return Err(PreflightError::StaleReviewRevision {
                expected: expected_review_revision,
                current: current_revision,
            });
        }
        validate_review_state(&tx, plan_id, run_id)?;

        let files = load_run_files(&tx, run_id)?;
        let directories = load_run_directories(&tx, run_id)?;
        let file_by_id = files
            .iter()
            .map(|file| (file.id, file.clone()))
            .collect::<HashMap<_, _>>();

        let mut removal_sources = HashMap::<i64, Vec<SourceSnapshot>>::new();
        {
            let mut statement = tx.prepare(
                "SELECT decision.file_id, decision.group_id
                 FROM effective_review_decision decision
                 WHERE decision.plan_id = ?1 AND decision.decision = 'remove'
                 ORDER BY decision.file_id",
            )?;
            let rows = statement.query_map(params![plan_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (file_id, group_id) = row?;
                let file =
                    file_by_id
                        .get(&file_id)
                        .ok_or_else(|| PreflightError::SnapshotConflict {
                            message: format!("review file {file_id} is absent from run {run_id}"),
                        })?;
                removal_sources
                    .entry(file_id)
                    .or_default()
                    .push(SourceSnapshot {
                        kind: "file_decision",
                        group_id: Some(group_id),
                        folder_group_id: None,
                        folder_member_id: None,
                        file_id: Some(file_id),
                        directory_id: None,
                        path: file.path.clone(),
                    });
            }
        }

        let removed_folders = load_removed_folders(&tx, plan_id)?;
        for folder in &removed_folders {
            for file in files
                .iter()
                .filter(|file| path_is_within(&file.path, &folder.path))
            {
                removal_sources
                    .entry(file.id)
                    .or_default()
                    .push(SourceSnapshot {
                        kind: "folder_decision",
                        group_id: file.group_id,
                        folder_group_id: Some(folder.group_id),
                        folder_member_id: Some(folder.member_id),
                        file_id: Some(file.id),
                        directory_id: Some(folder.directory_id),
                        path: file.path.clone(),
                    });
            }
        }
        if removal_sources.is_empty() && removed_folders.is_empty() {
            return Err(PreflightError::EmptyPlan);
        }

        let removed_file_ids = removal_sources.keys().copied().collect::<HashSet<_>>();
        let mut removals = BTreeMap::<String, PhysicalSnapshot>::new();
        let mut affected_groups = BTreeSet::<i64>::new();
        for (file_id, sources) in removal_sources {
            let file = file_by_id.get(&file_id).cloned().ok_or_else(|| {
                PreflightError::SnapshotConflict {
                    message: format!("removal file {file_id} is absent from run {run_id}"),
                }
            })?;
            if file.hash.is_none() {
                return Err(PreflightError::SnapshotConflict {
                    message: format!("removal file {file_id} has no complete scan hash"),
                });
            }
            if let Some(group_id) = file.group_id {
                affected_groups.insert(group_id);
            }
            insert_physical_snapshot(&mut removals, "remove", file, sources)?;
        }

        let mut survivors = BTreeMap::<String, PhysicalSnapshot>::new();
        for file in files.iter().filter(|file| {
            file.group_id
                .is_some_and(|group_id| affected_groups.contains(&group_id))
        }) {
            if removed_file_ids.contains(&file.id) {
                continue;
            }
            let group_id = file.group_id.expect("filtered group id");
            insert_physical_snapshot(
                &mut survivors,
                "survivor",
                file.clone(),
                vec![SourceSnapshot {
                    kind: "survivor",
                    group_id: Some(group_id),
                    folder_group_id: None,
                    folder_member_id: None,
                    file_id: Some(file.id),
                    directory_id: None,
                    path: file.path.clone(),
                }],
            )?;
        }
        for group_id in &affected_groups {
            if !survivors.values().any(|item| {
                item.sources
                    .iter()
                    .any(|source| source.group_id == Some(*group_id))
            }) {
                return Err(PreflightError::SnapshotConflict {
                    message: format!("duplicate group {group_id} has no physical survivor"),
                });
            }
        }

        let removed_folder_members = removed_folders
            .iter()
            .map(|folder| folder.member_id)
            .collect::<HashSet<_>>();
        let affected_folder_groups = removed_folders
            .iter()
            .map(|folder| folder.group_id)
            .collect::<BTreeSet<_>>();
        let all_affected_folders = load_folder_members(&tx, &affected_folder_groups)?;
        let mut folder_removals = Vec::new();
        let mut folder_survivors = Vec::new();
        for folder in all_affected_folders {
            let role = if removed_folder_members.contains(&folder.member_id) {
                "remove"
            } else {
                "survivor"
            };
            let sources = folder_sources(&folder, role, &files, &directories);
            let item = FolderItemSnapshot {
                role,
                folder,
                sources,
            };
            if role == "remove" {
                folder_removals.push(item);
            } else {
                folder_survivors.push(item);
            }
        }
        for group_id in &affected_folder_groups {
            if !folder_survivors
                .iter()
                .any(|item| item.folder.group_id == *group_id)
            {
                return Err(PreflightError::SnapshotConflict {
                    message: format!("exact-folder group {group_id} has no intact survivor"),
                });
            }
        }

        folder_removals.sort_by(|a, b| compare_paths(&a.folder.path, &b.folder.path));
        folder_survivors.sort_by(|a, b| compare_paths(&a.folder.path, &b.folder.path));
        let logical_removal_count = removals
            .values()
            .flat_map(|item| item.sources.iter().filter_map(|source| source.file_id))
            .collect::<HashSet<_>>()
            .len() as i64;
        let physical_removal_count = removals.len() as i64;
        let folder_removal_count = folder_removals.len() as i64;
        let planned_removal_bytes = removals.values().map(|item| item.file.size).sum::<i64>();
        let total_item_count =
            (removals.len() + survivors.len() + folder_removals.len() + folder_survivors.len())
                as i64;
        let snapshot_signature = snapshot_signature(
            run_id,
            plan_id,
            current_revision,
            removals.values(),
            survivors.values(),
            folder_removals.iter(),
            folder_survivors.iter(),
        );
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO preflight
                (operation_id, run_id, plan_id, review_revision, snapshot_signature, status,
                 logical_removal_count, physical_removal_count, folder_removal_count,
                 affected_group_count, planned_removal_bytes, total_item_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                operation_id,
                run_id,
                plan_id,
                current_revision,
                snapshot_signature,
                logical_removal_count,
                physical_removal_count,
                folder_removal_count,
                affected_groups.len() as i64,
                planned_removal_bytes,
                total_item_count,
                now,
            ],
        )?;
        let preflight_id = tx.last_insert_rowid();
        let mut ordinal = 0i64;
        for item in removals.values().chain(survivors.values()) {
            insert_file_item(&tx, preflight_id, ordinal, item)?;
            ordinal += 1;
        }
        for item in folder_removals.iter().chain(folder_survivors.iter()) {
            insert_folder_item(&tx, preflight_id, ordinal, item)?;
            ordinal += 1;
        }
        tx.commit()?;
        Ok(PreflightStartResult {
            view: self.get_preflight_view(preflight_id)?,
            replayed: false,
        })
    }

    pub fn get_preflight_view(&self, preflight_id: i64) -> Result<PreflightView, PreflightError> {
        let preflight = preflight_by_id(self.connection(), preflight_id)?
            .ok_or(PreflightError::NotFound { preflight_id })?;
        let current_review_revision = self
            .connection()
            .query_row(
                "SELECT revision FROM review_plan WHERE id = ?1 AND state = 'active'",
                params![preflight.plan_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(-1);
        Ok(PreflightView {
            is_current: current_review_revision == preflight.review_revision,
            current_review_revision,
            preflight,
        })
    }

    pub fn get_preflight_by_operation(
        &self,
        operation_id: &str,
    ) -> Result<Option<PreflightView>, PreflightError> {
        let id = self
            .connection()
            .query_row(
                "SELECT id FROM preflight WHERE operation_id = ?1",
                params![operation_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        id.map(|id| self.get_preflight_view(id)).transpose()
    }

    pub fn latest_preflight_for_run(
        &self,
        run_id: i64,
    ) -> Result<Option<PreflightView>, PreflightError> {
        let id = self
            .connection()
            .query_row(
                "SELECT id FROM preflight WHERE run_id = ?1 ORDER BY id DESC LIMIT 1",
                params![run_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        id.map(|id| self.get_preflight_view(id)).transpose()
    }

    pub fn page_preflight_items(
        &self,
        preflight_id: i64,
        offset: i64,
        limit: i64,
        outcome: Option<&str>,
    ) -> Result<PreflightItemPage, PreflightError> {
        if offset < 0 || !(1..=200).contains(&limit) {
            return Err(PreflightError::InvalidRequest {
                message: "offset must be non-negative and limit must be between 1 and 200"
                    .to_owned(),
            });
        }
        if preflight_by_id(self.connection(), preflight_id)?.is_none() {
            return Err(PreflightError::NotFound { preflight_id });
        }
        if outcome.is_some_and(|value| {
            !matches!(
                value,
                "pending" | "ready" | "changed" | "missing" | "unavailable" | "conflict"
            )
        }) {
            return Err(PreflightError::InvalidRequest {
                message:
                    "outcome must be pending, ready, changed, missing, unavailable, or conflict"
                        .to_owned(),
            });
        }
        let total = self.connection().query_row(
            "SELECT COUNT(*) FROM preflight_item
             WHERE preflight_id = ?1 AND (?2 IS NULL OR outcome = ?2)",
            params![preflight_id, outcome],
            |row| row.get::<_, i64>(0),
        )?;
        let mut statement = self.connection().prepare(
            "SELECT item.id, item.preflight_id, item.ordinal, item.target_kind, item.target_role,
                    item.physical_key, item.group_id, item.folder_group_id, item.folder_member_id,
                    item.snapshot_file_id, item.snapshot_directory_id, item.snapshot_path,
                    item.snapshot_file_identity, item.snapshot_file_size, item.snapshot_last_modified,
                    item.snapshot_content_hash, item.snapshot_structural_fingerprint,
                    item.snapshot_verified_fingerprint, item.outcome, item.reason_code,
                    item.observed_file_identity, item.observed_file_size,
                    item.observed_last_modified, item.observed_content_hash, item.os_error,
                    item.observed_at,
                    (SELECT COUNT(*) FROM preflight_item_source source WHERE source.item_id = item.id)
             FROM preflight_item item
             WHERE item.preflight_id = ?1 AND (?2 IS NULL OR item.outcome = ?2)
             ORDER BY CASE item.outcome
                        WHEN 'conflict' THEN 0 WHEN 'changed' THEN 1 WHEN 'missing' THEN 2
                        WHEN 'unavailable' THEN 3 WHEN 'pending' THEN 4 ELSE 5 END,
                      item.target_role, item.target_kind,
                      item.snapshot_path COLLATE UNICODE_NOCASE, item.id
             LIMIT ?3 OFFSET ?4",
        )?;
        let rows =
            statement.query_map(params![preflight_id, outcome, limit, offset], item_from_row)?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(PreflightItemPage {
            has_more: offset + (items.len() as i64) < total,
            items,
            total,
        })
    }

    pub fn mark_preflight_running(&self, preflight_id: i64) -> Result<Preflight, PreflightError> {
        let now = Utc::now().to_rfc3339();
        let changed = self.connection().execute(
            "UPDATE preflight SET status = 'running', started_at = ?1,
                    completed_at = NULL, error_code = NULL, error_detail = NULL
             WHERE id = ?2 AND status = 'pending'",
            params![now, preflight_id],
        )?;
        if changed == 0 {
            let existing = preflight_by_id(self.connection(), preflight_id)?
                .ok_or(PreflightError::NotFound { preflight_id })?;
            return Err(PreflightError::InvalidState {
                preflight_id,
                status: existing.status,
            });
        }
        Ok(preflight_by_id(self.connection(), preflight_id)?.expect("updated preflight"))
    }

    pub fn mark_preflight_cancelling(
        &self,
        preflight_id: i64,
    ) -> Result<Preflight, PreflightError> {
        self.connection().execute(
            "UPDATE preflight SET status = 'cancelling'
             WHERE id = ?1 AND status = 'running'",
            params![preflight_id],
        )?;
        let existing = preflight_by_id(self.connection(), preflight_id)?
            .ok_or(PreflightError::NotFound { preflight_id })?;
        if !matches!(existing.status.as_str(), "cancelling" | "cancelled") {
            return Err(PreflightError::InvalidState {
                preflight_id,
                status: existing.status,
            });
        }
        Ok(existing)
    }

    pub fn validate_preflight<F>(
        &self,
        preflight_id: i64,
        cancel_token: &AtomicBool,
        mut progress: F,
    ) -> Result<Preflight, PreflightError>
    where
        F: FnMut(&Preflight, Option<&str>),
    {
        let current = preflight_by_id(self.connection(), preflight_id)?
            .ok_or(PreflightError::NotFound { preflight_id })?;
        if current.status == "pending" {
            self.mark_preflight_running(preflight_id)?;
        } else if current.status != "running" {
            return Err(PreflightError::InvalidState {
                preflight_id,
                status: current.status,
            });
        }
        let exclusions = self.preflight_exclusions(preflight_id)?;
        loop {
            if cancel_token.load(Ordering::Acquire) {
                self.finish_preflight(preflight_id, "cancelled", None, None)?;
                let terminal =
                    preflight_by_id(self.connection(), preflight_id)?.expect("preflight");
                progress(&terminal, None);
                return Ok(terminal);
            }
            let Some(item) = next_pending_item(self.connection(), preflight_id)? else {
                break;
            };
            let sources = item_sources(self.connection(), item.id)?;
            let observation = validate_item(&item, &sources, &exclusions, cancel_token);
            if observation.outcome == "cancelled" {
                self.finish_preflight(preflight_id, "cancelled", None, None)?;
                let terminal =
                    preflight_by_id(self.connection(), preflight_id)?.expect("preflight");
                progress(&terminal, None);
                return Ok(terminal);
            }
            self.record_preflight_observation(item.id, &observation)?;
            let current = preflight_by_id(self.connection(), preflight_id)?.expect("preflight");
            progress(&current, Some(&item.snapshot_path));
        }
        enforce_live_survivors(self.connection(), preflight_id)?;
        refresh_preflight_counts(self.connection(), preflight_id)?;
        self.finish_preflight(preflight_id, "completed", None, None)?;
        let terminal = preflight_by_id(self.connection(), preflight_id)?.expect("preflight");
        progress(&terminal, None);
        Ok(terminal)
    }

    pub fn fail_preflight(
        &self,
        preflight_id: i64,
        error_code: &str,
        detail: &str,
    ) -> Result<(), PreflightError> {
        self.finish_preflight(preflight_id, "failed", Some(error_code), Some(detail))
    }

    fn finish_preflight(
        &self,
        preflight_id: i64,
        status: &str,
        error_code: Option<&str>,
        error_detail: Option<&str>,
    ) -> Result<(), PreflightError> {
        let now = Utc::now().to_rfc3339();
        self.connection().execute(
            "UPDATE preflight
             SET status = ?1, completed_at = ?2, error_code = ?3, error_detail = ?4
             WHERE id = ?5 AND status IN ('running', 'cancelling')",
            params![status, now, error_code, error_detail, preflight_id],
        )?;
        Ok(())
    }

    fn record_preflight_observation(
        &self,
        item_id: i64,
        observation: &PreflightObservation,
    ) -> Result<(), PreflightError> {
        if !matches!(
            observation.outcome.as_str(),
            "ready" | "changed" | "missing" | "unavailable" | "conflict"
        ) {
            return Err(PreflightError::InvalidRequest {
                message: "invalid preflight observation outcome".to_owned(),
            });
        }
        let now = Utc::now().to_rfc3339();
        self.connection().execute(
            "UPDATE preflight_item
             SET outcome = ?1, reason_code = ?2, observed_file_identity = ?3,
                 observed_file_size = ?4, observed_last_modified = ?5,
                 observed_content_hash = ?6, os_error = ?7, observed_at = ?8
             WHERE id = ?9 AND outcome = 'pending'",
            params![
                observation.outcome,
                observation.reason_code,
                observation.observed_file_identity,
                observation.observed_file_size,
                observation.observed_last_modified,
                observation.observed_content_hash,
                observation.os_error,
                now,
                item_id,
            ],
        )?;
        refresh_preflight_counts_for_item(self.connection(), item_id)?;
        Ok(())
    }

    fn preflight_exclusions(&self, preflight_id: i64) -> Result<Vec<String>, PreflightError> {
        let parameters_json = self.connection().query_row(
            "SELECT run.parameters_json
             FROM preflight validation JOIN scan_run run ON run.id = validation.run_id
             WHERE validation.id = ?1",
            params![preflight_id],
            |row| row.get::<_, String>(0),
        )?;
        let parameters = RunParameters::from_json(&parameters_json).ok_or_else(|| {
            PreflightError::SnapshotConflict {
                message: "run parameter snapshot could not be decoded".to_owned(),
            }
        })?;
        let mut exclusions = parameters.manual_location_exclusions;
        if parameters.cloud_policy == CloudPolicy::ExcludeRegisteredRoots {
            exclusions.extend(
                parameters
                    .registered_cloud_locations
                    .into_iter()
                    .map(|location| location.path),
            );
        }
        exclusions.sort_by(|left, right| compare_paths(left, right));
        exclusions.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        Ok(exclusions)
    }
}

fn load_run_files(tx: &Transaction<'_>, run_id: i64) -> Result<Vec<FileSnapshot>, PreflightError> {
    let mut statement = tx.prepare(
        "SELECT file.id, member.group_id, file.canonical_path, file.file_identity,
                file.file_size, file.last_modified, file.content_hash
         FROM scanned_file file
         LEFT JOIN duplicate_group_member member ON member.file_id = file.id
         LEFT JOIN duplicate_group duplicate_set
           ON duplicate_set.id = member.group_id AND duplicate_set.run_id = file.run_id
         WHERE file.run_id = ?1
         ORDER BY file.id",
    )?;
    let rows = statement.query_map(params![run_id], |row| {
        Ok(FileSnapshot {
            id: row.get(0)?,
            group_id: row.get(1)?,
            path: row.get(2)?,
            identity: row.get(3)?,
            size: row.get(4)?,
            modified: row.get(5)?,
            hash: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn load_run_directories(
    tx: &Transaction<'_>,
    run_id: i64,
) -> Result<Vec<(i64, String)>, PreflightError> {
    let mut statement =
        tx.prepare("SELECT id, path FROM directory_node WHERE run_id = ?1 ORDER BY id")?;
    let rows = statement.query_map(params![run_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn load_removed_folders(
    tx: &Transaction<'_>,
    plan_id: i64,
) -> Result<Vec<FolderSnapshot>, PreflightError> {
    let mut statement = tx.prepare(
        "SELECT decision.folder_group_id, decision.folder_member_id, decision.directory_id,
                decision.snapshot_path, decision.snapshot_structural_fingerprint,
                decision.snapshot_verified_fingerprint
         FROM review_folder_decision decision
         WHERE decision.plan_id = ?1 AND decision.decision = 'remove'
         ORDER BY decision.folder_member_id",
    )?;
    let rows = statement.query_map(params![plan_id], |row| {
        Ok(FolderSnapshot {
            group_id: row.get(0)?,
            member_id: row.get(1)?,
            directory_id: row.get(2)?,
            path: row.get(3)?,
            structural_fingerprint: row.get(4)?,
            verified_fingerprint: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn load_folder_members(
    tx: &Transaction<'_>,
    group_ids: &BTreeSet<i64>,
) -> Result<Vec<FolderSnapshot>, PreflightError> {
    if group_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut statement = tx.prepare(
        "SELECT member.group_id, member.id, member.directory_id, directory.path,
                folder.structural_fingerprint, folder.verified_fingerprint
         FROM duplicate_folder_group_member member
         JOIN duplicate_folder_group folder ON folder.id = member.group_id
         JOIN directory_node directory ON directory.id = member.directory_id
         WHERE member.group_id = ?1 ORDER BY member.id",
    )?;
    let mut folders = Vec::new();
    for group_id in group_ids {
        let rows = statement.query_map(params![group_id], |row| {
            Ok(FolderSnapshot {
                group_id: row.get(0)?,
                member_id: row.get(1)?,
                directory_id: row.get(2)?,
                path: row.get(3)?,
                structural_fingerprint: row.get(4)?,
                verified_fingerprint: row.get(5)?,
            })
        })?;
        folders.extend(rows.collect::<Result<Vec<_>, _>>()?);
    }
    Ok(folders)
}

fn folder_sources(
    folder: &FolderSnapshot,
    role: &str,
    files: &[FileSnapshot],
    directories: &[(i64, String)],
) -> Vec<SourceSnapshot> {
    let kind = if role == "remove" {
        "folder_decision"
    } else {
        "survivor"
    };
    let mut sources = files
        .iter()
        .filter(|file| path_is_within(&file.path, &folder.path))
        .map(|file| SourceSnapshot {
            kind,
            group_id: file.group_id,
            folder_group_id: Some(folder.group_id),
            folder_member_id: Some(folder.member_id),
            file_id: Some(file.id),
            directory_id: None,
            path: file.path.clone(),
        })
        .collect::<Vec<_>>();
    sources.extend(
        directories
            .iter()
            .filter(|(_, path)| {
                path_is_within(path, &folder.path) && !same_path(path, &folder.path)
            })
            .map(|(directory_id, path)| SourceSnapshot {
                kind,
                group_id: None,
                folder_group_id: Some(folder.group_id),
                folder_member_id: Some(folder.member_id),
                file_id: None,
                directory_id: Some(*directory_id),
                path: path.clone(),
            }),
    );
    sources.sort_by(|left, right| compare_paths(&left.path, &right.path));
    sources
}

fn insert_physical_snapshot(
    items: &mut BTreeMap<String, PhysicalSnapshot>,
    role: &'static str,
    file: FileSnapshot,
    mut sources: Vec<SourceSnapshot>,
) -> Result<(), PreflightError> {
    let key = physical_key(&file);
    if let Some(existing) = items.get_mut(&key) {
        if existing.file.identity != file.identity
            || existing.file.size != file.size
            || existing.file.modified != file.modified
            || existing.file.hash != file.hash
        {
            return Err(PreflightError::SnapshotConflict {
                message: format!("physical key {key} has divergent scan metadata"),
            });
        }
        existing.sources.append(&mut sources);
        existing
            .sources
            .sort_by(|left, right| compare_paths(&left.path, &right.path));
    } else {
        sources.sort_by(|left, right| compare_paths(&left.path, &right.path));
        items.insert(
            key,
            PhysicalSnapshot {
                role,
                file,
                sources,
            },
        );
    }
    Ok(())
}

fn insert_file_item(
    tx: &Transaction<'_>,
    preflight_id: i64,
    ordinal: i64,
    item: &PhysicalSnapshot,
) -> Result<(), PreflightError> {
    let group_id = item.sources.iter().find_map(|source| source.group_id);
    tx.execute(
        "INSERT INTO preflight_item
            (preflight_id, ordinal, target_kind, target_role, physical_key, group_id,
             snapshot_file_id, snapshot_path, snapshot_file_identity, snapshot_file_size,
             snapshot_last_modified, snapshot_content_hash)
         VALUES (?1, ?2, 'file', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            preflight_id,
            ordinal,
            item.role,
            physical_key(&item.file),
            group_id,
            item.file.id,
            item.file.path,
            item.file.identity,
            item.file.size,
            item.file.modified,
            item.file.hash,
        ],
    )?;
    let item_id = tx.last_insert_rowid();
    insert_sources(tx, item_id, &item.sources)
}

fn insert_folder_item(
    tx: &Transaction<'_>,
    preflight_id: i64,
    ordinal: i64,
    item: &FolderItemSnapshot,
) -> Result<(), PreflightError> {
    tx.execute(
        "INSERT INTO preflight_item
            (preflight_id, ordinal, target_kind, target_role, physical_key,
             folder_group_id, folder_member_id, snapshot_directory_id, snapshot_path,
             snapshot_structural_fingerprint, snapshot_verified_fingerprint)
         VALUES (?1, ?2, 'folder', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            preflight_id,
            ordinal,
            item.role,
            format!("folder:{}", normalize_path(&item.folder.path)),
            item.folder.group_id,
            item.folder.member_id,
            item.folder.directory_id,
            item.folder.path,
            item.folder.structural_fingerprint,
            item.folder.verified_fingerprint,
        ],
    )?;
    let item_id = tx.last_insert_rowid();
    insert_sources(tx, item_id, &item.sources)
}

fn insert_sources(
    tx: &Transaction<'_>,
    item_id: i64,
    sources: &[SourceSnapshot],
) -> Result<(), PreflightError> {
    let mut statement = tx.prepare(
        "INSERT INTO preflight_item_source
            (item_id, source_kind, group_id, folder_group_id, folder_member_id,
             file_id, directory_id, snapshot_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    for source in sources {
        statement.execute(params![
            item_id,
            source.kind,
            source.group_id,
            source.folder_group_id,
            source.folder_member_id,
            source.file_id,
            source.directory_id,
            source.path,
        ])?;
    }
    Ok(())
}

fn preflight_by_id(
    connection: &Connection,
    preflight_id: i64,
) -> rusqlite::Result<Option<Preflight>> {
    connection
        .query_row(
            "SELECT id, operation_id, run_id, plan_id, review_revision, snapshot_signature,
                    status, logical_removal_count, physical_removal_count, folder_removal_count,
                    affected_group_count, planned_removal_bytes, total_item_count,
                    processed_item_count, ready_count, changed_count, missing_count,
                    unavailable_count, conflict_count, created_at, started_at, completed_at,
                    error_code, error_detail
             FROM preflight WHERE id = ?1",
            params![preflight_id],
            |row| {
                Ok(Preflight {
                    id: row.get(0)?,
                    operation_id: row.get(1)?,
                    run_id: row.get(2)?,
                    plan_id: row.get(3)?,
                    review_revision: row.get(4)?,
                    snapshot_signature: row.get(5)?,
                    status: row.get(6)?,
                    summary: PreflightSummary {
                        logical_removal_count: row.get(7)?,
                        physical_removal_count: row.get(8)?,
                        folder_removal_count: row.get(9)?,
                        affected_group_count: row.get(10)?,
                        planned_removal_bytes: row.get(11)?,
                        total_item_count: row.get(12)?,
                        processed_item_count: row.get(13)?,
                        ready_count: row.get(14)?,
                        changed_count: row.get(15)?,
                        missing_count: row.get(16)?,
                        unavailable_count: row.get(17)?,
                        conflict_count: row.get(18)?,
                    },
                    created_at: row.get(19)?,
                    started_at: row.get(20)?,
                    completed_at: row.get(21)?,
                    error_code: row.get(22)?,
                    error_detail: row.get(23)?,
                })
            },
        )
        .optional()
}

fn item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PreflightItem> {
    Ok(PreflightItem {
        id: row.get(0)?,
        preflight_id: row.get(1)?,
        ordinal: row.get(2)?,
        target_kind: row.get(3)?,
        target_role: row.get(4)?,
        physical_key: row.get(5)?,
        group_id: row.get(6)?,
        folder_group_id: row.get(7)?,
        folder_member_id: row.get(8)?,
        snapshot_file_id: row.get(9)?,
        snapshot_directory_id: row.get(10)?,
        snapshot_path: row.get(11)?,
        snapshot_file_identity: row.get(12)?,
        snapshot_file_size: row.get(13)?,
        snapshot_last_modified: row.get(14)?,
        snapshot_content_hash: row.get(15)?,
        snapshot_structural_fingerprint: row.get(16)?,
        snapshot_verified_fingerprint: row.get(17)?,
        outcome: row.get(18)?,
        reason_code: row.get(19)?,
        observed_file_identity: row.get(20)?,
        observed_file_size: row.get(21)?,
        observed_last_modified: row.get(22)?,
        observed_content_hash: row.get(23)?,
        os_error: row.get(24)?,
        observed_at: row.get(25)?,
        source_count: row.get(26)?,
    })
}

fn next_pending_item(
    connection: &Connection,
    preflight_id: i64,
) -> rusqlite::Result<Option<PreflightItem>> {
    connection
        .query_row(
            "SELECT item.id, item.preflight_id, item.ordinal, item.target_kind, item.target_role,
                    item.physical_key, item.group_id, item.folder_group_id, item.folder_member_id,
                    item.snapshot_file_id, item.snapshot_directory_id, item.snapshot_path,
                    item.snapshot_file_identity, item.snapshot_file_size, item.snapshot_last_modified,
                    item.snapshot_content_hash, item.snapshot_structural_fingerprint,
                    item.snapshot_verified_fingerprint, item.outcome, item.reason_code,
                    item.observed_file_identity, item.observed_file_size,
                    item.observed_last_modified, item.observed_content_hash, item.os_error,
                    item.observed_at,
                    (SELECT COUNT(*) FROM preflight_item_source source WHERE source.item_id = item.id)
             FROM preflight_item item
             WHERE item.preflight_id = ?1 AND item.outcome = 'pending'
             ORDER BY item.ordinal LIMIT 1",
            params![preflight_id],
            item_from_row,
        )
        .optional()
}

fn item_sources(connection: &Connection, item_id: i64) -> rusqlite::Result<Vec<SourceSnapshot>> {
    let mut statement = connection.prepare(
        "SELECT source_kind, group_id, folder_group_id, folder_member_id,
                file_id, directory_id, snapshot_path
         FROM preflight_item_source WHERE item_id = ?1 ORDER BY id",
    )?;
    let rows = statement.query_map(params![item_id], |row| {
        let kind: String = row.get(0)?;
        let kind = match kind.as_str() {
            "file_decision" => "file_decision",
            "folder_decision" => "folder_decision",
            _ => "survivor",
        };
        Ok(SourceSnapshot {
            kind,
            group_id: row.get(1)?,
            folder_group_id: row.get(2)?,
            folder_member_id: row.get(3)?,
            file_id: row.get(4)?,
            directory_id: row.get(5)?,
            path: row.get(6)?,
        })
    })?;
    rows.collect()
}

fn validate_item(
    item: &PreflightItem,
    sources: &[SourceSnapshot],
    exclusions: &[String],
    cancel_token: &AtomicBool,
) -> PreflightObservation {
    if exclusions
        .iter()
        .any(|exclusion| path_is_within(&item.snapshot_path, exclusion))
    {
        return observation("conflict", "excluded_location");
    }
    if item.target_kind == "folder" {
        validate_folder(item, sources, exclusions, cancel_token)
    } else {
        validate_file(item, sources, exclusions, cancel_token)
    }
}

fn validate_file(
    item: &PreflightItem,
    sources: &[SourceSnapshot],
    exclusions: &[String],
    cancel_token: &AtomicBool,
) -> PreflightObservation {
    if cancel_token.load(Ordering::Acquire) {
        return observation("cancelled", "cancelled");
    }
    let expected_identity = match item.snapshot_file_identity.as_deref() {
        Some(identity) if !identity.is_empty() => identity,
        _ => return observation("conflict", "snapshot_identity_missing"),
    };
    let expected_hash = match item.snapshot_content_hash {
        Some(hash) => hash,
        None => return observation("conflict", "snapshot_hash_missing"),
    };
    let path = Path::new(&item.snapshot_path);
    match classify(path) {
        Ok(PathSafety::Missing) => return observation("missing", "path_missing"),
        Ok(PathSafety::CloudPlaceholder) => return observation("conflict", "cloud_placeholder"),
        Ok(PathSafety::ReparsePoint) => return observation("conflict", "reparse_point"),
        Ok(PathSafety::Directory) => return observation("conflict", "wrong_type_directory"),
        Ok(PathSafety::Other) => return observation("conflict", "wrong_type_other"),
        Ok(PathSafety::File) => {}
        Err(error) => return unavailable_observation("metadata_unavailable", &error),
    }
    let before = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return observation("missing", "path_missing")
        }
        Err(error) => return unavailable_observation("metadata_unavailable", &error),
    };
    let modified = match modified_nanos(&before) {
        Ok(modified) => modified,
        Err(error) => return unavailable_observation("timestamp_unavailable", &error),
    };
    let identity = match platform::file_identity(path) {
        Ok(Some(identity)) => identity,
        Ok(None) => return observation("conflict", "identity_unavailable"),
        Err(error) => return unavailable_observation("identity_unavailable", &error),
    };
    let mut actual = observation("ready", "matched_snapshot");
    actual.observed_file_identity = Some(identity.clone());
    actual.observed_file_size = Some(before.len().min(i64::MAX as u64) as i64);
    actual.observed_last_modified = Some(modified);
    if identity != expected_identity {
        actual.outcome = "changed".to_owned();
        actual.reason_code = Some("identity_changed".to_owned());
        return actual;
    }
    if item.snapshot_file_size != Some(before.len().min(i64::MAX as u64) as i64) {
        actual.outcome = "changed".to_owned();
        actual.reason_code = Some("size_changed".to_owned());
        return actual;
    }
    if item.snapshot_last_modified != Some(modified) {
        actual.outcome = "changed".to_owned();
        actual.reason_code = Some("timestamp_changed".to_owned());
        return actual;
    }

    for alias in sources
        .iter()
        .filter(|source| source.file_id.is_some() && !same_path(&source.path, &item.snapshot_path))
    {
        if exclusions
            .iter()
            .any(|exclusion| path_is_within(&alias.path, exclusion))
        {
            return observation("conflict", "alias_excluded_location");
        }
        match classify(Path::new(&alias.path)) {
            Ok(PathSafety::File) => {}
            Ok(PathSafety::Missing) => return observation("missing", "alias_missing"),
            Ok(PathSafety::CloudPlaceholder) => {
                return observation("conflict", "alias_cloud_placeholder")
            }
            Ok(PathSafety::ReparsePoint) => return observation("conflict", "alias_reparse_point"),
            Ok(_) => return observation("conflict", "alias_wrong_type"),
            Err(error) => return unavailable_observation("alias_unavailable", &error),
        }
        match platform::file_identity(Path::new(&alias.path)) {
            Ok(Some(alias_identity)) if alias_identity == identity => {}
            Ok(Some(_)) => return observation("conflict", "alias_identity_changed"),
            Ok(None) => return observation("conflict", "alias_identity_unavailable"),
            Err(error) => return unavailable_observation("alias_identity_unavailable", &error),
        }
    }

    let hash = match hash_file_streaming(path, cancel_token) {
        Ok(hash) => hash as i64,
        Err(error) if error.kind() == io::ErrorKind::Interrupted => {
            return observation("cancelled", "cancelled")
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return observation("missing", "path_missing_during_hash")
        }
        Err(error) => return unavailable_observation("hash_unavailable", &error),
    };
    actual.observed_content_hash = Some(hash);
    let after = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => return unavailable_observation("post_hash_metadata_unavailable", &error),
    };
    let after_modified = match modified_nanos(&after) {
        Ok(modified) => modified,
        Err(error) => return unavailable_observation("post_hash_timestamp_unavailable", &error),
    };
    let after_identity = match platform::file_identity(path) {
        Ok(Some(identity)) => identity,
        Ok(None) => return observation("conflict", "post_hash_identity_unavailable"),
        Err(error) => return unavailable_observation("post_hash_identity_unavailable", &error),
    };
    if after.len() != before.len() || after_modified != modified || after_identity != identity {
        actual.outcome = "changed".to_owned();
        actual.reason_code = Some("changed_during_validation".to_owned());
    } else if hash != expected_hash {
        actual.outcome = "changed".to_owned();
        actual.reason_code = Some("content_hash_changed".to_owned());
    }
    actual
}

fn validate_folder(
    item: &PreflightItem,
    sources: &[SourceSnapshot],
    exclusions: &[String],
    cancel_token: &AtomicBool,
) -> PreflightObservation {
    let root = Path::new(&item.snapshot_path);
    match classify(root) {
        Ok(PathSafety::Missing) => return observation("missing", "folder_missing"),
        Ok(PathSafety::Directory) => {}
        Ok(PathSafety::CloudPlaceholder) => {
            return observation("conflict", "folder_cloud_placeholder")
        }
        Ok(PathSafety::ReparsePoint) => return observation("conflict", "folder_reparse_point"),
        Ok(_) => return observation("conflict", "folder_wrong_type"),
        Err(error) => return unavailable_observation("folder_unavailable", &error),
    }
    let expected_files = sources
        .iter()
        .filter(|source| source.file_id.is_some())
        .filter_map(|source| relative_key(root, Path::new(&source.path)))
        .collect::<BTreeSet<_>>();
    let expected_directories = sources
        .iter()
        .filter(|source| source.directory_id.is_some())
        .filter_map(|source| relative_key(root, Path::new(&source.path)))
        .collect::<BTreeSet<_>>();
    let mut actual_files = BTreeSet::new();
    let mut actual_directories = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        if cancel_token.load(Ordering::Acquire) {
            return observation("cancelled", "cancelled");
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => return unavailable_observation("folder_enumeration_unavailable", &error),
        };
        for entry in entries {
            if cancel_token.load(Ordering::Acquire) {
                return observation("cancelled", "cancelled");
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => return unavailable_observation("folder_entry_unavailable", &error),
            };
            let path = entry.path();
            if exclusions
                .iter()
                .any(|excluded| path_is_within(&path.to_string_lossy(), excluded))
            {
                return observation("conflict", "folder_contains_excluded_location");
            }
            let key = match relative_key(root, &path) {
                Some(key) => key,
                None => return observation("conflict", "folder_path_escape"),
            };
            match classify(&path) {
                Ok(PathSafety::File) => {
                    actual_files.insert(key);
                }
                Ok(PathSafety::Directory) => {
                    actual_directories.insert(key);
                    pending.push(path);
                }
                Ok(PathSafety::CloudPlaceholder) => {
                    return observation("conflict", "folder_contains_cloud_placeholder")
                }
                Ok(PathSafety::ReparsePoint) => {
                    return observation("conflict", "folder_contains_reparse_point")
                }
                Ok(PathSafety::Missing) => {
                    return observation("changed", "folder_changed_during_enumeration")
                }
                Ok(PathSafety::Other) => {
                    return observation("conflict", "folder_contains_unsupported_type")
                }
                Err(error) => return unavailable_observation("folder_entry_unavailable", &error),
            }
        }
    }
    if actual_files != expected_files || actual_directories != expected_directories {
        return observation("conflict", "folder_tree_changed");
    }
    observation("ready", "folder_tree_matched")
}

fn enforce_live_survivors(
    connection: &Connection,
    preflight_id: i64,
) -> Result<(), PreflightError> {
    let tx = connection.unchecked_transaction()?;
    let mut groups = Vec::new();
    {
        let mut statement = tx.prepare(
            "SELECT DISTINCT source.group_id FROM preflight_item_source source
             JOIN preflight_item item ON item.id = source.item_id
             WHERE item.preflight_id = ?1 AND source.group_id IS NOT NULL",
        )?;
        let rows = statement.query_map(params![preflight_id], |row| row.get::<_, i64>(0))?;
        groups.extend(rows.collect::<Result<Vec<_>, _>>()?);
    }
    for group_id in groups {
        let ready: bool = tx.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM preflight_item item
                JOIN preflight_item_source source ON source.item_id = item.id
                WHERE item.preflight_id = ?1 AND item.target_kind = 'file'
                  AND item.target_role = 'survivor' AND item.outcome = 'ready'
                  AND source.group_id = ?2
             )",
            params![preflight_id, group_id],
            |row| row.get(0),
        )?;
        if !ready {
            tx.execute(
                "UPDATE preflight_item SET outcome = 'conflict',
                        reason_code = 'survivor_not_ready'
                 WHERE preflight_id = ?1 AND target_kind = 'file' AND target_role = 'remove'
                   AND outcome = 'ready' AND id IN (
                       SELECT item_id FROM preflight_item_source WHERE group_id = ?2
                   )",
                params![preflight_id, group_id],
            )?;
        }
    }
    let mut folder_groups = Vec::new();
    {
        let mut statement = tx.prepare(
            "SELECT DISTINCT folder_group_id FROM preflight_item
             WHERE preflight_id = ?1 AND target_kind = 'folder' AND folder_group_id IS NOT NULL",
        )?;
        let rows = statement.query_map(params![preflight_id], |row| row.get::<_, i64>(0))?;
        folder_groups.extend(rows.collect::<Result<Vec<_>, _>>()?);
    }
    for group_id in folder_groups {
        let ready: bool = tx.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM preflight_item WHERE preflight_id = ?1
                  AND target_kind = 'folder' AND target_role = 'survivor'
                  AND folder_group_id = ?2 AND outcome = 'ready'
             )",
            params![preflight_id, group_id],
            |row| row.get(0),
        )?;
        if !ready {
            tx.execute(
                "UPDATE preflight_item SET outcome = 'conflict',
                        reason_code = 'folder_survivor_not_ready'
                 WHERE preflight_id = ?1 AND target_kind = 'folder'
                   AND target_role = 'remove' AND folder_group_id = ?2 AND outcome = 'ready'",
                params![preflight_id, group_id],
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn refresh_preflight_counts_for_item(
    connection: &Connection,
    item_id: i64,
) -> rusqlite::Result<()> {
    let preflight_id = connection.query_row(
        "SELECT preflight_id FROM preflight_item WHERE id = ?1",
        params![item_id],
        |row| row.get::<_, i64>(0),
    )?;
    refresh_preflight_counts(connection, preflight_id)
}

fn refresh_preflight_counts(connection: &Connection, preflight_id: i64) -> rusqlite::Result<()> {
    connection.execute(
        "UPDATE preflight SET
             processed_item_count = (SELECT COUNT(*) FROM preflight_item WHERE preflight_id = ?1 AND outcome <> 'pending'),
             ready_count = (SELECT COUNT(*) FROM preflight_item WHERE preflight_id = ?1 AND outcome = 'ready'),
             changed_count = (SELECT COUNT(*) FROM preflight_item WHERE preflight_id = ?1 AND outcome = 'changed'),
             missing_count = (SELECT COUNT(*) FROM preflight_item WHERE preflight_id = ?1 AND outcome = 'missing'),
             unavailable_count = (SELECT COUNT(*) FROM preflight_item WHERE preflight_id = ?1 AND outcome = 'unavailable'),
             conflict_count = (SELECT COUNT(*) FROM preflight_item WHERE preflight_id = ?1 AND outcome = 'conflict')
         WHERE id = ?1",
        params![preflight_id],
    )?;
    Ok(())
}

fn observation(outcome: &str, reason: &str) -> PreflightObservation {
    PreflightObservation {
        outcome: outcome.to_owned(),
        reason_code: Some(reason.to_owned()),
        observed_file_identity: None,
        observed_file_size: None,
        observed_last_modified: None,
        observed_content_hash: None,
        os_error: None,
    }
}

fn unavailable_observation(reason: &str, error: &io::Error) -> PreflightObservation {
    let mut value = observation("unavailable", reason);
    value.os_error = error.raw_os_error().map(i64::from);
    value
}

fn classify(path: &Path) -> io::Result<PathSafety> {
    platform::classify_path_without_open(path)
}

fn modified_nanos(metadata: &fs::Metadata) -> io::Result<i64> {
    metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn physical_key(file: &FileSnapshot) -> String {
    file.identity.as_ref().map_or_else(
        || format!("path:{}", normalize_path(&file.path)),
        |identity| format!("identity:{identity}"),
    )
}

fn path_is_within(path: &str, root: &str) -> bool {
    let path = normalize_path(path);
    let root = normalize_path(root).trim_end_matches('/').to_owned();
    path == root
        || path
            .strip_prefix(&root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn same_path(left: &str, right: &str) -> bool {
    normalize_path(left) == normalize_path(right)
}

fn compare_paths(left: &str, right: &str) -> std::cmp::Ordering {
    normalize_path(left)
        .cmp(&normalize_path(right))
        .then_with(|| left.cmp(right))
}

fn relative_key(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|relative| normalize_path(&relative.to_string_lossy()))
        .filter(|relative| !relative.is_empty())
}

fn snapshot_signature<'a>(
    run_id: i64,
    plan_id: i64,
    revision: i64,
    removals: impl Iterator<Item = &'a PhysicalSnapshot>,
    survivors: impl Iterator<Item = &'a PhysicalSnapshot>,
    folder_removals: impl Iterator<Item = &'a FolderItemSnapshot>,
    folder_survivors: impl Iterator<Item = &'a FolderItemSnapshot>,
) -> String {
    let mut hasher = XxHash64::with_seed(0);
    hasher.write(format!("preflight-v1\n{run_id}\n{plan_id}\n{revision}\n").as_bytes());
    for item in removals.chain(survivors) {
        hasher.write(
            format!(
                "file|{}|{}|{}|{}|{}|{:?}\n",
                item.role,
                physical_key(&item.file),
                normalize_path(&item.file.path),
                item.file.size,
                item.file.modified,
                item.file.hash
            )
            .as_bytes(),
        );
    }
    for item in folder_removals.chain(folder_survivors) {
        hasher.write(
            format!(
                "folder|{}|{}|{}|{}\n",
                item.role,
                item.folder.member_id,
                normalize_path(&item.folder.path),
                item.folder.verified_fingerprint
            )
            .as_bytes(),
        );
    }
    format!("{:016x}", hasher.finish())
}
