PRAGMA user_version = 14;

-- Reusable, user-owned scan definitions.
CREATE TABLE IF NOT EXISTS scan_session (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    roots_json TEXT NOT NULL,
    ignore_patterns_json TEXT NOT NULL DEFAULT '[]',
    cloud_policy TEXT NOT NULL DEFAULT 'exclude_registered_roots'
        CHECK(cloud_policy IN ('exclude_registered_roots', 'include_sync_roots_skip_placeholders', 'allow_cloud_access')),
    manual_location_exclusions_json TEXT NOT NULL DEFAULT '[]',
    registered_cloud_locations_json TEXT NOT NULL DEFAULT '[]',
    cloud_detection_status TEXT NOT NULL DEFAULT 'unavailable'
        CHECK(cloud_detection_status IN ('complete', 'unsupported', 'unavailable')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Immutable executions. parameters_json is the authoritative settings snapshot.
CREATE TABLE IF NOT EXISTS scan_run (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES scan_session(id) ON DELETE CASCADE,
    parameters_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending', 'running', 'cancelling', 'completed', 'cancelled', 'failed', 'interrupted')),
    phase TEXT CHECK(phase IS NULL OR phase IN
        ('discovering', 'hashing', 'persisting', 'analyzing_folders', 'finalizing')),
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    files_discovered INTEGER NOT NULL DEFAULT 0 CHECK(files_discovered >= 0),
    bytes_discovered INTEGER NOT NULL DEFAULT 0 CHECK(bytes_discovered >= 0),
    files_hashed INTEGER NOT NULL DEFAULT 0 CHECK(files_hashed >= 0),
    duplicate_file_groups INTEGER NOT NULL DEFAULT 0 CHECK(duplicate_file_groups >= 0),
    duplicate_folder_groups INTEGER NOT NULL DEFAULT 0 CHECK(duplicate_folder_groups >= 0),
    wasted_bytes INTEGER NOT NULL DEFAULT 0 CHECK(wasted_bytes >= 0),
    warning_count INTEGER NOT NULL DEFAULT 0 CHECK(warning_count >= 0),
    excluded_subtree_count INTEGER NOT NULL DEFAULT 0 CHECK(excluded_subtree_count >= 0),
    error_message TEXT,
    engine_version TEXT NOT NULL
);

-- Immutable file snapshots owned by exactly one run.
CREATE TABLE IF NOT EXISTS scanned_file (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    root_path TEXT NOT NULL DEFAULT '',
    canonical_path TEXT NOT NULL,
    relative_path TEXT NOT NULL DEFAULT '',
    file_name TEXT NOT NULL,
    extension_key TEXT,
    parent_dir TEXT NOT NULL,
    drive_letter TEXT NOT NULL DEFAULT '',
    file_size INTEGER NOT NULL,
    last_modified INTEGER NOT NULL,
    partial_hash INTEGER,
    content_hash INTEGER,
    file_identity TEXT,
    warning_message TEXT,
    marked_deleted INTEGER NOT NULL DEFAULT 0,
    UNIQUE(run_id, canonical_path)
);

CREATE TABLE IF NOT EXISTS duplicate_group (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    content_hash INTEGER NOT NULL,
    file_size INTEGER NOT NULL,
    file_count INTEGER NOT NULL,
    wasted_bytes INTEGER NOT NULL,
    UNIQUE(run_id, content_hash, file_size)
);

CREATE TABLE IF NOT EXISTS duplicate_group_member (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id INTEGER NOT NULL REFERENCES duplicate_group(id) ON DELETE CASCADE,
    file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
    UNIQUE(group_id, file_id)
);

-- Verified exact duplicate folders. Suppressed rows are retained so a future UI can expose
-- nested matches without repeating them in the default result set.
CREATE TABLE IF NOT EXISTS duplicate_folder_group (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    structural_fingerprint TEXT NOT NULL,
    verified_fingerprint TEXT NOT NULL,
    total_size INTEGER NOT NULL CHECK(total_size >= 0),
    file_count INTEGER NOT NULL CHECK(file_count > 0),
    folder_count INTEGER NOT NULL CHECK(folder_count > 1),
    is_suppressed INTEGER NOT NULL DEFAULT 0 CHECK(is_suppressed IN (0, 1))
);

-- Run-scoped directory index shared by exact-folder results and deferred similarity UI.
CREATE TABLE IF NOT EXISTS directory_node (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    name TEXT NOT NULL,
    parent_id INTEGER REFERENCES directory_node(id) ON DELETE CASCADE,
    total_size INTEGER NOT NULL DEFAULT 0,
    file_count INTEGER NOT NULL DEFAULT 0,
    depth INTEGER NOT NULL DEFAULT 0,
    UNIQUE(run_id, path)
);

CREATE TABLE IF NOT EXISTS directory_fingerprint (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    directory_id INTEGER NOT NULL UNIQUE REFERENCES directory_node(id) ON DELETE CASCADE,
    content_fingerprint TEXT NOT NULL,
    file_hash_set TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS directory_similarity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    dir_a_id INTEGER NOT NULL REFERENCES directory_node(id) ON DELETE CASCADE,
    dir_b_id INTEGER NOT NULL REFERENCES directory_node(id) ON DELETE CASCADE,
    similarity_score REAL NOT NULL,
    shared_bytes INTEGER NOT NULL,
    match_type TEXT NOT NULL,
    UNIQUE(run_id, dir_a_id, dir_b_id),
    CHECK(dir_a_id < dir_b_id)
);

CREATE TABLE IF NOT EXISTS duplicate_folder_group_member (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id INTEGER NOT NULL REFERENCES duplicate_folder_group(id) ON DELETE CASCADE,
    directory_id INTEGER NOT NULL REFERENCES directory_node(id) ON DELETE CASCADE,
    UNIQUE(group_id, directory_id)
);

CREATE TABLE IF NOT EXISTS deletion_plan (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL UNIQUE REFERENCES scanned_file(id) ON DELETE CASCADE,
    marked_at TEXT NOT NULL,
    strategy TEXT,
    executed_at TEXT,
    execution_result TEXT
);

-- Durable review state remains separate from legacy deletion staging and immutable scan rows.
CREATE TABLE IF NOT EXISTS review_plan (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    state TEXT NOT NULL DEFAULT 'active' CHECK(state IN ('active', 'archived')),
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS review_decision (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
    group_id INTEGER NOT NULL REFERENCES duplicate_group(id) ON DELETE CASCADE,
    file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
    decision TEXT NOT NULL CHECK(decision IN ('keep', 'remove', 'undecided')),
    provenance TEXT NOT NULL CHECK(provenance = 'manual'),
    decided_at TEXT NOT NULL,
    snapshot_canonical_path TEXT NOT NULL,
    snapshot_file_identity TEXT,
    snapshot_file_size INTEGER NOT NULL CHECK(snapshot_file_size >= 0),
    snapshot_last_modified INTEGER NOT NULL,
    snapshot_content_hash INTEGER,
    manual_revision INTEGER NOT NULL DEFAULT 0 CHECK(manual_revision >= 0),
    UNIQUE(plan_id, file_id)
);

CREATE TABLE IF NOT EXISTS review_command (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
    operation_id TEXT NOT NULL,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    group_id INTEGER NOT NULL REFERENCES duplicate_group(id) ON DELETE CASCADE,
    file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
    decision TEXT NOT NULL CHECK(decision IN ('keep', 'remove', 'undecided')),
    expected_revision INTEGER NOT NULL CHECK(expected_revision >= 0),
    applied_revision INTEGER NOT NULL CHECK(applied_revision > 0),
    created_at TEXT NOT NULL,
    UNIQUE(plan_id, operation_id)
);

-- Exact-folder review remains a separate target kind with its own immutable snapshot and
-- idempotency payload. It shares only the owning plan and monotonic plan revision.
CREATE TABLE IF NOT EXISTS review_folder_decision (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
    folder_group_id INTEGER NOT NULL REFERENCES duplicate_folder_group(id) ON DELETE CASCADE,
    folder_member_id INTEGER NOT NULL REFERENCES duplicate_folder_group_member(id) ON DELETE CASCADE,
    directory_id INTEGER NOT NULL REFERENCES directory_node(id) ON DELETE CASCADE,
    decision TEXT NOT NULL CHECK(decision IN ('keep', 'remove', 'undecided')),
    provenance TEXT NOT NULL CHECK(provenance = 'manual'),
    decided_at TEXT NOT NULL,
    snapshot_path TEXT NOT NULL,
    snapshot_total_size INTEGER NOT NULL CHECK(snapshot_total_size >= 0),
    snapshot_file_count INTEGER NOT NULL CHECK(snapshot_file_count > 0),
    snapshot_structural_fingerprint TEXT NOT NULL,
    snapshot_verified_fingerprint TEXT NOT NULL,
    UNIQUE(plan_id, folder_member_id)
);

CREATE TABLE IF NOT EXISTS review_folder_command (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
    operation_id TEXT NOT NULL,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    folder_group_id INTEGER NOT NULL REFERENCES duplicate_folder_group(id) ON DELETE CASCADE,
    folder_member_id INTEGER NOT NULL REFERENCES duplicate_folder_group_member(id) ON DELETE CASCADE,
    decision TEXT NOT NULL CHECK(decision IN ('keep', 'remove', 'undecided')),
    expected_revision INTEGER NOT NULL CHECK(expected_revision >= 0),
    applied_revision INTEGER NOT NULL CHECK(applied_revision > 0),
    created_at TEXT NOT NULL,
    UNIQUE(plan_id, operation_id)
);

-- Reusable preference configuration is deliberately independent of review decisions and any
-- future execution state. Preview reads these rows but never writes a review plan.
CREATE TABLE IF NOT EXISTS preference_rule (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL COLLATE UNICODE_NOCASE UNIQUE,
    kind TEXT NOT NULL CHECK(kind = 'ordered_preferred_scan_roots'),
    state TEXT NOT NULL DEFAULT 'active' CHECK(state IN ('active', 'archived')),
    revision INTEGER NOT NULL CHECK(revision > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS preference_rule_root (
    rule_id INTEGER NOT NULL REFERENCES preference_rule(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0 AND ordinal < 64),
    root_path TEXT NOT NULL CHECK(root_path <> ''),
    PRIMARY KEY(rule_id, ordinal),
    UNIQUE(rule_id, root_path COLLATE UNICODE_NOCASE)
);

CREATE TABLE IF NOT EXISTS preference_rule_command (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id TEXT NOT NULL UNIQUE,
    requested_rule_id INTEGER,
    name TEXT NOT NULL,
    roots_json TEXT NOT NULL,
    expected_revision INTEGER NOT NULL CHECK(expected_revision >= 0),
    applied_rule_id INTEGER NOT NULL REFERENCES preference_rule(id) ON DELETE CASCADE,
    applied_revision INTEGER NOT NULL CHECK(applied_revision > 0),
    created_at TEXT NOT NULL
);

-- Applying a reusable rule creates review-only provenance and distinct rule-owned decisions.
-- Reversal removes only the child decision rows and retains the application history row.
CREATE TABLE IF NOT EXISTS review_rule_application (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
    operation_id TEXT NOT NULL UNIQUE,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    rule_id INTEGER NOT NULL REFERENCES preference_rule(id) ON DELETE RESTRICT,
    rule_revision INTEGER NOT NULL CHECK(rule_revision > 0),
    rule_name TEXT NOT NULL,
    rule_kind TEXT NOT NULL CHECK(rule_kind = 'ordered_preferred_scan_roots'),
    rule_roots_json TEXT NOT NULL,
    scope_kind TEXT NOT NULL CHECK(scope_kind IN ('selected_sets', 'current_filter', 'completed_run')),
    scope_json TEXT NOT NULL,
    scope_signature TEXT NOT NULL,
    preview_signature TEXT NOT NULL,
    source_review_revision INTEGER NOT NULL CHECK(source_review_revision >= 0),
    applied_revision INTEGER NOT NULL CHECK(applied_revision > 0),
    scoped_group_count INTEGER NOT NULL CHECK(scoped_group_count >= 0),
    applicable_group_count INTEGER NOT NULL CHECK(applicable_group_count >= 0),
    blocked_group_count INTEGER NOT NULL CHECK(blocked_group_count >= 0),
    rule_keep_path_count INTEGER NOT NULL CHECK(rule_keep_path_count >= 0),
    rule_remove_path_count INTEGER NOT NULL CHECK(rule_remove_path_count >= 0),
    rule_remove_physical_item_count INTEGER NOT NULL CHECK(rule_remove_physical_item_count >= 0),
    rule_remove_bytes INTEGER NOT NULL CHECK(rule_remove_bytes >= 0),
    state TEXT NOT NULL DEFAULT 'active' CHECK(state IN ('active', 'reversed')),
    created_at TEXT NOT NULL,
    reversed_at TEXT
);

CREATE TABLE IF NOT EXISTS review_rule_decision (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    application_id INTEGER NOT NULL REFERENCES review_rule_application(id) ON DELETE CASCADE,
    plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
    group_id INTEGER NOT NULL REFERENCES duplicate_group(id) ON DELETE CASCADE,
    file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
    decision TEXT NOT NULL CHECK(decision IN ('keep', 'remove')),
    explanation_code TEXT NOT NULL,
    preferred_rank INTEGER CHECK(preferred_rank IS NULL OR preferred_rank >= 0),
    decided_at TEXT NOT NULL,
    snapshot_canonical_path TEXT NOT NULL,
    snapshot_file_identity TEXT,
    snapshot_file_size INTEGER NOT NULL CHECK(snapshot_file_size >= 0),
    snapshot_last_modified INTEGER NOT NULL,
    snapshot_content_hash INTEGER,
    UNIQUE(plan_id, file_id)
);

CREATE TABLE IF NOT EXISTS review_rule_reversal_command (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
    operation_id TEXT NOT NULL UNIQUE,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    application_id INTEGER NOT NULL REFERENCES review_rule_application(id) ON DELETE CASCADE,
    expected_revision INTEGER NOT NULL CHECK(expected_revision >= 0),
    applied_revision INTEGER NOT NULL CHECK(applied_revision > 0),
    removed_keep_count INTEGER NOT NULL CHECK(removed_keep_count >= 0),
    removed_remove_count INTEGER NOT NULL CHECK(removed_remove_count >= 0),
    created_at TEXT NOT NULL
);

-- The latest bounded live observation is a mutable working overlay. It never rewrites the
-- immutable scan snapshot or the recorded manual/rule decision rows. decision_invalidated is
-- sticky until the operator records a fresh decision after a matching validation.
CREATE TABLE IF NOT EXISTS review_live_validation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id TEXT NOT NULL UNIQUE,
    request_signature TEXT NOT NULL,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    group_id INTEGER NOT NULL REFERENCES duplicate_group(id) ON DELETE CASCADE,
    expected_review_revision INTEGER NOT NULL CHECK(expected_review_revision >= 0),
    scope TEXT NOT NULL CHECK(scope IN ('selection', 'visible_page')),
    item_count INTEGER NOT NULL CHECK(item_count BETWEEN 1 AND 200),
    present_count INTEGER NOT NULL CHECK(present_count >= 0),
    changed_count INTEGER NOT NULL CHECK(changed_count >= 0),
    missing_count INTEGER NOT NULL CHECK(missing_count >= 0),
    unavailable_count INTEGER NOT NULL CHECK(unavailable_count >= 0),
    invalidated_decision_count INTEGER NOT NULL CHECK(invalidated_decision_count >= 0),
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS review_live_validation_item (
    validation_id INTEGER NOT NULL REFERENCES review_live_validation(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0 AND ordinal < 200),
    file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK(state IN ('present', 'changed', 'missing', 'unavailable')),
    reason_code TEXT NOT NULL,
    observed_file_identity TEXT,
    observed_file_size INTEGER CHECK(observed_file_size IS NULL OR observed_file_size >= 0),
    observed_last_modified INTEGER,
    os_error INTEGER,
    decision_invalidated INTEGER NOT NULL CHECK(decision_invalidated IN (0, 1)),
    invalidated_decision TEXT CHECK(invalidated_decision IS NULL OR invalidated_decision IN ('keep', 'remove')),
    observed_at TEXT NOT NULL,
    PRIMARY KEY(validation_id, ordinal),
    UNIQUE(validation_id, file_id)
);

-- Watcher overflow is a durable root-level loss-of-trust marker. The latest row is bounded by the
-- immutable run root count (maximum 64), while request ledgers provide idempotent retry evidence.
CREATE TABLE IF NOT EXISTS review_live_root_state (
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    root_path TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('dirty', 'clean')),
    dirty_revision INTEGER NOT NULL CHECK(dirty_revision > 0),
    reason_code TEXT NOT NULL CHECK(reason_code = 'watcher_overflow'),
    dirty_at TEXT NOT NULL,
    reconciliation_cursor_file_id INTEGER REFERENCES scanned_file(id) ON DELETE SET NULL,
    reconciled_item_count INTEGER NOT NULL DEFAULT 0 CHECK(reconciled_item_count >= 0),
    updated_at TEXT NOT NULL,
    PRIMARY KEY(run_id, root_path)
);

CREATE TABLE IF NOT EXISTS review_live_root_overflow (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id TEXT NOT NULL UNIQUE,
    request_signature TEXT NOT NULL,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    root_path TEXT NOT NULL,
    dirty_revision INTEGER NOT NULL CHECK(dirty_revision > 0),
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS review_live_root_reconciliation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id TEXT NOT NULL UNIQUE,
    request_signature TEXT NOT NULL,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    root_path TEXT NOT NULL,
    dirty_revision INTEGER NOT NULL CHECK(dirty_revision > 0),
    expected_review_revision INTEGER NOT NULL CHECK(expected_review_revision >= 0),
    start_after_file_id INTEGER,
    item_count INTEGER NOT NULL CHECK(item_count BETWEEN 0 AND 200),
    present_count INTEGER NOT NULL CHECK(present_count >= 0),
    changed_count INTEGER NOT NULL CHECK(changed_count >= 0),
    missing_count INTEGER NOT NULL CHECK(missing_count >= 0),
    unavailable_count INTEGER NOT NULL CHECK(unavailable_count >= 0),
    invalidated_decision_count INTEGER NOT NULL CHECK(invalidated_decision_count >= 0),
    next_cursor_file_id INTEGER,
    reconciliation_required INTEGER NOT NULL CHECK(reconciliation_required IN (0, 1)),
    reconciled_item_count INTEGER NOT NULL CHECK(reconciled_item_count >= 0),
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS review_live_root_reconciliation_item (
    reconciliation_id INTEGER NOT NULL
        REFERENCES review_live_root_reconciliation(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0 AND ordinal < 200),
    file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK(state IN ('present', 'changed', 'missing', 'unavailable')),
    reason_code TEXT NOT NULL,
    observed_file_identity TEXT,
    observed_file_size INTEGER CHECK(observed_file_size IS NULL OR observed_file_size >= 0),
    observed_last_modified INTEGER,
    os_error INTEGER,
    decision_invalidated INTEGER NOT NULL CHECK(decision_invalidated IN (0, 1)),
    invalidated_decision TEXT
        CHECK(invalidated_decision IS NULL OR invalidated_decision IN ('keep', 'remove')),
    observed_at TEXT NOT NULL,
    PRIMARY KEY(reconciliation_id, ordinal),
    UNIQUE(reconciliation_id, file_id)
);

CREATE TABLE IF NOT EXISTS review_live_file_state (
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
    validation_id INTEGER REFERENCES review_live_validation(id) ON DELETE CASCADE,
    reconciliation_id INTEGER REFERENCES review_live_root_reconciliation(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK(state IN ('present', 'changed', 'missing', 'unavailable')),
    reason_code TEXT NOT NULL,
    observed_file_identity TEXT,
    observed_file_size INTEGER CHECK(observed_file_size IS NULL OR observed_file_size >= 0),
    observed_last_modified INTEGER,
    os_error INTEGER,
    decision_invalidated INTEGER NOT NULL CHECK(decision_invalidated IN (0, 1)),
    invalidated_decision TEXT CHECK(invalidated_decision IS NULL OR invalidated_decision IN ('keep', 'remove')),
    observed_at TEXT NOT NULL,
    PRIMARY KEY(run_id, file_id),
    CHECK((validation_id IS NOT NULL AND reconciliation_id IS NULL)
          OR (validation_id IS NULL AND reconciliation_id IS NOT NULL))
);

CREATE VIEW IF NOT EXISTS recorded_review_decision AS
SELECT manual.plan_id, manual.group_id, manual.file_id, manual.decision,
       'manual' AS provenance, manual.decided_at, NULL AS application_id
FROM review_decision manual
LEFT JOIN (
    SELECT rule_decision.plan_id, rule_decision.file_id, application.applied_revision
    FROM review_rule_decision rule_decision
    JOIN review_rule_application application
      ON application.id = rule_decision.application_id AND application.state = 'active'
) rule ON rule.plan_id = manual.plan_id AND rule.file_id = manual.file_id
WHERE manual.decision IN ('keep', 'remove')
   OR rule.file_id IS NULL
   OR manual.manual_revision > rule.applied_revision
UNION ALL
SELECT rule_decision.plan_id, rule_decision.group_id, rule_decision.file_id,
       rule_decision.decision, 'rule' AS provenance, rule_decision.decided_at,
       rule_decision.application_id
FROM review_rule_decision rule_decision
JOIN review_rule_application application
  ON application.id = rule_decision.application_id AND application.state = 'active'
WHERE NOT EXISTS (
    SELECT 1 FROM review_decision manual
    WHERE manual.plan_id = rule_decision.plan_id
      AND manual.file_id = rule_decision.file_id
      AND (manual.decision IN ('keep', 'remove')
           OR manual.manual_revision > application.applied_revision)
);

CREATE VIEW IF NOT EXISTS effective_review_decision AS
SELECT recorded.plan_id, recorded.group_id, recorded.file_id, recorded.decision,
       recorded.provenance, recorded.decided_at, recorded.application_id
FROM recorded_review_decision recorded
JOIN review_plan plan ON plan.id = recorded.plan_id
LEFT JOIN review_live_file_state live
  ON live.run_id = plan.run_id AND live.file_id = recorded.file_id
WHERE live.decision_invalidated IS NULL OR live.decision_invalidated = 0;

-- A preflight is an immutable review-revision snapshot plus mutable observations for exactly one
-- validation generation. It is intentionally independent of future file-operation state.
CREATE TABLE IF NOT EXISTS preflight (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id TEXT NOT NULL UNIQUE,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
    review_revision INTEGER NOT NULL CHECK(review_revision >= 0),
    snapshot_signature TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending', 'running', 'cancelling', 'completed', 'cancelled', 'interrupted', 'failed')),
    logical_removal_count INTEGER NOT NULL CHECK(logical_removal_count >= 0),
    physical_removal_count INTEGER NOT NULL CHECK(physical_removal_count >= 0),
    folder_removal_count INTEGER NOT NULL CHECK(folder_removal_count >= 0),
    affected_group_count INTEGER NOT NULL CHECK(affected_group_count >= 0),
    planned_removal_bytes INTEGER NOT NULL CHECK(planned_removal_bytes >= 0),
    total_item_count INTEGER NOT NULL CHECK(total_item_count >= 0),
    processed_item_count INTEGER NOT NULL DEFAULT 0 CHECK(processed_item_count >= 0),
    ready_count INTEGER NOT NULL DEFAULT 0 CHECK(ready_count >= 0),
    changed_count INTEGER NOT NULL DEFAULT 0 CHECK(changed_count >= 0),
    missing_count INTEGER NOT NULL DEFAULT 0 CHECK(missing_count >= 0),
    unavailable_count INTEGER NOT NULL DEFAULT 0 CHECK(unavailable_count >= 0),
    conflict_count INTEGER NOT NULL DEFAULT 0 CHECK(conflict_count >= 0),
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    error_code TEXT,
    error_detail TEXT
);

CREATE TABLE IF NOT EXISTS preflight_item (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    preflight_id INTEGER NOT NULL REFERENCES preflight(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    target_kind TEXT NOT NULL CHECK(target_kind IN ('file', 'folder')),
    target_role TEXT NOT NULL CHECK(target_role IN ('remove', 'survivor')),
    physical_key TEXT NOT NULL,
    group_id INTEGER REFERENCES duplicate_group(id) ON DELETE CASCADE,
    folder_group_id INTEGER REFERENCES duplicate_folder_group(id) ON DELETE CASCADE,
    folder_member_id INTEGER REFERENCES duplicate_folder_group_member(id) ON DELETE CASCADE,
    snapshot_file_id INTEGER REFERENCES scanned_file(id) ON DELETE CASCADE,
    snapshot_directory_id INTEGER REFERENCES directory_node(id) ON DELETE CASCADE,
    snapshot_path TEXT NOT NULL,
    snapshot_file_identity TEXT,
    snapshot_file_size INTEGER CHECK(snapshot_file_size IS NULL OR snapshot_file_size >= 0),
    snapshot_last_modified INTEGER,
    snapshot_content_hash INTEGER,
    snapshot_structural_fingerprint TEXT,
    snapshot_verified_fingerprint TEXT,
    outcome TEXT NOT NULL DEFAULT 'pending'
        CHECK(outcome IN ('pending', 'ready', 'changed', 'missing', 'unavailable', 'conflict')),
    reason_code TEXT,
    observed_file_identity TEXT,
    observed_file_size INTEGER,
    observed_last_modified INTEGER,
    observed_content_hash INTEGER,
    os_error INTEGER,
    observed_at TEXT,
    UNIQUE(preflight_id, ordinal)
);

-- One physical item may represent multiple hard-link aliases and multiple contributing review
-- decisions. Sources preserve those immutable logical paths without causing duplicate I/O.
CREATE TABLE IF NOT EXISTS preflight_item_source (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL REFERENCES preflight_item(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL CHECK(source_kind IN ('file_decision', 'folder_decision', 'survivor')),
    group_id INTEGER REFERENCES duplicate_group(id) ON DELETE CASCADE,
    folder_group_id INTEGER REFERENCES duplicate_folder_group(id) ON DELETE CASCADE,
    folder_member_id INTEGER REFERENCES duplicate_folder_group_member(id) ON DELETE CASCADE,
    file_id INTEGER REFERENCES scanned_file(id) ON DELETE CASCADE,
    directory_id INTEGER REFERENCES directory_node(id) ON DELETE CASCADE,
    snapshot_path TEXT NOT NULL
);

-- A non-scheduling, revision-bound Recycle Bin intent. Schema v10 persists the complete
-- operation contract and injected reports, but does not itself perform filesystem or Shell work.
CREATE TABLE IF NOT EXISTS recycle_operation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id TEXT NOT NULL UNIQUE,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
    preflight_id INTEGER NOT NULL REFERENCES preflight(id) ON DELETE CASCADE,
    review_revision INTEGER NOT NULL CHECK(review_revision >= 0),
    preflight_snapshot_signature TEXT NOT NULL,
    intent_signature TEXT NOT NULL,
    policy_version INTEGER NOT NULL DEFAULT 1 CHECK(policy_version > 0),
    status TEXT NOT NULL DEFAULT 'prepared'
        CHECK(status IN ('prepared', 'awaiting_confirmation', 'submitted', 'executing',
                         'cancelling', 'expired', 'cancelled', 'completed',
                         'partially_completed', 'failed', 'recovery_required')),
    logical_removal_count INTEGER NOT NULL CHECK(logical_removal_count >= 0),
    shell_item_count INTEGER NOT NULL CHECK(shell_item_count >= 0),
    physical_item_count INTEGER NOT NULL CHECK(physical_item_count >= 0),
    folder_item_count INTEGER NOT NULL CHECK(folder_item_count >= 0),
    affected_group_count INTEGER NOT NULL CHECK(affected_group_count >= 0),
    planned_removal_bytes INTEGER NOT NULL CHECK(planned_removal_bytes >= 0),
    affected_location_count INTEGER NOT NULL DEFAULT 0 CHECK(affected_location_count >= 0),
    exclusion_count INTEGER NOT NULL DEFAULT 0 CHECK(exclusion_count >= 0),
    prepared_at TEXT NOT NULL,
    confirmation_signature TEXT,
    confirmation_expires_at TEXT,
    submitted_at TEXT,
    completed_at TEXT,
    cancellation_requested INTEGER NOT NULL DEFAULT 0 CHECK(cancellation_requested IN (0, 1)),
    error_code TEXT,
    error_detail TEXT
);

CREATE TABLE IF NOT EXISTS recycle_operation_batch (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recycle_operation_id INTEGER NOT NULL REFERENCES recycle_operation(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    item_signature TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending', 'admitted', 'shell_started', 'reported', 'skipped', 'ambiguous')),
    admission_expires_at TEXT,
    shell_attempt_id TEXT,
    started_at TEXT,
    reported_at TEXT,
    UNIQUE(recycle_operation_id, ordinal)
);

CREATE TABLE IF NOT EXISTS recycle_operation_item (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recycle_operation_id INTEGER NOT NULL REFERENCES recycle_operation(id) ON DELETE CASCADE,
    batch_id INTEGER NOT NULL REFERENCES recycle_operation_batch(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    preflight_item_id INTEGER NOT NULL REFERENCES preflight_item(id) ON DELETE CASCADE,
    preflight_source_id INTEGER REFERENCES preflight_item_source(id) ON DELETE CASCADE,
    target_kind TEXT NOT NULL CHECK(target_kind IN ('file', 'folder')),
    physical_key TEXT NOT NULL,
    snapshot_path TEXT NOT NULL,
    group_id INTEGER REFERENCES duplicate_group(id) ON DELETE CASCADE,
    folder_group_id INTEGER REFERENCES duplicate_folder_group(id) ON DELETE CASCADE,
    folder_member_id INTEGER REFERENCES duplicate_folder_group_member(id) ON DELETE CASCADE,
    snapshot_file_id INTEGER REFERENCES scanned_file(id) ON DELETE CASCADE,
    snapshot_directory_id INTEGER REFERENCES directory_node(id) ON DELETE CASCADE,
    planned_bytes INTEGER NOT NULL DEFAULT 0 CHECK(planned_bytes >= 0),
    eligibility_status TEXT NOT NULL DEFAULT 'pending'
        CHECK(eligibility_status IN ('pending', 'eligible', 'non_recyclable')),
    eligibility_code TEXT,
    result_status TEXT NOT NULL DEFAULT 'pending'
        CHECK(result_status IN ('pending', 'recycled', 'failed', 'cancelled', 'unknown')),
    result_code TEXT,
    shell_hresult INTEGER,
    recycled_item_present INTEGER CHECK(recycled_item_present IS NULL OR recycled_item_present IN (0, 1)),
    result_at TEXT,
    UNIQUE(recycle_operation_id, ordinal)
);

CREATE TABLE IF NOT EXISTS recycle_operation_report (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recycle_operation_id INTEGER NOT NULL REFERENCES recycle_operation(id) ON DELETE CASCADE,
    batch_id INTEGER REFERENCES recycle_operation_batch(id) ON DELETE CASCADE,
    report_operation_id TEXT NOT NULL UNIQUE,
    report_kind TEXT NOT NULL CHECK(report_kind IN ('eligibility', 'confirmation', 'batch_begin', 'result', 'recovery')),
    payload_signature TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS recycle_operation_recovery (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recycle_operation_id INTEGER NOT NULL REFERENCES recycle_operation(id) ON DELETE CASCADE,
    batch_id INTEGER REFERENCES recycle_operation_batch(id) ON DELETE CASCADE,
    item_id INTEGER REFERENCES recycle_operation_item(id) ON DELETE CASCADE,
    reason_code TEXT NOT NULL,
    detail TEXT,
    created_at TEXT NOT NULL
);

-- Append-only operator attestations for recovery-required unknown items. These records never
-- rewrite Shell outcomes and contain no live filesystem or Recycle Bin inspection results.
CREATE TABLE IF NOT EXISTS recovery_review_observation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id TEXT NOT NULL UNIQUE,
    payload_signature TEXT NOT NULL,
    recycle_operation_id INTEGER NOT NULL
        REFERENCES recycle_operation(id) ON DELETE CASCADE,
    item_id INTEGER NOT NULL REFERENCES recycle_operation_item(id) ON DELETE CASCADE,
    observation TEXT NOT NULL
        CHECK(observation IN ('observed_in_recycle_bin', 'observed_at_source',
                              'observed_in_both', 'observed_in_neither',
                              'deferred_unresolved')),
    observed_at TEXT NOT NULL,
    note TEXT,
    evidence_version INTEGER NOT NULL CHECK(evidence_version = 1),
    supersedes_observation_id INTEGER UNIQUE
        REFERENCES recovery_review_observation(id) ON DELETE CASCADE,
    correction_reason TEXT,
    created_at TEXT NOT NULL,
    CHECK((supersedes_observation_id IS NULL AND correction_reason IS NULL)
          OR (supersedes_observation_id IS NOT NULL AND correction_reason IS NOT NULL))
);

-- Structured, run-owned records for whole subtrees pruned before filesystem content access.
CREATE TABLE IF NOT EXISTS run_exclusion (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    provider_id TEXT,
    provider_name TEXT,
    occurrence_count INTEGER NOT NULL DEFAULT 1 CHECK(occurrence_count > 0),
    UNIQUE(run_id, path, reason_code)
);

-- Bounded, run-owned warning aggregates. Exact occurrence counts remain drillable without
-- retaining one row per filesystem failure; examples_json is capped by the engine at three.
CREATE TABLE IF NOT EXISTS run_warning_aggregate (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    phase TEXT NOT NULL,
    category TEXT NOT NULL,
    code TEXT NOT NULL,
    severity TEXT NOT NULL CHECK(severity = 'warning'),
    message TEXT NOT NULL,
    occurrence_count INTEGER NOT NULL CHECK(occurrence_count > 0),
    examples_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(run_id, code)
);

CREATE INDEX IF NOT EXISTS idx_session_name ON scan_session(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_run_session ON scan_run(session_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_run_status ON scan_run(status);
CREATE INDEX IF NOT EXISTS idx_file_run_size ON scanned_file(run_id, file_size);
CREATE INDEX IF NOT EXISTS idx_file_run_hash ON scanned_file(run_id, content_hash) WHERE content_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_file_run_parent ON scanned_file(run_id, parent_dir);
CREATE INDEX IF NOT EXISTS idx_file_run_path ON scanned_file(run_id, canonical_path);
CREATE INDEX IF NOT EXISTS idx_file_run_path_unicode_nocase
    ON scanned_file(run_id, canonical_path COLLATE UNICODE_NOCASE);
CREATE INDEX IF NOT EXISTS idx_group_run ON duplicate_group(run_id);
CREATE INDEX IF NOT EXISTS idx_group_run_wasted ON duplicate_group(run_id, wasted_bytes DESC);
CREATE INDEX IF NOT EXISTS idx_group_run_size ON duplicate_group(run_id, file_size, id);
CREATE INDEX IF NOT EXISTS idx_group_run_count ON duplicate_group(run_id, file_count, id);
CREATE INDEX IF NOT EXISTS idx_group_member_group ON duplicate_group_member(group_id);
CREATE INDEX IF NOT EXISTS idx_group_member_file ON duplicate_group_member(file_id, group_id);
CREATE INDEX IF NOT EXISTS idx_folder_group_run_bytes ON duplicate_folder_group(run_id, is_suppressed, total_size DESC, id);
CREATE INDEX IF NOT EXISTS idx_folder_group_run_count ON duplicate_folder_group(run_id, is_suppressed, folder_count, id);
CREATE INDEX IF NOT EXISTS idx_dir_run_parent ON directory_node(run_id, parent_id);
CREATE INDEX IF NOT EXISTS idx_dir_fingerprint ON directory_fingerprint(content_fingerprint);
CREATE INDEX IF NOT EXISTS idx_dir_similarity_run_score ON directory_similarity(run_id, similarity_score DESC);
CREATE INDEX IF NOT EXISTS idx_folder_group_member_group ON duplicate_folder_group_member(group_id, id);
CREATE INDEX IF NOT EXISTS idx_folder_group_member_directory
    ON duplicate_folder_group_member(directory_id, group_id, id);
CREATE INDEX IF NOT EXISTS idx_run_exclusion_run_path ON run_exclusion(run_id, path COLLATE NOCASE, id);
CREATE INDEX IF NOT EXISTS idx_run_warning_run_id ON run_warning_aggregate(run_id, id);
CREATE INDEX IF NOT EXISTS idx_run_warning_run_phase
    ON run_warning_aggregate(run_id, phase COLLATE UNICODE_NOCASE, id);
CREATE INDEX IF NOT EXISTS idx_run_warning_run_occurrence
    ON run_warning_aggregate(run_id, occurrence_count, id);
CREATE INDEX IF NOT EXISTS idx_run_warning_run_message
    ON run_warning_aggregate(run_id, message COLLATE UNICODE_NOCASE, id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_review_plan_one_active_run
    ON review_plan(run_id) WHERE state = 'active';
CREATE INDEX IF NOT EXISTS idx_review_decision_plan_group
    ON review_decision(plan_id, group_id, file_id);
CREATE INDEX IF NOT EXISTS idx_review_decision_plan_decision
    ON review_decision(plan_id, decision, group_id);
CREATE INDEX IF NOT EXISTS idx_review_live_validation_run
    ON review_live_validation(run_id, group_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_review_live_validation_item_file
    ON review_live_validation_item(file_id, validation_id DESC);
CREATE INDEX IF NOT EXISTS idx_review_live_file_state_invalidated
    ON review_live_file_state(run_id, decision_invalidated, file_id);
CREATE INDEX IF NOT EXISTS idx_review_live_root_state_dirty
    ON review_live_root_state(run_id, state, dirty_revision, root_path COLLATE UNICODE_NOCASE);
CREATE INDEX IF NOT EXISTS idx_review_live_root_reconciliation_run
    ON review_live_root_reconciliation(run_id, root_path COLLATE UNICODE_NOCASE, id DESC);
CREATE INDEX IF NOT EXISTS idx_review_live_root_reconciliation_item_file
    ON review_live_root_reconciliation_item(file_id, reconciliation_id DESC);
CREATE INDEX IF NOT EXISTS idx_review_command_plan_operation
    ON review_command(plan_id, operation_id);
CREATE INDEX IF NOT EXISTS idx_review_folder_decision_plan_group
    ON review_folder_decision(plan_id, folder_group_id, folder_member_id);
CREATE INDEX IF NOT EXISTS idx_review_folder_decision_plan_decision
    ON review_folder_decision(plan_id, decision, directory_id);
CREATE INDEX IF NOT EXISTS idx_review_folder_command_plan_operation
    ON review_folder_command(plan_id, operation_id);
CREATE INDEX IF NOT EXISTS idx_preference_rule_state_name
    ON preference_rule(state, name COLLATE UNICODE_NOCASE, id);
CREATE INDEX IF NOT EXISTS idx_preference_rule_root_path
    ON preference_rule_root(rule_id, root_path COLLATE UNICODE_NOCASE, ordinal);
CREATE INDEX IF NOT EXISTS idx_review_rule_application_plan_state
    ON review_rule_application(plan_id, state, id DESC);
CREATE INDEX IF NOT EXISTS idx_review_rule_application_run_rule
    ON review_rule_application(run_id, rule_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_review_rule_decision_application
    ON review_rule_decision(application_id, group_id, file_id);
CREATE INDEX IF NOT EXISTS idx_review_rule_decision_plan_group
    ON review_rule_decision(plan_id, group_id, decision, file_id);
CREATE INDEX IF NOT EXISTS idx_review_rule_reversal_plan_operation
    ON review_rule_reversal_command(plan_id, operation_id);
CREATE INDEX IF NOT EXISTS idx_preflight_run_created
    ON preflight(run_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_preflight_status
    ON preflight(status, id);
CREATE INDEX IF NOT EXISTS idx_preflight_item_page
    ON preflight_item(preflight_id, outcome, target_role, target_kind, snapshot_path COLLATE UNICODE_NOCASE, id);
CREATE INDEX IF NOT EXISTS idx_preflight_item_pending
    ON preflight_item(preflight_id, ordinal) WHERE outcome = 'pending';
CREATE INDEX IF NOT EXISTS idx_preflight_item_source_item
    ON preflight_item_source(item_id, id);
CREATE INDEX IF NOT EXISTS idx_recycle_operation_run_created
    ON recycle_operation(run_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_recycle_operation_status
    ON recycle_operation(status, id);
CREATE INDEX IF NOT EXISTS idx_recycle_operation_preflight
    ON recycle_operation(preflight_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_recycle_operation_batch_state
    ON recycle_operation_batch(recycle_operation_id, status, ordinal);
CREATE INDEX IF NOT EXISTS idx_recycle_operation_item_page
    ON recycle_operation_item(recycle_operation_id, result_status, eligibility_status,
                              target_kind, snapshot_path COLLATE UNICODE_NOCASE, id);
CREATE INDEX IF NOT EXISTS idx_recycle_operation_item_batch
    ON recycle_operation_item(batch_id, ordinal);
CREATE INDEX IF NOT EXISTS idx_recycle_operation_report_operation
    ON recycle_operation_report(recycle_operation_id, id);
CREATE INDEX IF NOT EXISTS idx_recycle_operation_recovery_operation
    ON recycle_operation_recovery(recycle_operation_id, id);
CREATE INDEX IF NOT EXISTS idx_recovery_review_operation_item
    ON recovery_review_observation(recycle_operation_id, item_id, id);
CREATE INDEX IF NOT EXISTS idx_recovery_review_supersession
    ON recovery_review_observation(supersedes_observation_id);
