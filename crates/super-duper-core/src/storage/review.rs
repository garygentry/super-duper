use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use thiserror::Error;

use super::models::{
    ReviewDecisionKind, ReviewDecisionMutation, ReviewFolderDecisionMutation,
    ReviewFolderGroupPage, ReviewFolderGroupSummary, ReviewGroupPage, ReviewGroupSummary,
    ReviewPlan, ReviewPlanSummary, ReviewPlanView,
};
use super::Database;

#[derive(Debug, Error)]
pub enum ReviewError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("scan run {run_id} was not found")]
    RunNotFound { run_id: i64 },
    #[error("scan run {run_id} is {status}; review requires a completed run")]
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
    #[error("operation id {operation_id} was already used for a different review command")]
    IdempotencyConflict { operation_id: String },
    #[error("removing file {file_id} would leave duplicate group {group_id} without an independently accessible physical copy")]
    UnsafeRemoval { group_id: i64, file_id: i64 },
    #[error("exact-folder group {folder_group_id} was not found in run {run_id}")]
    FolderGroupNotFound { run_id: i64, folder_group_id: i64 },
    #[error("folder copy {folder_member_id} is not a visible member of exact-folder group {folder_group_id} in run {run_id}")]
    FolderMemberNotFound {
        run_id: i64,
        folder_group_id: i64,
        folder_member_id: i64,
    },
    #[error(
        "review decisions overlap between {first_kind} {first_id} and {second_kind} {second_id}"
    )]
    Overlap {
        first_kind: String,
        first_id: i64,
        second_kind: String,
        second_id: i64,
    },
    #[error("review decisions would leave duplicate-file group {duplicate_group_id} without an independently accessible physical copy")]
    UnsafePhysicalRemoval { duplicate_group_id: i64 },
    #[error("review decisions would leave exact-folder group {folder_group_id} without an intact independently accessible copy")]
    UnsafeFolderRemoval { folder_group_id: i64 },
    #[error("run {run_id} is locked by recycle operation {operation_id}")]
    OperationLocked { run_id: i64, operation_id: i64 },
}

impl Database {
    pub fn get_review_plan_view(&self, run_id: i64) -> Result<ReviewPlanView, ReviewError> {
        self.ensure_reviewable_run(run_id)?;
        let plan = self.active_review_plan(run_id)?;
        let summary = self.review_plan_summary(run_id, plan.as_ref().map(|value| value.id))?;
        Ok(ReviewPlanView { plan, summary })
    }

    pub fn get_review_group_view(
        &self,
        run_id: i64,
        group_id: i64,
    ) -> Result<(Option<ReviewPlan>, ReviewGroupSummary), ReviewError> {
        self.ensure_reviewable_run(run_id)?;
        if !self.duplicate_file_group_exists(run_id, group_id)? {
            return Err(ReviewError::GroupNotFound { run_id, group_id });
        }
        let plan = self.active_review_plan(run_id)?;
        let summary = self.review_group_summary(group_id, plan.as_ref().map(|value| value.id))?;
        Ok((plan, summary))
    }

