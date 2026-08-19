use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use thiserror::Error;

use super::models::{
    ReviewDecisionKind, ReviewDecisionMutation, ReviewGroupPage, ReviewGroupSummary, ReviewPlan,
    ReviewPlanSummary, ReviewPlanView,
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
             LEFT JOIN review_decision decision
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
                     LEFT JOIN review_decision existing
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

        tx.execute(
            "INSERT INTO review_decision
                (plan_id, group_id, file_id, decision, provenance, decided_at,
                 snapshot_canonical_path, snapshot_file_identity, snapshot_file_size,
                 snapshot_last_modified, snapshot_content_hash)
             VALUES (?1, ?2, ?3, ?4, 'manual', ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(plan_id, file_id) DO UPDATE SET
                 group_id = excluded.group_id,
                 decision = excluded.decision,
                 provenance = excluded.provenance,
                 decided_at = excluded.decided_at,
                 snapshot_canonical_path = excluded.snapshot_canonical_path,
                 snapshot_file_identity = excluded.snapshot_file_identity,
                 snapshot_file_size = excluded.snapshot_file_size,
                 snapshot_last_modified = excluded.snapshot_last_modified,
                 snapshot_content_hash = excluded.snapshot_content_hash",
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
            ],
        )?;
        let applied_revision = current_revision + 1;
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
        let summary = self.connection().query_row(
            "WITH member_state AS (
                 SELECT duplicate_group.id AS group_id, file.file_size, file.file_identity,
                        file.canonical_path, decision.decision
                 FROM duplicate_group
                 JOIN duplicate_group_member member ON member.group_id = duplicate_group.id
                 JOIN scanned_file file ON file.id = member.file_id
                 LEFT JOIN review_decision decision
                   ON decision.plan_id = ?2 AND decision.group_id = duplicate_group.id
                  AND decision.file_id = file.id
                 WHERE duplicate_group.run_id = ?1
             ),
             physical_state AS (
                 SELECT group_id,
                        CASE WHEN file_identity IS NOT NULL AND file_identity <> ''
                             THEN 'identity:' || file_identity ELSE 'path:' || canonical_path END
                             AS physical_key,
                        MAX(CASE WHEN decision IS NULL OR decision <> 'remove' THEN 1 ELSE 0 END)
                             AS survives
                 FROM member_state
                 GROUP BY group_id, physical_key
             )
             SELECT
                 COUNT(DISTINCT CASE WHEN decision IN ('keep', 'remove') THEN group_id END),
                 COALESCE(SUM(CASE WHEN decision = 'keep' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN decision = 'remove' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN decision IS NULL OR decision = 'undecided' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN decision = 'remove' THEN file_size ELSE 0 END), 0),
                 (SELECT COALESCE(SUM(survives), 0) FROM physical_state)
             FROM member_state",
            params![run_id, plan_id],
            |row| {
                Ok(ReviewPlanSummary {
                    decided_group_count: row.get(0)?,
                    keep_count: row.get(1)?,
                    remove_count: row.get(2)?,
                    undecided_count: row.get(3)?,
                    planned_removal_bytes: row.get(4)?,
                    remaining_physical_copy_count: row.get(5)?,
                })
            },
        )?;
        Ok(summary)
    }

    pub(super) fn review_group_summary(
        &self,
        group_id: i64,
        plan_id: Option<i64>,
    ) -> rusqlite::Result<ReviewGroupSummary> {
        let summary = self.connection().query_row(
            "SELECT ?1,
                    COALESCE(SUM(CASE WHEN decision.decision = 'keep' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN decision.decision = 'remove' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN decision.decision IS NULL OR decision.decision = 'undecided'
                                      THEN 1 ELSE 0 END), 0),
                    COUNT(DISTINCT CASE
                        WHEN decision.decision IS NULL OR decision.decision <> 'remove'
                        THEN CASE
                            WHEN file.file_identity IS NOT NULL AND file.file_identity <> ''
                            THEN 'identity:' || file.file_identity
                            ELSE 'path:' || file.canonical_path
                        END
                    END)
             FROM duplicate_group_member member
             JOIN scanned_file file ON file.id = member.file_id
             LEFT JOIN review_decision decision
               ON decision.plan_id = ?2 AND decision.group_id = ?1 AND decision.file_id = file.id
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
