use super::models::*;
use super::queries::duplicate_file_group_predicate;
use super::Database;
use chrono::Utc;
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, OptionalExtension, Transaction, TransactionBehavior};
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use twox_hash::XxHash64;

const MAX_PREVIEW_GROUPS: usize = 100_000;
const MAX_PREVIEW_LOGICAL_PATHS: usize = 500_000;

#[derive(Debug, thiserror::Error)]
pub enum PreferenceError {
    #[error("run {run_id} was not found")]
    RunNotFound { run_id: i64 },
    #[error("run {run_id} is {status}, not completed")]
    RunNotCompleted { run_id: i64, status: String },
    #[error("preference rule {rule_id} was not found")]
    RuleNotFound { rule_id: i64 },
    #[error("preference rule {rule_id} is archived")]
    RuleArchived { rule_id: i64 },
    #[error("preference rule revision changed")]
    StaleRuleRevision {
        rule_id: i64,
        expected: i64,
        current: i64,
    },
    #[error("review plan revision changed")]
    StaleReviewRevision { expected: i64, current: i64 },
    #[error("operation id was already used with another preference-rule payload")]
    IdempotencyConflict { operation_id: String },
    #[error("preference rule name already exists")]
    DuplicateName { name: String },
    #[error("duplicate group {group_id} does not belong to run {run_id}")]
    InvalidSelectedGroup { run_id: i64, group_id: i64 },
    #[error("invalid preference rule: {message}")]
    InvalidRule { message: String },
    #[error("preference preview scope exceeds the bounded complexity limit")]
    PreviewTooComplex {
        scoped_group_count: usize,
        maximum_group_count: usize,
        scoped_logical_path_count: Option<usize>,
        maximum_logical_path_count: usize,
    },
    #[error("preview signature does not match the submitted rule, review revision, and scope")]
    PreviewConflict,
    #[error("the rule application contains no applicable decisions")]
    ApplicationEmpty,
    #[error("file {file_id} is already owned by active rule application {application_id}")]
    ApplicationOverlap { file_id: i64, application_id: i64 },
    #[error("rule application {application_id} was not found in run {run_id}")]
    ApplicationNotFound { run_id: i64, application_id: i64 },
    #[error("rule application {application_id} is already reversed")]
    ApplicationAlreadyReversed { application_id: i64 },
    #[error("run {run_id} is locked by recycle operation {operation_id}")]
    OperationLocked { run_id: i64, operation_id: i64 },
    #[error(transparent)]
    Review(#[from] super::review::ReviewError),
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
struct PreviewMember {
    file_id: i64,
    group_id: i64,
    root_path: String,
    physical_key: String,
    file_size: i64,
    parent_directory_id: Option<i64>,
    effective_decision: ReviewDecisionKind,
    provenance: Option<String>,
    canonical_path: String,
    file_identity: Option<String>,
    last_modified: i64,
    content_hash: Option<i64>,
}

#[derive(Debug, Default)]
struct DirectoryReviewState {
    parents: HashMap<i64, Option<i64>>,
    keep_roots: HashMap<i64, i64>,
    remove_roots: HashMap<i64, i64>,
    folder_groups: HashMap<i64, Vec<(i64, i64)>>,
}

#[derive(Debug)]
struct RuleDecisionProposal {
    member: PreviewMember,
    decision: ReviewDecisionKind,
    explanation_code: String,
    preferred_rank: i64,
}

impl Database {
    pub fn list_preference_rules(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<(Vec<PreferenceRuleSummary>, i64), PreferenceError> {
        let total = self.connection().query_row(
            "SELECT COUNT(*) FROM preference_rule WHERE state = 'active'",
            [],
            |row| row.get(0),
        )?;
        let mut statement = self.connection().prepare(
            "SELECT rule.id, rule.name, rule.kind, rule.revision, COUNT(root.ordinal),
                    rule.updated_at
             FROM preference_rule rule
             LEFT JOIN preference_rule_root root ON root.rule_id = rule.id
             WHERE rule.state = 'active'
             GROUP BY rule.id
             ORDER BY rule.name COLLATE UNICODE_NOCASE, rule.id
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = statement.query_map(params![limit, offset], |row| {
            Ok(PreferenceRuleSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: row.get(2)?,
                revision: row.get(3)?,
                root_count: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        Ok((rows.collect::<Result<Vec<_>, _>>()?, total))
    }

    pub fn get_preference_rule(&self, rule_id: i64) -> Result<PreferenceRule, PreferenceError> {
        let mut rule = self
            .connection()
            .query_row(
                "SELECT id, name, kind, state, revision, created_at, updated_at
                 FROM preference_rule WHERE id = ?1",
                params![rule_id],
                |row| {
                    Ok(PreferenceRule {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        kind: row.get(2)?,
                        state: row.get(3)?,
                        revision: row.get(4)?,
                        roots: Vec::new(),
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .optional()?
            .ok_or(PreferenceError::RuleNotFound { rule_id })?;
        let mut statement = self.connection().prepare(
            "SELECT root_path FROM preference_rule_root
             WHERE rule_id = ?1 ORDER BY ordinal",
        )?;
        rule.roots = statement
            .query_map(params![rule_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rule)
    }

    pub fn save_preference_rule(
        &self,
        operation_id: &str,
        rule_id: Option<i64>,
        name: &str,
        roots: &[String],
        expected_revision: i64,
    ) -> Result<PreferenceRuleSaveResult, PreferenceError> {
        validate_rule_storage_inputs(operation_id, name, roots, expected_revision)?;
        let roots_json = serde_json::to_string(roots)?;
        let now = Utc::now().to_rfc3339();
        let transaction =
            Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;

        let replay = transaction
            .query_row(
                "SELECT requested_rule_id, name, roots_json, expected_revision,
                        applied_rule_id, applied_revision, created_at
                 FROM preference_rule_command WHERE operation_id = ?1",
                params![operation_id],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()?;
        if let Some((
            saved_rule_id,
            saved_name,
            saved_roots,
            saved_expected,
            applied_id,
            applied_revision,
            command_created_at,
        )) = replay
        {
            if saved_rule_id != rule_id
                || saved_name != name
                || saved_roots != roots_json
                || saved_expected != expected_revision
            {
                return Err(PreferenceError::IdempotencyConflict {
                    operation_id: operation_id.to_owned(),
                });
            }
            transaction.commit()?;
            let mut rule = self.get_preference_rule(applied_id)?;
            rule.name = saved_name;
            rule.roots = serde_json::from_str(&saved_roots)?;
            rule.revision = applied_revision;
            rule.updated_at = command_created_at;
            return Ok(PreferenceRuleSaveResult {
                rule,
                replayed: true,
            });
        }

        let applied_id;
        let applied_revision;
        if let Some(existing_id) = rule_id {
            let (state, current_revision) = transaction
                .query_row(
                    "SELECT state, revision FROM preference_rule WHERE id = ?1",
                    params![existing_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional()?
                .ok_or(PreferenceError::RuleNotFound {
                    rule_id: existing_id,
                })?;
            if state != "active" {
                return Err(PreferenceError::RuleArchived {
                    rule_id: existing_id,
                });
            }
            if current_revision != expected_revision {
                return Err(PreferenceError::StaleRuleRevision {
                    rule_id: existing_id,
                    expected: expected_revision,
                    current: current_revision,
                });
            }
            applied_id = existing_id;
            applied_revision = current_revision + 1;
            transaction
                .execute(
                    "UPDATE preference_rule SET name = ?1, revision = ?2, updated_at = ?3
                     WHERE id = ?4",
                    params![name, applied_revision, now, existing_id],
                )
                .map_err(|error| map_rule_write_error(error, name))?;
            transaction.execute(
                "DELETE FROM preference_rule_root WHERE rule_id = ?1",
                params![existing_id],
            )?;
        } else {
            if expected_revision != 0 {
                return Err(PreferenceError::StaleRuleRevision {
                    rule_id: 0,
                    expected: expected_revision,
                    current: 0,
                });
            }
            transaction
                .execute(
                    "INSERT INTO preference_rule
                        (name, kind, state, revision, created_at, updated_at)
                     VALUES (?1, 'ordered_preferred_scan_roots', 'active', 1, ?2, ?2)",
                    params![name, now],
                )
                .map_err(|error| map_rule_write_error(error, name))?;
            applied_id = transaction.last_insert_rowid();
            applied_revision = 1;
        }

        for (ordinal, root) in roots.iter().enumerate() {
            transaction.execute(
                "INSERT INTO preference_rule_root (rule_id, ordinal, root_path)
                 VALUES (?1, ?2, ?3)",
                params![applied_id, ordinal as i64, root],
            )?;
        }
        transaction.execute(
            "INSERT INTO preference_rule_command
                (operation_id, requested_rule_id, name, roots_json, expected_revision,
                 applied_rule_id, applied_revision, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                operation_id,
                rule_id,
                name,
                roots_json,
                expected_revision,
                applied_id,
                applied_revision,
                now
            ],
        )?;
        transaction.commit()?;
        Ok(PreferenceRuleSaveResult {
            rule: self.get_preference_rule(applied_id)?,
            replayed: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn page_preference_preview(
        &self,
        run_id: i64,
        rule_id: i64,
        expected_rule_revision: i64,
        expected_review_revision: i64,
        scope: &PreferencePreviewScope,
        limit: i64,
        after_group_id: Option<i64>,
    ) -> Result<PreferencePreviewPage, PreferenceError> {
        self.ensure_preference_preview_run(run_id)?;
        let rule = self.get_preference_rule(rule_id)?;
        if rule.state != "active" {
            return Err(PreferenceError::RuleArchived { rule_id });
        }
        if rule.revision != expected_rule_revision {
            return Err(PreferenceError::StaleRuleRevision {
                rule_id,
                expected: expected_rule_revision,
                current: rule.revision,
            });
        }
        let plan = self.active_review_plan(run_id)?;
        let review_revision = plan.as_ref().map_or(0, |value| value.revision);
        if review_revision != expected_review_revision {
            return Err(PreferenceError::StaleReviewRevision {
                expected: expected_review_revision,
                current: review_revision,
            });
        }
        let plan_id = plan.as_ref().map(|value| value.id);
        let group_ids = self.preference_scope_group_ids(run_id, scope)?;
        if group_ids.len() > MAX_PREVIEW_GROUPS {
            return Err(PreferenceError::PreviewTooComplex {
                scoped_group_count: group_ids.len(),
                maximum_group_count: MAX_PREVIEW_GROUPS,
                scoped_logical_path_count: None,
                maximum_logical_path_count: MAX_PREVIEW_LOGICAL_PATHS,
            });
        }
        let directory_state = self.preference_directory_state(run_id, plan_id)?;
        let manual_removed_files = self.preference_manual_removed_files(run_id, plan_id)?;

        let rank_by_root = rule
            .roots
            .iter()
            .enumerate()
            .map(|(index, root)| (root.to_lowercase(), (index as i64, root.clone())))
            .collect::<HashMap<_, _>>();
        let mut present_roots = HashSet::new();
        let mut scoped_physical = HashMap::<String, i64>::new();
        let mut summary = PreferencePreviewSummary {
            scoped_group_count: group_ids.len() as i64,
            ..PreferencePreviewSummary::default()
        };
        let mut all_rows = Vec::new();
        let mut proposed_physical = HashMap::<String, i64>::new();
        let (member_sql, member_values) =
            self.preference_preview_member_query(run_id, plan_id, scope, &group_ids);
        let mut member_statement = self.connection().prepare(&member_sql)?;
        let mut member_rows = member_statement.query_map(
            params_from_iter(member_values.iter()),
            preview_member_from_row,
        )?;
        let mut next_member = member_rows.next().transpose()?;

        for group_id in group_ids {
            let mut group_members = Vec::new();
            while next_member
                .as_ref()
                .is_some_and(|member| member.group_id == group_id)
            {
                let member = next_member.take().expect("member was checked above");
                present_roots.insert(member.root_path.to_lowercase());
                scoped_physical
                    .entry(member.physical_key.clone())
                    .or_insert(member.file_size);
                summary.scoped_logical_path_count += 1;
                if summary.scoped_logical_path_count as usize > MAX_PREVIEW_LOGICAL_PATHS {
                    return Err(PreferenceError::PreviewTooComplex {
                        scoped_group_count: summary.scoped_group_count as usize,
                        maximum_group_count: MAX_PREVIEW_GROUPS,
                        scoped_logical_path_count: Some(summary.scoped_logical_path_count as usize),
                        maximum_logical_path_count: MAX_PREVIEW_LOGICAL_PATHS,
                    });
                }
                group_members.push(member);
                next_member = member_rows.next().transpose()?;
            }
            if group_members.is_empty() {
                summary.no_ranked_root_group_count += 1;
                continue;
            }
            let manual_keep_count = group_members
                .iter()
                .filter(|member| {
                    member.provenance.as_deref() == Some("manual")
                        && member.effective_decision == ReviewDecisionKind::Keep
                })
                .count() as i64;
            let manual_remove_count = group_members
                .iter()
                .filter(|member| {
                    member.provenance.as_deref() == Some("manual")
                        && member.effective_decision == ReviewDecisionKind::Remove
                })
                .count() as i64;
            summary.manual_keep_path_count += manual_keep_count;
            summary.manual_remove_path_count += manual_remove_count;

            let mut eligible = Vec::new();
            for member in &group_members {
                let (_, folder_remove) = directory_decisions(member, &directory_state);
                if member.effective_decision == ReviewDecisionKind::Undecided
                    && folder_remove.is_none()
                {
                    eligible.push(member);
                }
            }
            let best = eligible
                .iter()
                .filter_map(|member| rank_by_root.get(&member.root_path.to_lowercase()))
                .min_by_key(|(rank, _)| *rank)
                .cloned();
            let Some((best_rank, preferred_root)) = best else {
                summary.no_ranked_root_group_count += 1;
                continue;
            };
            let proposed_keep = eligible
                .iter()
                .copied()
                .filter(|member| {
                    rank_by_root
                        .get(&member.root_path.to_lowercase())
                        .is_some_and(|(rank, _)| *rank == best_rank)
                })
                .collect::<Vec<_>>();
            let proposed_remove = eligible
                .iter()
                .copied()
                .filter(|member| {
                    !proposed_keep
                        .iter()
                        .any(|keep| keep.file_id == member.file_id)
                })
                .collect::<Vec<_>>();

            let mut status = PreferencePreviewStatus::Applicable;
            let mut explanation = if proposed_keep.len() > 1 {
                "highest_rank_tie"
            } else if manual_keep_count > 0 {
                "manual_keep_precedence"
            } else {
                "preferred_root_rank"
            };
            let mut conflict_file_id = None;
            let mut conflict_folder_member_id = None;

            if let Some((member, folder_member)) = proposed_remove.iter().find_map(|member| {
                let (folder_keep, _) = directory_decisions(member, &directory_state);
                folder_keep.map(|folder_member| (*member, folder_member))
            }) {
                status = PreferencePreviewStatus::Blocked;
                explanation = "manual_folder_keep_conflict";
                conflict_file_id = Some(member.file_id);
                conflict_folder_member_id = Some(folder_member);
                summary.overlap_conflict_count += 1;
            } else if !physical_survivor_remains(&group_members, &proposed_remove, &directory_state)
            {
                status = PreferencePreviewStatus::Blocked;
                explanation = "file_survivor_conflict";
                summary.file_survivor_conflict_count += 1;
            } else if let Some((file_id, folder_member_id)) =
                folder_survivor_conflict(&proposed_remove, &manual_removed_files, &directory_state)
            {
                status = PreferencePreviewStatus::Blocked;
                explanation = "folder_survivor_conflict";
                conflict_file_id = Some(file_id);
                conflict_folder_member_id = Some(folder_member_id);
                summary.folder_survivor_conflict_count += 1;
            }

            let (remove_physical_count, remove_bytes, fully_removed_keys) =
                physical_remove_totals(&group_members, &proposed_remove, &directory_state);
            if status == PreferencePreviewStatus::Applicable {
                summary.affected_group_count += 1;
                summary.proposed_keep_path_count += proposed_keep.len() as i64;
                summary.proposed_remove_path_count += proposed_remove.len() as i64;
                for (key, bytes) in fully_removed_keys {
                    proposed_physical.entry(key).or_insert(bytes);
                }
            } else {
                summary.blocked_group_count += 1;
            }
            if proposed_keep.len() > 1 {
                summary.tied_group_count += 1;
            }
            all_rows.push(PreferencePreviewGroup {
                group_id,
                status,
                best_rank: Some(best_rank),
                preferred_root: Some(preferred_root),
                tied_preferred_path_count: proposed_keep.len() as i64,
                proposed_keep_path_count: if status == PreferencePreviewStatus::Applicable {
                    proposed_keep.len() as i64
                } else {
                    0
                },
                proposed_remove_path_count: if status == PreferencePreviewStatus::Applicable {
                    proposed_remove.len() as i64
                } else {
                    0
                },
                proposed_remove_physical_item_count: if status
                    == PreferencePreviewStatus::Applicable
                {
                    remove_physical_count
                } else {
                    0
                },
                proposed_remove_bytes: if status == PreferencePreviewStatus::Applicable {
                    remove_bytes
                } else {
                    0
                },
                manual_keep_count,
                manual_remove_count,
                explanation_code: explanation.to_owned(),
                conflict_file_id,
                conflict_folder_member_id,
            });
        }
        summary.scoped_physical_item_count = scoped_physical.len() as i64;
        summary.scoped_bytes = scoped_physical.values().sum();
        summary.missing_rule_root_count = rule
            .roots
            .iter()
            .filter(|root| !present_roots.contains(&root.to_lowercase()))
            .count() as i64;
        summary.proposed_remove_physical_item_count = proposed_physical.len() as i64;
        summary.proposed_remove_bytes = proposed_physical.values().sum();
        all_rows.sort_by_key(|row| row.group_id);
        let total = all_rows.len() as i64;
        let start = after_group_id
            .map(|id| all_rows.partition_point(|row| row.group_id <= id))
            .unwrap_or(0);
        let end = (start + limit as usize + 1).min(all_rows.len());
        let mut page_rows = all_rows[start..end].to_vec();
        let has_more = page_rows.len() > limit as usize;
        if has_more {
            page_rows.pop();
        }
        Ok(PreferencePreviewPage {
            groups: page_rows,
            total,
            has_more,
            rule_id,
            rule_revision: rule.revision,
            review_plan_id: plan_id,
            review_revision,
            preview_signature: preference_preview_signature(
                run_id,
                rule_id,
                rule.revision,
                review_revision,
                scope,
            )?,
            summary,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_preference_rule(
        &self,
        operation_id: &str,
        run_id: i64,
        rule_id: i64,
        rule_revision: i64,
        source_review_revision: i64,
        preview_signature: &str,
        scope: &PreferencePreviewScope,
    ) -> Result<PreferenceRuleApplicationResult, PreferenceError> {
        validate_operation_id(operation_id)?;
        let scope_json = preference_scope_json(scope)?;
        let scope_signature = signature_for(&scope_json);
        let expected_preview_signature = preference_preview_signature(
            run_id,
            rule_id,
            rule_revision,
            source_review_revision,
            scope,
        )?;
        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        if let Some(existing) = preference_application_by_operation(&tx, operation_id)? {
            if existing.run_id != run_id
                || existing.rule_id != rule_id
                || existing.rule_revision != rule_revision
                || existing.source_review_revision != source_review_revision
                || existing.scope_json != scope_json
                || existing.scope_signature != scope_signature
                || existing.preview_signature != preview_signature
            {
                return Err(PreferenceError::IdempotencyConflict {
                    operation_id: operation_id.to_owned(),
                });
            }
            tx.commit()?;
            return Ok(PreferenceRuleApplicationResult {
                application: existing,
                replayed: true,
            });
        }
        ensure_preference_operation_unlocked(&tx, run_id)?;
        if preview_signature != expected_preview_signature {
            return Err(PreferenceError::PreviewConflict);
        }

        self.ensure_preference_preview_run(run_id)?;
        let rule = self.get_preference_rule(rule_id)?;
        if rule.state != "active" {
            return Err(PreferenceError::RuleArchived { rule_id });
        }
        if rule.revision != rule_revision {
            return Err(PreferenceError::StaleRuleRevision {
                rule_id,
                expected: rule_revision,
                current: rule.revision,
            });
        }
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
        if current_revision != source_review_revision {
            return Err(PreferenceError::StaleReviewRevision {
                expected: source_review_revision,
                current: current_revision,
            });
        }

        let preview = self.page_preference_preview(
            run_id,
            rule_id,
            rule_revision,
            source_review_revision,
            scope,
            MAX_PREVIEW_GROUPS as i64,
            None,
        )?;
        if preview.preview_signature != preview_signature {
            return Err(PreferenceError::PreviewConflict);
        }
        let proposals =
            self.preference_rule_proposals(run_id, plan_id, &rule, scope, &preview.groups)?;
        if proposals.is_empty() {
            return Err(PreferenceError::ApplicationEmpty);
        }

        let proposed_file_ids = proposals
            .iter()
            .map(|proposal| proposal.member.file_id)
            .collect::<HashSet<_>>();
        {
            let mut existing = tx.prepare(
                "SELECT file_id, application_id FROM review_rule_decision WHERE plan_id = ?1",
            )?;
            for row in existing.query_map(params![plan_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })? {
                let (file_id, application_id) = row?;
                if proposed_file_ids.contains(&file_id) {
                    return Err(PreferenceError::ApplicationOverlap {
                        file_id,
                        application_id,
                    });
                }
            }
        }

        let applied_revision = current_revision + 1;
        let rule_roots_json = serde_json::to_string(&rule.roots)?;
        tx.execute(
            "INSERT INTO review_rule_application
                (plan_id, operation_id, run_id, rule_id, rule_revision, rule_name, rule_kind,
                 rule_roots_json, scope_kind, scope_json, scope_signature, preview_signature,
                 source_review_revision, applied_revision, scoped_group_count,
                 applicable_group_count, blocked_group_count, rule_keep_path_count,
                 rule_remove_path_count, rule_remove_physical_item_count, rule_remove_bytes,
                 state, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                     ?15, ?16, ?17, ?18, ?19, ?20, ?21, 'active', ?22)",
            params![
                plan_id,
                operation_id,
                run_id,
                rule_id,
                rule_revision,
                rule.name,
                rule.kind,
                rule_roots_json,
                preference_scope_kind(scope),
                scope_json,
                scope_signature,
                preview_signature,
                source_review_revision,
                applied_revision,
                preview.summary.scoped_group_count,
                preview.summary.affected_group_count,
                preview.summary.blocked_group_count,
                preview.summary.proposed_keep_path_count,
                preview.summary.proposed_remove_path_count,
                preview.summary.proposed_remove_physical_item_count,
                preview.summary.proposed_remove_bytes,
                now,
            ],
        )?;
        let application_id = tx.last_insert_rowid();
        {
            let mut insert = tx.prepare_cached(
                "INSERT INTO review_rule_decision
                    (application_id, plan_id, group_id, file_id, decision, explanation_code,
                     preferred_rank, decided_at, snapshot_canonical_path,
                     snapshot_file_identity, snapshot_file_size, snapshot_last_modified,
                     snapshot_content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )?;
            for proposal in proposals {
                insert.execute(params![
                    application_id,
                    plan_id,
                    proposal.member.group_id,
                    proposal.member.file_id,
                    proposal.decision.as_str(),
                    proposal.explanation_code,
                    proposal.preferred_rank,
                    now,
                    proposal.member.canonical_path,
                    proposal.member.file_identity,
                    proposal.member.file_size,
                    proposal.member.last_modified,
                    proposal.member.content_hash,
                ])?;
            }
        }
        super::review::validate_review_state(&tx, plan_id, run_id)?;
        tx.execute(
            "UPDATE review_plan SET revision = ?1, updated_at = ?2
             WHERE id = ?3 AND revision = ?4",
            params![applied_revision, now, plan_id, current_revision],
        )?;
        let application = preference_application_by_id(&tx, run_id, application_id)?.ok_or(
            PreferenceError::ApplicationNotFound {
                run_id,
                application_id,
            },
        )?;
        tx.commit()?;
        Ok(PreferenceRuleApplicationResult {
            application,
            replayed: false,
        })
    }

    fn preference_rule_proposals(
        &self,
        run_id: i64,
        plan_id: i64,
        rule: &PreferenceRule,
        scope: &PreferencePreviewScope,
        preview_groups: &[PreferencePreviewGroup],
    ) -> Result<Vec<RuleDecisionProposal>, PreferenceError> {
        let applicable = preview_groups
            .iter()
            .filter(|group| group.status == PreferencePreviewStatus::Applicable)
            .map(|group| (group.group_id, group))
            .collect::<HashMap<_, _>>();
        let group_ids = self.preference_scope_group_ids(run_id, scope)?;
        let rank_by_root = rule
            .roots
            .iter()
            .enumerate()
            .map(|(index, root)| (root.to_lowercase(), index as i64))
            .collect::<HashMap<_, _>>();
        let directory_state = self.preference_directory_state(run_id, Some(plan_id))?;
        let (sql, values) =
            self.preference_preview_member_query(run_id, Some(plan_id), scope, &group_ids);
        let mut statement = self.connection().prepare(&sql)?;
        let mut rows =
            statement.query_map(params_from_iter(values.iter()), preview_member_from_row)?;
        let mut next = rows.next().transpose()?;
        let mut proposals = Vec::new();
        for group_id in group_ids {
            let mut members = Vec::new();
            while next
                .as_ref()
                .is_some_and(|member| member.group_id == group_id)
            {
                members.push(next.take().expect("member was checked"));
                next = rows.next().transpose()?;
            }
            let Some(preview_group) = applicable.get(&group_id) else {
                continue;
            };
            let eligible = members
                .into_iter()
                .filter(|member| {
                    member.effective_decision == ReviewDecisionKind::Undecided
                        && directory_decisions(member, &directory_state).1.is_none()
                })
                .collect::<Vec<_>>();
            let Some(best_rank) = eligible
                .iter()
                .filter_map(|member| rank_by_root.get(&member.root_path.to_lowercase()).copied())
                .min()
            else {
                continue;
            };
            for member in eligible {
                let decision = if rank_by_root
                    .get(&member.root_path.to_lowercase())
                    .is_some_and(|rank| *rank == best_rank)
                {
                    ReviewDecisionKind::Keep
                } else {
                    ReviewDecisionKind::Remove
                };
                proposals.push(RuleDecisionProposal {
                    member,
                    decision,
                    explanation_code: preview_group.explanation_code.clone(),
                    preferred_rank: best_rank,
                });
            }
        }
        Ok(proposals)
    }

    pub fn get_preference_application(
        &self,
        run_id: i64,
        application_id: i64,
    ) -> Result<PreferenceRuleApplication, PreferenceError> {
        self.ensure_preference_preview_run(run_id)?;
        preference_application_by_id(self.connection(), run_id, application_id)?.ok_or(
            PreferenceError::ApplicationNotFound {
                run_id,
                application_id,
            },
        )
    }

    pub fn page_preference_applications(
        &self,
        run_id: i64,
        rule_id: Option<i64>,
        state: Option<&str>,
        limit: i64,
        before_application_id: Option<i64>,
    ) -> Result<PreferenceRuleApplicationPage, PreferenceError> {
        self.ensure_preference_preview_run(run_id)?;
        let plan = self.active_review_plan(run_id)?;
        let plan_id = plan.as_ref().map(|value| value.id);
        let revision = plan.as_ref().map_or(0, |value| value.revision);
        let state = state.unwrap_or("all");
        if !matches!(state, "all" | "active" | "reversed") {
            return Err(PreferenceError::InvalidRule {
                message: "application state must be active, reversed, or all".to_owned(),
            });
        }
        let total = self.connection().query_row(
            "SELECT COUNT(*) FROM review_rule_application
             WHERE run_id = ?1 AND (?2 IS NULL OR rule_id = ?2)
               AND (?3 = 'all' OR state = ?3)",
            params![run_id, rule_id, state],
            |row| row.get(0),
        )?;
        let mut statement = self.connection().prepare(
            "SELECT id FROM review_rule_application
             WHERE run_id = ?1 AND (?2 IS NULL OR rule_id = ?2)
               AND (?3 = 'all' OR state = ?3) AND id < ?4
             ORDER BY id DESC LIMIT ?5",
        )?;
        let ids = statement
            .query_map(
                params![
                    run_id,
                    rule_id,
                    state,
                    before_application_id.unwrap_or(i64::MAX),
                    limit + 1
                ],
                |row| row.get::<_, i64>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = ids.len() > limit as usize;
        let mut applications = Vec::with_capacity(ids.len().min(limit as usize));
        for id in ids.into_iter().take(limit as usize) {
            if let Some(application) = preference_application_by_id(self.connection(), run_id, id)?
            {
                applications.push(application);
            }
        }
        Ok(PreferenceRuleApplicationPage {
            applications,
            total,
            has_more,
            plan_id,
            revision,
        })
    }

    pub fn reverse_preference_rule_application(
        &self,
        operation_id: &str,
        run_id: i64,
        application_id: i64,
        expected_revision: i64,
    ) -> Result<PreferenceRuleReversalResult, PreferenceError> {
        validate_operation_id(operation_id)?;
        let tx = Transaction::new_unchecked(self.connection(), TransactionBehavior::Immediate)?;
        if let Some(replay) = tx
            .query_row(
                "SELECT plan_id, run_id, application_id, expected_revision, applied_revision,
                        removed_keep_count, removed_remove_count
                 FROM review_rule_reversal_command WHERE operation_id = ?1",
                params![operation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()?
        {
            if replay.1 != run_id || replay.2 != application_id || replay.3 != expected_revision {
                return Err(PreferenceError::IdempotencyConflict {
                    operation_id: operation_id.to_owned(),
                });
            }
            tx.commit()?;
            return Ok(PreferenceRuleReversalResult {
                application_id,
                plan_id: replay.0,
                applied_revision: replay.4,
                replayed: true,
                removed_keep_count: replay.5,
                removed_remove_count: replay.6,
            });
        }
        ensure_preference_operation_unlocked(&tx, run_id)?;
        let application = preference_application_by_id(&tx, run_id, application_id)?.ok_or(
            PreferenceError::ApplicationNotFound {
                run_id,
                application_id,
            },
        )?;
        if application.state != "active" {
            return Err(PreferenceError::ApplicationAlreadyReversed { application_id });
        }
        let current_revision: i64 = tx.query_row(
            "SELECT revision FROM review_plan WHERE id = ?1 AND run_id = ?2 AND state = 'active'",
            params![application.plan_id, run_id],
            |row| row.get(0),
        )?;
        if current_revision != expected_revision {
            return Err(PreferenceError::StaleReviewRevision {
                expected: expected_revision,
                current: current_revision,
            });
        }
        let (removed_keep_count, removed_remove_count) = tx.query_row(
            "SELECT COALESCE(SUM(CASE WHEN decision = 'keep' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN decision = 'remove' THEN 1 ELSE 0 END), 0)
             FROM review_rule_decision WHERE application_id = ?1",
            params![application_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        tx.execute(
            "DELETE FROM review_rule_decision WHERE application_id = ?1",
            params![application_id],
        )?;
        super::review::validate_review_state(&tx, application.plan_id, run_id)?;
        let applied_revision = current_revision + 1;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE review_rule_application SET state = 'reversed', reversed_at = ?1
             WHERE id = ?2 AND state = 'active'",
            params![now, application_id],
        )?;
        tx.execute(
            "UPDATE review_plan SET revision = ?1, updated_at = ?2
             WHERE id = ?3 AND revision = ?4",
            params![applied_revision, now, application.plan_id, current_revision],
        )?;
        tx.execute(
            "INSERT INTO review_rule_reversal_command
                (plan_id, operation_id, run_id, application_id, expected_revision,
                 applied_revision, removed_keep_count, removed_remove_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                application.plan_id,
                operation_id,
                run_id,
                application_id,
                expected_revision,
                applied_revision,
                removed_keep_count,
                removed_remove_count,
                now
            ],
        )?;
        tx.commit()?;
        Ok(PreferenceRuleReversalResult {
            application_id,
            plan_id: application.plan_id,
            applied_revision,
            replayed: false,
            removed_keep_count,
            removed_remove_count,
        })
    }

    fn ensure_preference_preview_run(&self, run_id: i64) -> Result<(), PreferenceError> {
        let status = self
            .connection()
            .query_row(
                "SELECT status FROM scan_run WHERE id = ?1",
                params![run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match status {
            None => Err(PreferenceError::RunNotFound { run_id }),
            Some(status) if status != "completed" => {
                Err(PreferenceError::RunNotCompleted { run_id, status })
            }
            Some(_) => Ok(()),
        }
    }

    fn preference_scope_group_ids(
        &self,
        run_id: i64,
        scope: &PreferencePreviewScope,
    ) -> Result<Vec<i64>, PreferenceError> {
        match scope {
            PreferencePreviewScope::CompletedRun => {
                let mut statement = self
                    .connection()
                    .prepare("SELECT id FROM duplicate_group WHERE run_id = ?1 ORDER BY id")?;
                let rows = statement
                    .query_map(params![run_id], |row| row.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            }
            PreferencePreviewScope::CurrentFilter(filter) => {
                let (predicates, values) =
                    duplicate_file_group_predicate(run_id, filter, true, true);
                let sql = format!(
                    "SELECT dg.id FROM duplicate_group dg WHERE {} ORDER BY dg.id",
                    predicates.join(" AND ")
                );
                let mut statement = self.connection().prepare(&sql)?;
                let rows = statement
                    .query_map(params_from_iter(values.iter()), |row| row.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            }
            PreferencePreviewScope::SelectedSets(ids) => {
                let mut ids = ids.clone();
                ids.sort_unstable();
                ids.dedup();
                for group_id in &ids {
                    let owned = self.connection().query_row(
                        "SELECT EXISTS(SELECT 1 FROM duplicate_group WHERE id = ?1 AND run_id = ?2)",
                        params![group_id, run_id],
                        |row| row.get::<_, bool>(0),
                    )?;
                    if !owned {
                        return Err(PreferenceError::InvalidSelectedGroup {
                            run_id,
                            group_id: *group_id,
                        });
                    }
                }
                Ok(ids)
            }
        }
    }

    fn preference_preview_member_query(
        &self,
        run_id: i64,
        plan_id: Option<i64>,
        scope: &PreferencePreviewScope,
        group_ids: &[i64],
    ) -> (String, Vec<SqlValue>) {
        let (where_sql, mut values) = match scope {
            PreferencePreviewScope::CompletedRun => {
                ("dg.run_id = ?".to_owned(), vec![SqlValue::Integer(run_id)])
            }
            PreferencePreviewScope::CurrentFilter(filter) => {
                let (predicates, values) =
                    duplicate_file_group_predicate(run_id, filter, true, true);
                (predicates.join(" AND "), values)
            }
            PreferencePreviewScope::SelectedSets(_) => {
                let placeholders = (0..group_ids.len())
                    .map(|_| "?")
                    .collect::<Vec<_>>()
                    .join(",");
                let mut values = vec![SqlValue::Integer(run_id)];
                values.extend(group_ids.iter().copied().map(SqlValue::Integer));
                (
                    format!("dg.run_id = ? AND dg.id IN ({placeholders})"),
                    values,
                )
            }
        };
        values.insert(0, plan_id.map_or(SqlValue::Null, SqlValue::Integer));
        let sql = format!(
            "SELECT sf.id, member.group_id, sf.root_path,
                    CASE WHEN sf.file_identity IS NOT NULL AND sf.file_identity <> ''
                         THEN 'i:' || sf.file_identity ELSE 'p:' || sf.canonical_path END,
                    sf.file_size, directory.id, COALESCE(decision.decision, 'undecided'),
                    decision.provenance, sf.canonical_path,
                    sf.file_identity, sf.last_modified, sf.content_hash
             FROM duplicate_group dg
             JOIN duplicate_group_member member ON member.group_id = dg.id
             JOIN scanned_file sf ON sf.id = member.file_id AND sf.run_id = dg.run_id
             LEFT JOIN directory_node directory
               ON directory.run_id = sf.run_id AND directory.path = sf.parent_dir
             LEFT JOIN effective_review_decision decision
               ON decision.plan_id = ? AND decision.file_id = sf.id
             WHERE {where_sql}
             ORDER BY member.group_id, sf.id"
        );
        (sql, values)
    }

    fn preference_directory_state(
        &self,
        run_id: i64,
        plan_id: Option<i64>,
    ) -> Result<DirectoryReviewState, PreferenceError> {
        let mut state = DirectoryReviewState::default();
        let mut directory_statement = self
            .connection()
            .prepare("SELECT id, parent_id FROM directory_node WHERE run_id = ?1")?;
        for row in directory_statement.query_map(params![run_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?))
        })? {
            let (id, parent) = row?;
            state.parents.insert(id, parent);
        }
        if let Some(plan_id) = plan_id {
            let mut decision_statement = self.connection().prepare(
                "SELECT directory_id, folder_member_id, decision
                 FROM review_folder_decision WHERE plan_id = ?1 AND decision <> 'undecided'",
            )?;
            for row in decision_statement.query_map(params![plan_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })? {
                let (directory_id, member_id, decision) = row?;
                if decision == "keep" {
                    state.keep_roots.insert(directory_id, member_id);
                } else {
                    state.remove_roots.insert(directory_id, member_id);
                }
            }
        }
        let mut folder_statement = self.connection().prepare(
            "SELECT group_member.group_id, group_member.id, group_member.directory_id
             FROM duplicate_folder_group_member group_member
             JOIN duplicate_folder_group folder_group ON folder_group.id = group_member.group_id
             WHERE folder_group.run_id = ?1",
        )?;
        for row in folder_statement.query_map(params![run_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })? {
            let (group_id, member_id, directory_id) = row?;
            state
                .folder_groups
                .entry(group_id)
                .or_default()
                .push((member_id, directory_id));
        }
        Ok(state)
    }

    fn preference_manual_removed_files(
        &self,
        run_id: i64,
        plan_id: Option<i64>,
    ) -> Result<Vec<(i64, Option<i64>)>, PreferenceError> {
        let Some(plan_id) = plan_id else {
            return Ok(Vec::new());
        };
        let mut statement = self.connection().prepare(
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
                 SELECT file.id
                 FROM removed_directories removed
                 JOIN directory_node directory ON directory.id = removed.directory_id
                 JOIN scanned_file file
                   ON file.run_id = ?1
                  AND file.parent_dir = directory.path COLLATE UNICODE_NOCASE
             )
             SELECT removed.file_id, directory.id
             FROM removed_files removed
             JOIN scanned_file file ON file.id = removed.file_id AND file.run_id = ?1
             LEFT JOIN directory_node directory
               ON directory.run_id = file.run_id
              AND directory.path = file.parent_dir COLLATE UNICODE_NOCASE",
        )?;
        let rows = statement
            .query_map(params![run_id, plan_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

fn preview_member_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PreviewMember> {
    let decision: String = row.get(6)?;
    Ok(PreviewMember {
        file_id: row.get(0)?,
        group_id: row.get(1)?,
        root_path: row.get(2)?,
        physical_key: row.get(3)?,
        file_size: row.get(4)?,
        parent_directory_id: row.get(5)?,
        effective_decision: ReviewDecisionKind::parse(&decision).unwrap_or_default(),
        provenance: row.get(7)?,
        canonical_path: row.get(8)?,
        file_identity: row.get(9)?,
        last_modified: row.get(10)?,
        content_hash: row.get(11)?,
    })
}

fn preference_scope_json(scope: &PreferencePreviewScope) -> Result<String, serde_json::Error> {
    let value = match scope {
        PreferencePreviewScope::CompletedRun => serde_json::json!({"kind":"completed_run"}),
        PreferencePreviewScope::SelectedSets(ids) => {
            let mut ids = ids.clone();
            ids.sort_unstable();
            ids.dedup();
            serde_json::json!({"kind":"selected_sets","groupIds":ids})
        }
        PreferencePreviewScope::CurrentFilter(filter) => serde_json::json!({
            "kind":"current_filter",
            "filter":{
                "search":filter.search,
                "pathMatch":match filter.path_match {
                    DuplicateFilePathMatchMode::Substring => "substring",
                    DuplicateFilePathMatchMode::Exact => "exact",
                },
                "extension":filter.extension_key,
                "extensionMatch":match filter.extension_match {
                    DuplicateFileExtensionMatchMode::AnyMember => "any",
                    DuplicateFileExtensionMatchMode::AllMembers => "all",
                },
                "minimumSize":filter.minimum_size.to_string(),
                "minimumCopyCount":filter.minimum_copy_count,
                "acrossDrives":filter.across_drives,
                "selectedRoot":filter.selected_root,
                "selectedDrive":filter.selected_drive,
            }
        }),
    };
    serde_json::to_string(&value)
}

fn preference_scope_kind(scope: &PreferencePreviewScope) -> &'static str {
    match scope {
        PreferencePreviewScope::CompletedRun => "completed_run",
        PreferencePreviewScope::SelectedSets(_) => "selected_sets",
        PreferencePreviewScope::CurrentFilter(_) => "current_filter",
    }
}

fn signature_for(value: &str) -> String {
    let mut hasher = XxHash64::with_seed(0x5355_5045_525f_4455);
    hasher.write(value.as_bytes());
    format!("v1-{:016x}", hasher.finish())
}

fn preference_preview_signature(
    run_id: i64,
    rule_id: i64,
    rule_revision: i64,
    review_revision: i64,
    scope: &PreferencePreviewScope,
) -> Result<String, serde_json::Error> {
    let scope_json = preference_scope_json(scope)?;
    Ok(signature_for(&format!(
        "preview\n{run_id}\n{rule_id}\n{rule_revision}\n{review_revision}\n{scope_json}"
    )))
}

fn ensure_preference_operation_unlocked(
    tx: &Transaction<'_>,
    run_id: i64,
) -> Result<(), PreferenceError> {
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
                        error_detail = 'Rule provenance mutation invalidated the unsubmitted operation intent'
                 WHERE id = ?2",
                params![Utc::now().to_rfc3339(), operation_id],
            )?;
            return Ok(());
        }
        return Err(PreferenceError::OperationLocked {
            run_id,
            operation_id,
        });
    }
    Ok(())
}

fn validate_operation_id(operation_id: &str) -> Result<(), PreferenceError> {
    if operation_id.is_empty() || operation_id.chars().count() > 128 {
        return Err(PreferenceError::InvalidRule {
            message: "operation id must contain 1 to 128 characters".to_owned(),
        });
    }
    Ok(())
}

fn preference_application_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<PreferenceRuleApplication> {
    let roots_json = row.get::<_, String>(7)?;
    let rule_roots = serde_json::from_str(&roots_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(PreferenceRuleApplication {
        id: row.get(0)?,
        plan_id: row.get(1)?,
        run_id: row.get(2)?,
        rule_id: row.get(3)?,
        rule_revision: row.get(4)?,
        rule_name: row.get(5)?,
        rule_kind: row.get(6)?,
        rule_roots,
        scope_kind: row.get(8)?,
        scope_json: row.get(9)?,
        scope_signature: row.get(10)?,
        preview_signature: row.get(11)?,
        source_review_revision: row.get(12)?,
        applied_revision: row.get(13)?,
        state: row.get(14)?,
        created_at: row.get(15)?,
        reversed_at: row.get(16)?,
        summary: PreferenceApplicationSummary {
            scoped_group_count: row.get(17)?,
            applicable_group_count: row.get(18)?,
            blocked_group_count: row.get(19)?,
            rule_keep_path_count: row.get(20)?,
            rule_remove_path_count: row.get(21)?,
            rule_remove_physical_item_count: row.get(22)?,
            rule_remove_bytes: row.get(23)?,
        },
    })
}

const APPLICATION_SELECT: &str =
    "SELECT id, plan_id, run_id, rule_id, rule_revision, rule_name, rule_kind,
            rule_roots_json, scope_kind, scope_json, scope_signature, preview_signature,
            source_review_revision, applied_revision, state, created_at, reversed_at,
            scoped_group_count, applicable_group_count, blocked_group_count,
            rule_keep_path_count, rule_remove_path_count,
            rule_remove_physical_item_count, rule_remove_bytes
     FROM review_rule_application";

fn preference_application_by_operation(
    connection: &rusqlite::Connection,
    operation_id: &str,
) -> Result<Option<PreferenceRuleApplication>, PreferenceError> {
    Ok(connection
        .query_row(
            &format!("{APPLICATION_SELECT} WHERE operation_id = ?1"),
            params![operation_id],
            preference_application_from_row,
        )
        .optional()?)
}

fn preference_application_by_id(
    connection: &rusqlite::Connection,
    run_id: i64,
    application_id: i64,
) -> Result<Option<PreferenceRuleApplication>, PreferenceError> {
    Ok(connection
        .query_row(
            &format!("{APPLICATION_SELECT} WHERE run_id = ?1 AND id = ?2"),
            params![run_id, application_id],
            preference_application_from_row,
        )
        .optional()?)
}

fn validate_rule_storage_inputs(
    operation_id: &str,
    name: &str,
    roots: &[String],
    expected_revision: i64,
) -> Result<(), PreferenceError> {
    if operation_id.is_empty() || operation_id.chars().count() > 128 {
        return Err(PreferenceError::InvalidRule {
            message: "operation id must contain 1 to 128 characters".to_owned(),
        });
    }
    if name.trim().is_empty() || name.trim().chars().count() > 128 || name != name.trim() {
        return Err(PreferenceError::InvalidRule {
            message: "name must contain 1 to 128 characters without surrounding whitespace"
                .to_owned(),
        });
    }
    if !(1..=64).contains(&roots.len()) || expected_revision < 0 {
        return Err(PreferenceError::InvalidRule {
            message: "rules require 1 to 64 roots and a non-negative expected revision".to_owned(),
        });
    }
    let mut distinct = HashSet::new();
    for root in roots {
        if root.is_empty() || root.chars().count() > 32_767 || root != root.trim() {
            return Err(PreferenceError::InvalidRule {
                message: "root values must be nonblank and contain at most 32767 characters"
                    .to_owned(),
            });
        }
        if !distinct.insert(root.to_lowercase()) {
            return Err(PreferenceError::InvalidRule {
                message: "root values must be unique ignoring case".to_owned(),
            });
        }
    }
    Ok(())
}

fn map_rule_write_error(error: rusqlite::Error, name: &str) -> PreferenceError {
    if matches!(error, rusqlite::Error::SqliteFailure(_, _)) {
        PreferenceError::DuplicateName {
            name: name.to_owned(),
        }
    } else {
        PreferenceError::Database(error)
    }
}

fn ancestor_chain(start: Option<i64>, state: &DirectoryReviewState) -> Vec<i64> {
    let mut result = Vec::new();
    let mut current = start;
    let mut remaining = state.parents.len().saturating_add(1);
    while let Some(id) = current {
        if remaining == 0 {
            break;
        }
        result.push(id);
        current = state.parents.get(&id).copied().flatten();
        remaining -= 1;
    }
    result
}

fn directory_decisions(
    member: &PreviewMember,
    state: &DirectoryReviewState,
) -> (Option<i64>, Option<i64>) {
    let ancestors = ancestor_chain(member.parent_directory_id, state);
    let keep = ancestors
        .iter()
        .find_map(|id| state.keep_roots.get(id).copied());
    let remove = ancestors
        .iter()
        .find_map(|id| state.remove_roots.get(id).copied());
    (keep, remove)
}

fn member_removed(
    member: &PreviewMember,
    proposed_remove: &[&PreviewMember],
    state: &DirectoryReviewState,
) -> bool {
    member.effective_decision == ReviewDecisionKind::Remove
        || directory_decisions(member, state).1.is_some()
        || proposed_remove
            .iter()
            .any(|candidate| candidate.file_id == member.file_id)
}

fn physical_survivor_remains(
    members: &[PreviewMember],
    proposed_remove: &[&PreviewMember],
    state: &DirectoryReviewState,
) -> bool {
    members
        .iter()
        .any(|member| !member_removed(member, proposed_remove, state))
}

fn physical_remove_totals(
    members: &[PreviewMember],
    proposed_remove: &[&PreviewMember],
    state: &DirectoryReviewState,
) -> (i64, i64, HashMap<String, i64>) {
    let proposed_ids = proposed_remove
        .iter()
        .map(|member| member.file_id)
        .collect::<HashSet<_>>();
    let mut keys = HashMap::new();
    for member in proposed_remove {
        let has_surviving_alias = members.iter().any(|candidate| {
            candidate.physical_key == member.physical_key
                && !member_removed(candidate, proposed_remove, state)
        });
        if !has_surviving_alias && proposed_ids.contains(&member.file_id) {
            keys.entry(member.physical_key.clone())
                .or_insert(member.file_size);
        }
    }
    (keys.len() as i64, keys.values().sum(), keys)
}

fn is_descendant_or_same(
    directory_id: Option<i64>,
    root_id: i64,
    state: &DirectoryReviewState,
) -> bool {
    ancestor_chain(directory_id, state).contains(&root_id)
}

fn folder_survivor_conflict(
    proposed_remove: &[&PreviewMember],
    manual_removed_files: &[(i64, Option<i64>)],
    state: &DirectoryReviewState,
) -> Option<(i64, i64)> {
    for member in proposed_remove {
        for copies in state.folder_groups.values() {
            let Some((touched_member_id, _)) = copies.iter().find(|(_, directory_id)| {
                is_descendant_or_same(member.parent_directory_id, *directory_id, state)
            }) else {
                continue;
            };
            let any_intact = copies.iter().any(|(_, root_directory_id)| {
                if state.remove_roots.keys().any(|removed_root| {
                    is_descendant_or_same(Some(*root_directory_id), *removed_root, state)
                }) {
                    return false;
                }
                let existing_removed_in_copy =
                    manual_removed_files.iter().any(|(_, parent_directory_id)| {
                        is_descendant_or_same(*parent_directory_id, *root_directory_id, state)
                    });
                let proposed_removed_in_copy = proposed_remove.iter().any(|candidate| {
                    is_descendant_or_same(candidate.parent_directory_id, *root_directory_id, state)
                });
                !existing_removed_in_copy && !proposed_removed_in_copy
            });
            if !any_intact {
                return Some((member.file_id, *touched_member_id));
            }
        }
    }
    None
}