    pub fn page_review_groups(
        &self,
        run_id: i64,
        limit: i64,
        after_group_id: Option<i64>,
    ) -> Result<ReviewGroupPage, ReviewError> {
        self.ensure_reviewable_run(run_id)?;
        let plan = self.active_review_plan(run_id)?;
        let plan_id = plan.as_ref().map(|value| value.id);
        let revision = plan.as_ref().map_or(0, |value| value.revision);
        let total = self.connection().query_row(
            "SELECT COUNT(*) FROM duplicate_group WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        let mut statement = self.connection().prepare(
            "WITH page_groups AS (
                 SELECT id
                 FROM duplicate_group
                 WHERE run_id = ?1 AND id > ?2
                 ORDER BY id
                 LIMIT ?3
             )
             SELECT page_groups.id,
                    COALESCE(SUM(CASE WHEN decision.decision = 'keep' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN decision.decision = 'remove' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN file.id IS NOT NULL
                                           AND (decision.decision IS NULL
                                                OR decision.decision = 'undecided')
                                      THEN 1 ELSE 0 END), 0),
                    COUNT(DISTINCT CASE
                        WHEN decision.decision IS NULL OR decision.decision <> 'remove'
                        THEN CASE
                            WHEN file.file_identity IS NOT NULL AND file.file_identity <> ''
                            THEN 'identity:' || file.file_identity
                            ELSE 'path:' || file.canonical_path
                        END
                    END)
             FROM page_groups
             LEFT JOIN duplicate_group_member member ON member.group_id = page_groups.id
             LEFT JOIN scanned_file file ON file.id = member.file_id
             LEFT JOIN effective_review_decision decision
               ON decision.plan_id = ?4 AND decision.group_id = page_groups.id
              AND decision.file_id = file.id
             GROUP BY page_groups.id
             ORDER BY page_groups.id",
        )?;
        let rows = statement.query_map(
            params![run_id, after_group_id.unwrap_or(0), limit + 1, plan_id],
            |row| {
                Ok(ReviewGroupSummary {
                    group_id: row.get(0)?,
                    keep_count: row.get(1)?,
                    remove_count: row.get(2)?,
                    undecided_count: row.get(3)?,
                    remaining_physical_copy_count: row.get(4)?,
                })
            },
        )?;
        let mut groups = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        let has_more = groups.len() > limit as usize;
        if has_more {
            groups.pop();
        }
        Ok(ReviewGroupPage {
            groups,
            total,
            has_more,
            plan_id,
            revision,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_review_decision(
        &self,
        operation_id: &str,
        run_id: i64,
        group_id: i64,
        file_id: i64,
        decision: ReviewDecisionKind,
        expected_revision: i64,
    ) -> Result<ReviewDecisionMutation, ReviewError> {
        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        let status = tx
            .query_row(
                "SELECT status FROM scan_run WHERE id = ?1",
                params![run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(status) = status else {
            return Err(ReviewError::RunNotFound { run_id });
        };
        if status != "completed" {
            return Err(ReviewError::RunNotCompleted { run_id, status });
        }
        let group_exists = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM duplicate_group WHERE id = ?1 AND run_id = ?2)",
            params![group_id, run_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !group_exists {
            return Err(ReviewError::GroupNotFound { run_id, group_id });
        }
        let snapshot = tx
            .query_row(
                "SELECT file.canonical_path, file.file_identity, file.file_size,
                        file.last_modified, file.content_hash
                 FROM duplicate_group_member member
                 JOIN scanned_file file ON file.id = member.file_id
                 JOIN duplicate_group duplicate_group ON duplicate_group.id = member.group_id
                 WHERE duplicate_group.id = ?1 AND duplicate_group.run_id = ?2 AND file.id = ?3
                   AND file.run_id = duplicate_group.run_id",
                params![group_id, run_id, file_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((canonical_path, file_identity, file_size, last_modified, content_hash)) =
            snapshot
        else {
            return Err(ReviewError::MemberNotFound {
                run_id,
                group_id,
                file_id,
            });
        };

        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO review_plan (run_id, state, revision, created_at, updated_at)
             VALUES (?1, 'active', 0, ?2, ?2)
             ON CONFLICT DO NOTHING",
            params![run_id, now],
        )?;
        let (plan_id, current_revision) = tx.query_row(
            "SELECT id, revision FROM review_plan WHERE run_id = ?1 AND state = 'active'",
            params![run_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;

        let replay = tx
            .query_row(
                "SELECT run_id, group_id, file_id, decision, expected_revision, applied_revision
                 FROM review_command WHERE plan_id = ?1 AND operation_id = ?2",
                params![plan_id, operation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?;
        if let Some((
            stored_run,
            stored_group,
            stored_file,
            stored_decision,
            stored_expected,
            applied,
        )) = replay
        {
            if stored_run != run_id
                || stored_group != group_id
                || stored_file != file_id
                || stored_decision != decision.as_str()
                || stored_expected != expected_revision
            {
                return Err(ReviewError::IdempotencyConflict {
                    operation_id: operation_id.to_owned(),
                });
            }
            tx.commit()?;
            return Ok(ReviewDecisionMutation {
                plan_id,
                applied_revision: applied,
                replayed: true,
                decision,
            });
        }

        if current_revision != expected_revision {
            return Err(ReviewError::StaleRevision {
                expected: expected_revision,
                actual: current_revision,
            });
        }
        ensure_operation_unlocked(&tx, run_id)?;
        if decision == ReviewDecisionKind::Remove {
            let survivors: i64 = tx.query_row(
                "SELECT COUNT(*)
                 FROM (
                     SELECT CASE
                                WHEN file.file_identity IS NOT NULL AND file.file_identity <> ''
                                THEN 'identity:' || file.file_identity
                                ELSE 'path:' || file.canonical_path
                            END AS physical_key
                     FROM duplicate_group_member member
                     JOIN scanned_file file ON file.id = member.file_id
                     LEFT JOIN effective_review_decision existing
                       ON existing.plan_id = ?1 AND existing.group_id = ?2
                      AND existing.file_id = file.id
                     WHERE member.group_id = ?2
                     GROUP BY physical_key
                     HAVING SUM(CASE
                         WHEN file.id = ?3 THEN 0
                         WHEN existing.decision = 'remove' THEN 0
                         ELSE 1
                     END) > 0
                 )",
                params![plan_id, group_id, file_id],
                |row| row.get(0),
            )?;
            if survivors == 0 {
                return Err(ReviewError::UnsafeRemoval { group_id, file_id });
            }
        }

        let applied_revision = current_revision + 1;
        tx.execute(
            "INSERT INTO review_decision
                (plan_id, group_id, file_id, decision, provenance, decided_at,
                 snapshot_canonical_path, snapshot_file_identity, snapshot_file_size,
                 snapshot_last_modified, snapshot_content_hash, manual_revision)
             VALUES (?1, ?2, ?3, ?4, 'manual', ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(plan_id, file_id) DO UPDATE SET
                 group_id = excluded.group_id,
                 decision = excluded.decision,
                 provenance = excluded.provenance,
                 decided_at = excluded.decided_at,
                 snapshot_canonical_path = excluded.snapshot_canonical_path,
                 snapshot_file_identity = excluded.snapshot_file_identity,
                 snapshot_file_size = excluded.snapshot_file_size,
                 snapshot_last_modified = excluded.snapshot_last_modified,
                 snapshot_content_hash = excluded.snapshot_content_hash,
                 manual_revision = excluded.manual_revision",
            params![
                plan_id,
                group_id,
                file_id,
                decision.as_str(),
                now,
                canonical_path,
                file_identity,
                file_size,
                last_modified,
                content_hash,
                applied_revision,
            ],
        )?;
        validate_review_state(&tx, plan_id, run_id)?;
        tx.execute(
            "UPDATE review_plan SET revision = ?1, updated_at = ?2
             WHERE id = ?3 AND revision = ?4",
            params![applied_revision, now, plan_id, current_revision],
        )?;
        tx.execute(
            "INSERT INTO review_command
                (plan_id, operation_id, run_id, group_id, file_id, decision,
                 expected_revision, applied_revision, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                plan_id,
                operation_id,
                run_id,
                group_id,
                file_id,
                decision.as_str(),
                expected_revision,
                applied_revision,
                now,
            ],
        )?;
        tx.commit()?;
        Ok(ReviewDecisionMutation {
            plan_id,
            applied_revision,
            replayed: false,
            decision,
        })
    }

    pub fn get_review_folder_group_view(
        &self,
        run_id: i64,
        folder_group_id: i64,
    ) -> Result<(Option<ReviewPlan>, ReviewFolderGroupSummary), ReviewError> {
        self.ensure_reviewable_run(run_id)?;
        if !self.visible_duplicate_folder_group_exists(run_id, folder_group_id)? {
            return Err(ReviewError::FolderGroupNotFound {
                run_id,
                folder_group_id,
            });
        }
        let plan = self.active_review_plan(run_id)?;
        let summary = review_folder_group_summary_tx(
            self.connection(),
            folder_group_id,
            plan.as_ref().map(|value| value.id),
        )?;
        Ok((plan, summary))
    }

    pub fn page_review_folder_groups(
        &self,
        run_id: i64,
        limit: i64,
        after_group_id: Option<i64>,
    ) -> Result<ReviewFolderGroupPage, ReviewError> {
        self.ensure_reviewable_run(run_id)?;
        let plan = self.active_review_plan(run_id)?;
        let plan_id = plan.as_ref().map(|value| value.id);
        let revision = plan.as_ref().map_or(0, |value| value.revision);
        let total = self.connection().query_row(
            "SELECT COUNT(*) FROM duplicate_folder_group
             WHERE run_id = ?1 AND is_suppressed = 0",
            params![run_id],
            |row| row.get(0),
        )?;
        let has_effective_removals = plan_id.is_some()
            && self.connection().query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM effective_review_decision
                     WHERE plan_id = ?1 AND decision = 'remove'
                     UNION ALL
                     SELECT 1 FROM review_folder_decision
                     WHERE plan_id = ?1 AND decision = 'remove'
                 )",
                params![plan_id],
                |row| row.get::<_, bool>(0),
            )?;
        if !has_effective_removals {
            let mut statement = self.connection().prepare(
                "WITH page_groups(id) AS MATERIALIZED (
                     SELECT id FROM duplicate_folder_group
                     WHERE run_id = ?1 AND is_suppressed = 0 AND id > ?2
                     ORDER BY id LIMIT ?3
                 )
                 SELECT page_groups.id,
                        COALESCE(SUM(CASE WHEN decision.decision = 'keep' THEN 1 ELSE 0 END), 0),
                        0,
                        COALESCE(SUM(CASE WHEN decision.decision IS NULL
                                               OR decision.decision = 'undecided' THEN 1 ELSE 0 END), 0),
                        COUNT(member.id)
                 FROM page_groups
                 JOIN duplicate_folder_group_member member INDEXED BY idx_folder_group_member_group
                   ON member.group_id = page_groups.id
                 LEFT JOIN review_folder_decision decision
                   ON decision.plan_id = ?4 AND decision.folder_member_id = member.id
                 GROUP BY page_groups.id ORDER BY page_groups.id",
            )?;
            let rows = statement.query_map(
                params![run_id, after_group_id.unwrap_or(0), limit + 1, plan_id],
                |row| {
                    Ok(ReviewFolderGroupSummary {
                        folder_group_id: row.get(0)?,
                        keep_count: row.get(1)?,
                        remove_count: row.get(2)?,
                        undecided_count: row.get(3)?,
                        intact_copy_count: row.get(4)?,
                    })
                },
            )?;
            let mut groups = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            let has_more = groups.len() > limit as usize;
            if has_more {
                groups.pop();
            }
            return Ok(ReviewFolderGroupPage {
                groups,
                total,
                has_more,
                plan_id,
                revision,
            });
        }
        let mut statement = self.connection().prepare(
            "WITH RECURSIVE page_groups(id) AS MATERIALIZED (
                 SELECT id FROM duplicate_folder_group
                 WHERE run_id = ?1 AND is_suppressed = 0 AND id > ?2
                 ORDER BY id LIMIT ?3
             ),
             page_members(id, group_id, directory_id) AS MATERIALIZED (
                 SELECT member.id, member.group_id, member.directory_id
                 FROM page_groups
                 JOIN duplicate_folder_group_member member INDEXED BY idx_folder_group_member_group
                   ON member.group_id = page_groups.id
             ),
             removed_directories(directory_id) AS (
                 SELECT directory_id FROM review_folder_decision
                 WHERE plan_id = ?4 AND decision = 'remove'
                 UNION
                 SELECT child.id FROM removed_directories removed
                 JOIN directory_node child ON child.parent_id = removed.directory_id
                 WHERE child.run_id = ?1
             ),
             removed_files(file_id) AS (
                 SELECT file_id FROM effective_review_decision
                 WHERE plan_id = ?4 AND decision = 'remove'
                 UNION
                 SELECT file.id FROM removed_directories removed
                 JOIN directory_node directory ON directory.id = removed.directory_id
                 JOIN scanned_file file
                   ON file.run_id = ?1
                  AND file.parent_dir = directory.path COLLATE UNICODE_NOCASE
             ),
             folder_tree(folder_member_id, directory_id) AS (
                 SELECT id, directory_id FROM page_members
                 UNION ALL
                 SELECT tree.folder_member_id, child.id
                 FROM folder_tree tree
                 JOIN directory_node child ON child.parent_id = tree.directory_id
                 WHERE child.run_id = ?1
             ),
             copy_state AS (
                 SELECT member.id,
                        MAX(CASE WHEN removed.file_id IS NOT NULL THEN 1 ELSE 0 END) AS changed
                 FROM page_members member
                 JOIN folder_tree tree ON tree.folder_member_id = member.id
                 JOIN directory_node directory ON directory.id = tree.directory_id
                 LEFT JOIN scanned_file file
                   ON file.run_id = ?1
                  AND file.parent_dir = directory.path COLLATE UNICODE_NOCASE
                 LEFT JOIN removed_files removed ON removed.file_id = file.id
                 GROUP BY member.id
             )
             SELECT page_groups.id,
                    COALESCE(SUM(CASE WHEN decision.decision = 'keep' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN decision.decision = 'remove' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN decision.decision IS NULL
                                           OR decision.decision = 'undecided' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN copy_state.changed = 0 THEN 1 ELSE 0 END), 0)
             FROM page_groups
             JOIN page_members member ON member.group_id = page_groups.id
             LEFT JOIN review_folder_decision decision
               ON decision.plan_id = ?4 AND decision.folder_member_id = member.id
             LEFT JOIN copy_state ON copy_state.id = member.id
             GROUP BY page_groups.id ORDER BY page_groups.id",
        )?;
        let rows = statement.query_map(
            params![run_id, after_group_id.unwrap_or(0), limit + 1, plan_id],
            |row| {
                Ok(ReviewFolderGroupSummary {
                    folder_group_id: row.get(0)?,
                    keep_count: row.get(1)?,
                    remove_count: row.get(2)?,
                    undecided_count: row.get(3)?,
                    intact_copy_count: row.get(4)?,
                })
            },
        )?;
        let mut groups = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        let has_more = groups.len() > limit as usize;
        if has_more {
            groups.pop();
        }
        Ok(ReviewFolderGroupPage {
            groups,
            total,
            has_more,
            plan_id,
            revision,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_review_folder_decision(
        &self,
        operation_id: &str,
        run_id: i64,
        folder_group_id: i64,
        folder_member_id: i64,
        decision: ReviewDecisionKind,
        expected_revision: i64,
    ) -> Result<ReviewFolderDecisionMutation, ReviewError> {
        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        let status = tx
            .query_row(
                "SELECT status FROM scan_run WHERE id = ?1",
                params![run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(status) = status else {
            return Err(ReviewError::RunNotFound { run_id });
        };
        if status != "completed" {
            return Err(ReviewError::RunNotCompleted { run_id, status });
        }
        let group_exists = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM duplicate_folder_group
                           WHERE id = ?1 AND run_id = ?2 AND is_suppressed = 0)",
            params![folder_group_id, run_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !group_exists {
            return Err(ReviewError::FolderGroupNotFound {
                run_id,
                folder_group_id,
            });
        }
        let snapshot = tx
            .query_row(
                "SELECT member.directory_id, directory.path, folder.total_size,
                        folder.file_count, folder.structural_fingerprint,
                        folder.verified_fingerprint
                 FROM duplicate_folder_group_member member
                 JOIN duplicate_folder_group folder ON folder.id = member.group_id
                 JOIN directory_node directory ON directory.id = member.directory_id
                 WHERE member.id = ?1 AND folder.id = ?2 AND folder.run_id = ?3
                   AND folder.is_suppressed = 0 AND directory.run_id = folder.run_id",
                params![folder_member_id, folder_group_id, run_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((directory_id, path, total_size, file_count, structural, verified)) = snapshot
        else {
            return Err(ReviewError::FolderMemberNotFound {
                run_id,
                folder_group_id,
                folder_member_id,
            });
        };

        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO review_plan (run_id, state, revision, created_at, updated_at)
             VALUES (?1, 'active', 0, ?2, ?2) ON CONFLICT DO NOTHING",
            params![run_id, now],
        )?;
        let (plan_id, current_revision) = tx.query_row(
            "SELECT id, revision FROM review_plan WHERE run_id = ?1 AND state = 'active'",
            params![run_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        let replay = tx
            .query_row(
                "SELECT run_id, folder_group_id, folder_member_id, decision,
                        expected_revision, applied_revision
                 FROM review_folder_command WHERE plan_id = ?1 AND operation_id = ?2",
                params![plan_id, operation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?;
        if let Some((
            stored_run,
            stored_group,
            stored_member,
            stored_decision,
            stored_expected,
            applied,
        )) = replay
        {
            if stored_run != run_id
                || stored_group != folder_group_id
                || stored_member != folder_member_id
                || stored_decision != decision.as_str()
                || stored_expected != expected_revision
            {
                return Err(ReviewError::IdempotencyConflict {
                    operation_id: operation_id.to_owned(),
                });
            }
            tx.commit()?;
            return Ok(ReviewFolderDecisionMutation {
                plan_id,
                applied_revision: applied,
                replayed: true,
                decision,
            });
        }
        if current_revision != expected_revision {
            return Err(ReviewError::StaleRevision {
                expected: expected_revision,
                actual: current_revision,
            });
        }
        ensure_operation_unlocked(&tx, run_id)?;
        tx.execute(
            "INSERT INTO review_folder_decision
                (plan_id, folder_group_id, folder_member_id, directory_id, decision,
                 provenance, decided_at, snapshot_path, snapshot_total_size,
                 snapshot_file_count, snapshot_structural_fingerprint,
                 snapshot_verified_fingerprint)
             VALUES (?1, ?2, ?3, ?4, ?5, 'manual', ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(plan_id, folder_member_id) DO UPDATE SET
                 folder_group_id = excluded.folder_group_id,
                 directory_id = excluded.directory_id,
                 decision = excluded.decision,
                 provenance = excluded.provenance,
                 decided_at = excluded.decided_at,
                 snapshot_path = excluded.snapshot_path,
                 snapshot_total_size = excluded.snapshot_total_size,
                 snapshot_file_count = excluded.snapshot_file_count,
                 snapshot_structural_fingerprint = excluded.snapshot_structural_fingerprint,
                 snapshot_verified_fingerprint = excluded.snapshot_verified_fingerprint",
            params![
                plan_id,
                folder_group_id,
                folder_member_id,
                directory_id,
                decision.as_str(),
                now,
                path,
                total_size,
                file_count,
                structural,
                verified,
            ],
        )?;
        validate_review_state(&tx, plan_id, run_id)?;
        let applied_revision = current_revision + 1;
        tx.execute(
            "UPDATE review_plan SET revision = ?1, updated_at = ?2
             WHERE id = ?3 AND revision = ?4",
            params![applied_revision, now, plan_id, current_revision],
        )?;
        tx.execute(
            "INSERT INTO review_folder_command
                (plan_id, operation_id, run_id, folder_group_id, folder_member_id,
                 decision, expected_revision, applied_revision, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                plan_id,
                operation_id,
                run_id,
                folder_group_id,
                folder_member_id,
                decision.as_str(),
                expected_revision,
                applied_revision,
                now,
            ],
        )?;
        tx.commit()?;
        Ok(ReviewFolderDecisionMutation {
            plan_id,
            applied_revision,
            replayed: false,
            decision,
        })
    }

    fn visible_duplicate_folder_group_exists(
        &self,
        run_id: i64,
        folder_group_id: i64,
    ) -> rusqlite::Result<bool> {
        self.connection().query_row(
            "SELECT EXISTS(SELECT 1 FROM duplicate_folder_group
                           WHERE run_id = ?1 AND id = ?2 AND is_suppressed = 0)",
            params![run_id, folder_group_id],
            |row| row.get(0),
        )
    }

    fn ensure_reviewable_run(&self, run_id: i64) -> Result<(), ReviewError> {
        let status = self
            .connection()
            .query_row(
                "SELECT status FROM scan_run WHERE id = ?1",
                params![run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match status {
            None => Err(ReviewError::RunNotFound { run_id }),
            Some(status) if status != "completed" => {
                Err(ReviewError::RunNotCompleted { run_id, status })
            }
            Some(_) => Ok(()),
        }
    }

    pub(super) fn active_review_plan(&self, run_id: i64) -> rusqlite::Result<Option<ReviewPlan>> {
        Ok(self
            .connection()
            .query_row(
                "SELECT id, run_id, state, revision, created_at, updated_at
                 FROM review_plan WHERE run_id = ?1 AND state = 'active'",
                params![run_id],
                |row| {
                    Ok(ReviewPlan {
                        id: row.get(0)?,
                        run_id: row.get(1)?,
                        state: row.get(2)?,
                        revision: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    fn review_plan_summary(
        &self,
        run_id: i64,
        plan_id: Option<i64>,
    ) -> Result<ReviewPlanSummary, ReviewError> {
        let mut summary = self.connection().query_row(
            "WITH totals(member_count) AS (
                 SELECT COALESCE(SUM(file_count), 0)
                 FROM duplicate_group WHERE run_id = ?1
             ),
             decisions AS (
                 SELECT decision.group_id, decision.decision
                 FROM effective_review_decision decision
                 JOIN duplicate_group duplicate_group ON duplicate_group.id = decision.group_id
                 WHERE decision.plan_id = ?2 AND duplicate_group.run_id = ?1
             )
             SELECT COUNT(DISTINCT CASE WHEN decision IN ('keep', 'remove') THEN group_id END),
                    COALESCE(SUM(CASE WHEN decision = 'keep' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN decision = 'remove' THEN 1 ELSE 0 END), 0),
                    (SELECT member_count FROM totals)
                      - COALESCE(SUM(CASE WHEN decision IN ('keep', 'remove') THEN 1 ELSE 0 END), 0)
             FROM decisions",
            params![run_id, plan_id],
            |row| {
                Ok(ReviewPlanSummary {
                    decided_group_count: row.get(0)?,
                    keep_count: row.get(1)?,
                    remove_count: row.get(2)?,
                    undecided_count: row.get(3)?,
                    ..ReviewPlanSummary::default()
                })
            },
        )?;
        (
            summary.rule_keep_count,
            summary.rule_remove_count,
            summary.active_rule_application_count,
        ) = self.connection().query_row(
            "SELECT
                 COALESCE(SUM(CASE WHEN decision.decision = 'keep'
                                    AND decision.provenance = 'rule' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN decision.decision = 'remove'
                                    AND decision.provenance = 'rule' THEN 1 ELSE 0 END), 0),
                 (SELECT COUNT(*) FROM review_rule_application
                  WHERE plan_id = ?2 AND run_id = ?1 AND state = 'active')
             FROM effective_review_decision decision
             JOIN duplicate_group duplicate_group ON duplicate_group.id = decision.group_id
             WHERE decision.plan_id = ?2 AND duplicate_group.run_id = ?1",
            params![run_id, plan_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        (
            summary.decided_folder_group_count,
            summary.folder_keep_count,
            summary.folder_remove_count,
            summary.folder_undecided_count,
        ) = self.connection().query_row(
            "WITH totals(member_count) AS (
                 SELECT COALESCE(SUM(folder_count), 0)
                 FROM duplicate_folder_group
                 WHERE run_id = ?1 AND is_suppressed = 0
             ),
             decisions AS (
                 SELECT decision.folder_group_id, decision.decision
                 FROM review_folder_decision decision
                 JOIN duplicate_folder_group folder ON folder.id = decision.folder_group_id
                 WHERE decision.plan_id = ?2
                   AND folder.run_id = ?1
                   AND folder.is_suppressed = 0
             )
             SELECT COUNT(DISTINCT CASE WHEN decision IN ('keep', 'remove')
                                        THEN folder_group_id END),
                    COALESCE(SUM(CASE WHEN decision = 'keep' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN decision = 'remove' THEN 1 ELSE 0 END), 0),
                    (SELECT member_count FROM totals)
                      - COALESCE(SUM(CASE WHEN decision IN ('keep', 'remove') THEN 1 ELSE 0 END), 0)
             FROM decisions",
            params![run_id, plan_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        (
            summary.effective_removal_file_count,
            summary.planned_removal_physical_item_count,
            summary.planned_removal_bytes,
            summary.remaining_physical_copy_count,
        ) = self.connection().query_row(
            "WITH RECURSIVE removed_directories(directory_id) AS (
                 SELECT directory_id FROM review_folder_decision
                 WHERE plan_id = ?2 AND decision = 'remove'
                 UNION
                 SELECT child.id FROM removed_directories removed
                 JOIN directory_node child ON child.parent_id = removed.directory_id
                 WHERE child.run_id = ?1
             ),
             removed_files(file_id) AS (
                 SELECT file_id FROM effective_review_decision
                 WHERE plan_id = ?2 AND decision = 'remove'
                 UNION
                 SELECT file.id FROM removed_directories removed
                 JOIN directory_node directory ON directory.id = removed.directory_id
                 JOIN scanned_file file
                   ON file.run_id = ?1
                  AND file.parent_dir = directory.path COLLATE UNICODE_NOCASE
             ),
             removed_physical AS (
                 SELECT CASE WHEN file.file_identity IS NOT NULL AND file.file_identity <> ''
                             THEN 'identity:' || file.file_identity
                             ELSE 'path:' || file.canonical_path END AS physical_key,
                        MAX(file.file_size) AS physical_size
                 FROM removed_files removed JOIN scanned_file file ON file.id = removed.file_id
                 GROUP BY physical_key
             ),
             survivor_physical AS (
                 SELECT member.group_id,
                        CASE WHEN file.file_identity IS NOT NULL AND file.file_identity <> ''
                             THEN 'identity:' || file.file_identity
                             ELSE 'path:' || file.canonical_path END AS physical_key,
                        MAX(CASE WHEN removed.file_id IS NULL THEN 1 ELSE 0 END) AS survives
                 FROM duplicate_group_member member
                 JOIN duplicate_group duplicate_group ON duplicate_group.id = member.group_id
                 JOIN scanned_file file ON file.id = member.file_id
                 LEFT JOIN removed_files removed ON removed.file_id = file.id
                 WHERE duplicate_group.run_id = ?1
                 GROUP BY member.group_id, physical_key
             )
             SELECT (SELECT COUNT(*) FROM removed_files),
                    (SELECT COUNT(*) FROM removed_physical),
                    (SELECT COALESCE(SUM(physical_size), 0) FROM removed_physical),
                    (SELECT COALESCE(SUM(survives), 0) FROM survivor_physical)",
            params![run_id, plan_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let total_folder_copy_count = summary.folder_keep_count
            + summary.folder_remove_count
            + summary.folder_undecided_count;
        if summary.remove_count == 0 && summary.folder_remove_count == 0 {
            summary.intact_folder_copy_count = total_folder_copy_count;
            return Ok(summary);
        }
        let affected_folder_copy_count: i64 = self.connection().query_row(
            "WITH RECURSIVE removed_folder_ancestors(directory_id) AS (
                 SELECT directory_id FROM review_folder_decision
                 WHERE plan_id = ?2 AND decision = 'remove'
                 UNION
                 SELECT parent.id FROM removed_folder_ancestors removed
                 JOIN directory_node child ON child.id = removed.directory_id
                 JOIN directory_node parent ON parent.id = child.parent_id
                 WHERE parent.run_id = ?1
             ),
             removed_folder_descendants(directory_id) AS (
                 SELECT directory_id FROM review_folder_decision
                 WHERE plan_id = ?2 AND decision = 'remove'
                 UNION
                 SELECT child.id FROM removed_folder_descendants removed
                 JOIN directory_node child ON child.parent_id = removed.directory_id
                 WHERE child.run_id = ?1
             ),
             removed_file_ancestors(directory_id) AS (
                 SELECT directory.id
                 FROM effective_review_decision decision
                 JOIN scanned_file file ON file.id = decision.file_id AND file.run_id = ?1
                 JOIN directory_node directory
                   ON directory.run_id = ?1
                  AND directory.path = file.parent_dir COLLATE UNICODE_NOCASE
                 WHERE decision.plan_id = ?2 AND decision.decision = 'remove'
                 UNION
                 SELECT parent.id FROM removed_file_ancestors removed
                 JOIN directory_node child ON child.id = removed.directory_id
                 JOIN directory_node parent ON parent.id = child.parent_id
                 WHERE parent.run_id = ?1
             ),
             affected_members(folder_member_id) AS (
                 SELECT member.id FROM duplicate_folder_group_member member
                 JOIN removed_folder_ancestors removed ON removed.directory_id = member.directory_id
                 UNION
                 SELECT member.id FROM duplicate_folder_group_member member
                 JOIN removed_folder_descendants removed ON removed.directory_id = member.directory_id
                 UNION
                 SELECT member.id FROM duplicate_folder_group_member member
                 JOIN removed_file_ancestors removed ON removed.directory_id = member.directory_id
             ),
             affected_visible(folder_member_id) AS (
                 SELECT DISTINCT affected.folder_member_id
                 FROM affected_members affected
                 JOIN duplicate_folder_group_member member ON member.id = affected.folder_member_id
                 JOIN duplicate_folder_group folder ON folder.id = member.group_id
                 WHERE folder.run_id = ?1 AND folder.is_suppressed = 0
             )
             SELECT COUNT(*) FROM affected_visible",
            params![run_id, plan_id],
            |row| row.get(0),
        )?;
        summary.intact_folder_copy_count = total_folder_copy_count - affected_folder_copy_count;
        Ok(summary)
    }

    pub(super) fn review_group_summary(
        &self,
        group_id: i64,
        plan_id: Option<i64>,
    ) -> rusqlite::Result<ReviewGroupSummary> {
        let summary = self.connection().query_row(
            "WITH RECURSIVE run_context(run_id) AS (
                 SELECT run_id FROM duplicate_group WHERE id = ?1
             ),
             removed_directories(directory_id) AS (
                 SELECT directory_id FROM review_folder_decision
                 WHERE plan_id = ?2 AND decision = 'remove'
                 UNION
                 SELECT child.id FROM removed_directories removed
                 JOIN directory_node child ON child.parent_id = removed.directory_id
                 JOIN run_context ON child.run_id = run_context.run_id
             ),
             folder_removed_files(file_id) AS (
                 SELECT file.id FROM removed_directories removed
                 JOIN directory_node directory ON directory.id = removed.directory_id
                 JOIN run_context
                 JOIN scanned_file file
                   ON file.run_id = run_context.run_id
                  AND file.parent_dir = directory.path COLLATE UNICODE_NOCASE
             )
             SELECT ?1,
                    COALESCE(SUM(CASE WHEN decision.decision = 'keep' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN decision.decision = 'remove' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN decision.decision IS NULL OR decision.decision = 'undecided'
                                      THEN 1 ELSE 0 END), 0),
                    COUNT(DISTINCT CASE
                        WHEN (decision.decision IS NULL OR decision.decision <> 'remove')
                             AND folder_removed.file_id IS NULL
                        THEN CASE
                            WHEN file.file_identity IS NOT NULL AND file.file_identity <> ''
                            THEN 'identity:' || file.file_identity
                            ELSE 'path:' || file.canonical_path
                        END
                    END)
             FROM duplicate_group_member member
             JOIN scanned_file file ON file.id = member.file_id
             LEFT JOIN effective_review_decision decision
               ON decision.plan_id = ?2 AND decision.group_id = ?1 AND decision.file_id = file.id
             LEFT JOIN folder_removed_files folder_removed ON folder_removed.file_id = file.id
             WHERE member.group_id = ?1",
            params![group_id, plan_id],
            |row| {
                Ok(ReviewGroupSummary {
                    group_id: row.get(0)?,
                    keep_count: row.get(1)?,
                    remove_count: row.get(2)?,
                    undecided_count: row.get(3)?,
                    remaining_physical_copy_count: row.get(4)?,
                })
            },
        )?;
        Ok(summary)
    }
}

fn ensure_operation_unlocked(tx: &Transaction<'_>, run_id: i64) -> Result<(), ReviewError> {
    let operation = tx
        .query_row(
            "SELECT id, status FROM recycle_operation
             WHERE run_id = ?1 AND status IN
                ('prepared', 'awaiting_confirmation', 'submitted', 'executing', 'cancelling', 'recovery_required')
             ORDER BY id DESC LIMIT 1",
            params![run_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    if let Some((operation_id, status)) = operation {
        if status == "prepared" || status == "awaiting_confirmation" {
            tx.execute(
                "UPDATE recycle_operation SET status = 'expired', completed_at = ?1,
                        error_code = 'review_changed',
                        error_detail = 'Review mutation invalidated the unsubmitted operation intent'
                 WHERE id = ?2",
                params![Utc::now().to_rfc3339(), operation_id],
            )?;
            return Ok(());
        }
        return Err(ReviewError::OperationLocked {
            run_id,
            operation_id,
        });
    }
    Ok(())
}

pub(super) fn validate_review_state(
    tx: &Transaction<'_>,
    plan_id: i64,
    run_id: i64,
) -> Result<(), ReviewError> {
    let file_folder_overlap = tx
        .query_row(
            "WITH RECURSIVE folder_tree(folder_member_id, folder_decision, directory_id) AS (
                 SELECT folder_member_id, decision, directory_id
                 FROM review_folder_decision
                 WHERE plan_id = ?1 AND decision IN ('keep', 'remove')
                 UNION ALL
                 SELECT tree.folder_member_id, tree.folder_decision, child.id
                 FROM folder_tree tree
                 JOIN directory_node child ON child.parent_id = tree.directory_id
                 WHERE child.run_id = ?2
             )
             SELECT file_decision.file_id, folder_tree.folder_member_id,
                    file_decision.decision, folder_tree.folder_decision
             FROM effective_review_decision file_decision
             JOIN scanned_file file ON file.id = file_decision.file_id AND file.run_id = ?2
             JOIN directory_node parent
               ON parent.run_id = ?2
              AND parent.path = file.parent_dir COLLATE UNICODE_NOCASE
             JOIN folder_tree ON folder_tree.directory_id = parent.id
             WHERE file_decision.plan_id = ?1
               AND file_decision.decision IN ('keep', 'remove')
               AND (folder_tree.folder_decision = 'remove'
                    OR file_decision.decision = 'remove')
             LIMIT 1",
            params![plan_id, run_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    if let Some((file_id, folder_member_id, file_decision, folder_decision)) = file_folder_overlap {
        return Err(ReviewError::Overlap {
            first_kind: format!("file-{file_decision}"),
            first_id: file_id,
            second_kind: format!("folder-{folder_decision}"),
            second_id: folder_member_id,
        });
    }

    let folder_overlap = tx
        .query_row(
            "WITH RECURSIVE folder_tree(root_decision_id, root_member_id,
                                        root_decision, directory_id) AS (
                 SELECT id, folder_member_id, decision, directory_id
                 FROM review_folder_decision
                 WHERE plan_id = ?1 AND decision IN ('keep', 'remove')
                 UNION ALL
                 SELECT tree.root_decision_id, tree.root_member_id,
                        tree.root_decision, child.id
                 FROM folder_tree tree
                 JOIN directory_node child ON child.parent_id = tree.directory_id
                 WHERE child.run_id = ?2
             )
             SELECT tree.root_member_id, nested.folder_member_id,
                    tree.root_decision, nested.decision
             FROM folder_tree tree
             JOIN review_folder_decision nested
               ON nested.plan_id = ?1 AND nested.directory_id = tree.directory_id
              AND nested.id <> tree.root_decision_id
             WHERE nested.decision IN ('keep', 'remove')
               AND (tree.root_decision = 'remove' OR nested.decision = 'remove')
             LIMIT 1",
            params![plan_id, run_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    if let Some((first_id, second_id, first_decision, second_decision)) = folder_overlap {
        return Err(ReviewError::Overlap {
            first_kind: format!("folder-{first_decision}"),
            first_id,
            second_kind: format!("folder-{second_decision}"),
            second_id,
        });
    }

    let unsafe_file_group = tx
        .query_row(
            "WITH RECURSIVE removed_directories(directory_id) AS (
                 SELECT directory_id FROM review_folder_decision
                 WHERE plan_id = ?1 AND decision = 'remove'
                 UNION
                 SELECT child.id FROM removed_directories removed
                 JOIN directory_node child ON child.parent_id = removed.directory_id
                 WHERE child.run_id = ?2
             ),
             removed_files(file_id) AS (
                 SELECT file_id FROM effective_review_decision
                 WHERE plan_id = ?1 AND decision = 'remove'
                 UNION
                 SELECT file.id
                 FROM removed_directories removed
                 JOIN directory_node directory ON directory.id = removed.directory_id
                 JOIN scanned_file file
                   ON file.run_id = ?2
                  AND file.parent_dir = directory.path COLLATE UNICODE_NOCASE
             ),
             physical_state AS (
                 SELECT member.group_id,
                        CASE WHEN file.file_identity IS NOT NULL AND file.file_identity <> ''
                             THEN 'identity:' || file.file_identity
                             ELSE 'path:' || file.canonical_path END AS physical_key,
                        MAX(CASE WHEN removed.file_id IS NULL THEN 1 ELSE 0 END) AS survives
                 FROM duplicate_group_member member
                 JOIN duplicate_group duplicate_group ON duplicate_group.id = member.group_id
                 JOIN scanned_file file ON file.id = member.file_id
                 LEFT JOIN removed_files removed ON removed.file_id = file.id
                 WHERE duplicate_group.run_id = ?2
                 GROUP BY member.group_id, physical_key
             )
             SELECT group_id FROM physical_state
             GROUP BY group_id HAVING SUM(survives) = 0 LIMIT 1",
            params![plan_id, run_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if let Some(duplicate_group_id) = unsafe_file_group {
        return Err(ReviewError::UnsafePhysicalRemoval { duplicate_group_id });
    }

    let unsafe_folder_group = tx
        .query_row(
            "WITH RECURSIVE removed_directories(directory_id) AS (
                 SELECT directory_id FROM review_folder_decision
                 WHERE plan_id = ?1 AND decision = 'remove'
                 UNION
                 SELECT child.id FROM removed_directories removed
                 JOIN directory_node child ON child.parent_id = removed.directory_id
                 WHERE child.run_id = ?2
             ),
             removed_files(file_id) AS (
                 SELECT file_id FROM effective_review_decision
                 WHERE plan_id = ?1 AND decision = 'remove'
                 UNION
                 SELECT file.id
                 FROM removed_directories removed
                 JOIN directory_node directory ON directory.id = removed.directory_id
                 JOIN scanned_file file
                   ON file.run_id = ?2
                  AND file.parent_dir = directory.path COLLATE UNICODE_NOCASE
             ),
             folder_tree(folder_member_id, directory_id) AS (
                 SELECT member.id, member.directory_id
                 FROM duplicate_folder_group_member member
                 JOIN duplicate_folder_group folder ON folder.id = member.group_id
                 WHERE folder.run_id = ?2
                 UNION ALL
                 SELECT tree.folder_member_id, child.id
                 FROM folder_tree tree
                 JOIN directory_node child ON child.parent_id = tree.directory_id
                 WHERE child.run_id = ?2
             ),
             copy_state AS (
                 SELECT member.group_id, member.id,
                        MAX(CASE WHEN removed.file_id IS NOT NULL THEN 1 ELSE 0 END) AS changed
                 FROM duplicate_folder_group_member member
                 JOIN duplicate_folder_group folder ON folder.id = member.group_id
                 JOIN folder_tree tree ON tree.folder_member_id = member.id
                 JOIN directory_node directory ON directory.id = tree.directory_id
                 LEFT JOIN scanned_file file
                   ON file.run_id = ?2
                  AND file.parent_dir = directory.path COLLATE UNICODE_NOCASE
                 LEFT JOIN removed_files removed ON removed.file_id = file.id
                 WHERE folder.run_id = ?2
                 GROUP BY member.group_id, member.id
             )
             SELECT group_id FROM copy_state
             GROUP BY group_id
             HAVING SUM(CASE WHEN changed = 0 THEN 1 ELSE 0 END) = 0
             LIMIT 1",
            params![plan_id, run_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if let Some(folder_group_id) = unsafe_folder_group {
        return Err(ReviewError::UnsafeFolderRemoval { folder_group_id });
    }
    Ok(())
}

pub(super) fn review_folder_group_summary_tx(
    connection: &rusqlite::Connection,
    folder_group_id: i64,
    plan_id: Option<i64>,
) -> rusqlite::Result<ReviewFolderGroupSummary> {
    connection.query_row(
        "WITH RECURSIVE run_context(run_id) AS (
             SELECT run_id FROM duplicate_folder_group WHERE id = ?1
         ),
         removed_directories(directory_id) AS (
             SELECT directory_id FROM review_folder_decision
             WHERE plan_id = ?2 AND decision = 'remove'
             UNION
             SELECT child.id FROM removed_directories removed
             JOIN directory_node child ON child.parent_id = removed.directory_id
             JOIN run_context ON child.run_id = run_context.run_id
         ),
         removed_files(file_id) AS (
             SELECT file_id FROM effective_review_decision
             WHERE plan_id = ?2 AND decision = 'remove'
             UNION
             SELECT file.id
             FROM removed_directories removed
             JOIN directory_node directory ON directory.id = removed.directory_id
             JOIN run_context
             JOIN scanned_file file
               ON file.run_id = run_context.run_id
              AND file.parent_dir = directory.path COLLATE UNICODE_NOCASE
         ),
         folder_tree(folder_member_id, directory_id) AS (
             SELECT id, directory_id FROM duplicate_folder_group_member WHERE group_id = ?1
             UNION ALL
             SELECT tree.folder_member_id, child.id
             FROM folder_tree tree
             JOIN directory_node child ON child.parent_id = tree.directory_id
             JOIN run_context ON child.run_id = run_context.run_id
         ),
         copy_state AS (
             SELECT member.id,
                    MAX(CASE WHEN removed.file_id IS NOT NULL THEN 1 ELSE 0 END) AS changed
             FROM duplicate_folder_group_member member
             JOIN folder_tree tree ON tree.folder_member_id = member.id
             JOIN directory_node directory ON directory.id = tree.directory_id
             JOIN run_context
             LEFT JOIN scanned_file file
               ON file.run_id = run_context.run_id
              AND file.parent_dir = directory.path COLLATE UNICODE_NOCASE
             LEFT JOIN removed_files removed ON removed.file_id = file.id
             WHERE member.group_id = ?1
             GROUP BY member.id
         )
         SELECT ?1,
                COALESCE(SUM(CASE WHEN decision.decision = 'keep' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN decision.decision = 'remove' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN decision.decision IS NULL OR decision.decision = 'undecided'
                                  THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN copy_state.changed = 0 THEN 1 ELSE 0 END), 0)
         FROM duplicate_folder_group_member member
         LEFT JOIN review_folder_decision decision
           ON decision.plan_id = ?2 AND decision.folder_member_id = member.id
         LEFT JOIN copy_state ON copy_state.id = member.id
         WHERE member.group_id = ?1",
        params![folder_group_id, plan_id],
        |row| {
            Ok(ReviewFolderGroupSummary {
                folder_group_id: row.get(0)?,
                keep_count: row.get(1)?,
                remove_count: row.get(2)?,
                undecided_count: row.get(3)?,
                intact_copy_count: row.get(4)?,
            })
        },
    )
}
