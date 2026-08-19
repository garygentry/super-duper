use rusqlite::{params, Connection, Error as SqlError};
use std::sync::{atomic::AtomicBool, Mutex};
use super_duper_core::storage::models::{
    CloudDetectionStatus, CloudPolicy, DuplicateFileDriveFacetPageQuery,
    DuplicateFileDriveFacetSortField, DuplicateFileExtensionMatchMode, DuplicateFileGroupFilter,
    DuplicateFileGroupPageQuery, DuplicateFileGroupSortField, DuplicateFileMemberFilter,
    DuplicateFileMemberPageQuery, DuplicateFileMemberSortField, DuplicateFilePathMatchMode,
    DuplicateFileSelectedRootFacetPageQuery, DuplicateFileSelectedRootFacetSortField,
    ExactFolderGroupInsert, PageCursor, PageCursorValue, PreferencePreviewScope,
    RegisteredCloudLocation, ReviewDecisionKind, RunExclusionInsert, RunParameters, ScannedFile,
    SortDirection,
};
use super_duper_core::storage::preference::PreferenceError;
use super_duper_core::storage::review::ReviewError;
use super_duper_core::storage::sqlite::CURRENT_SCHEMA_VERSION;
use super_duper_core::storage::Database;
use tempfile::tempdir;
#[cfg(windows)]
use winapi::um::processthreadsapi::GetCurrentProcess;
#[cfg(windows)]
use winapi::um::psapi::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};

static LARGE_FIXTURE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn parameters(roots: &[&str], ignores: &[&str]) -> RunParameters {
    RunParameters {
        roots: roots.iter().map(|value| value.to_string()).collect(),
        ignore_patterns: ignores.iter().map(|value| value.to_string()).collect(),
        directory_similarity_threshold_millis: 500,
        cloud_policy: Default::default(),
        manual_location_exclusions: Vec::new(),
        registered_cloud_locations: Vec::new(),
        cloud_detection_status: Default::default(),
    }
}

fn session_and_run(db: &Database, name: &str, roots: &[&str]) -> (i64, i64) {
    let params = parameters(roots, &[]);
    let session_id = db
        .create_session(name, &params.roots, &params.ignore_patterns)
        .unwrap();
    let run_id = db.create_scan_run(session_id, &params, "test").unwrap();
    db.start_scan_run(run_id).unwrap();
    (session_id, run_id)
}

fn file(run_id: i64, path: &str, size: i64, hash: i64) -> ScannedFile {
    ScannedFile {
        id: 0,
        run_id,
        root_path: "/root".to_string(),
        canonical_path: path.to_string(),
        relative_path: path.trim_start_matches("/root/").to_string(),
        file_name: path.rsplit('/').next().unwrap_or(path).to_string(),
        parent_dir: path
            .rsplit_once('/')
            .map(|pair| pair.0)
            .unwrap_or("")
            .to_string(),
        drive_letter: String::new(),
        file_size: size,
        last_modified: 1_700_000_000,
        partial_hash: None,
        content_hash: Some(hash),
        file_identity: None,
        warning_message: None,
        marked_deleted: false,
    }
}

#[test]
fn schema_version_is_explicit_and_current() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
}

#[test]
fn version_six_migrates_named_preference_rules_transactionally() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("v6-preferences.db");
    let db = Database::open(path.to_str().unwrap()).unwrap();
    db.connection()
        .execute_batch(
            "DROP TABLE preference_rule_command;
             DROP TABLE preference_rule_root;
             DROP TABLE preference_rule;
             PRAGMA user_version = 6;",
        )
        .unwrap();
    drop(db);

    let migrated = Database::open(path.to_str().unwrap()).unwrap();
    assert_eq!(migrated.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    for table in [
        "preference_rule",
        "preference_rule_root",
        "preference_rule_command",
    ] {
        let exists: bool = migrated
            .connection()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                params![table],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "missing migrated table {table}");
    }
    assert_eq!(
        migrated.get_review_plan_view(999).unwrap_err().to_string(),
        "scan run 999 was not found"
    );
}

#[test]
fn named_preference_rules_are_revisioned_idempotent_and_persistent() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("preference-rules.db");
    let db = Database::open(path.to_str().unwrap()).unwrap();
    let created = db
        .save_preference_rule(
            "create-rule",
            None,
            "Primary libraries",
            &["C:/Photos".to_owned(), "D:/Backup".to_owned()],
            0,
        )
        .unwrap();
    assert_eq!(created.rule.revision, 1);
    assert!(!created.replayed);
    let replay = db
        .save_preference_rule(
            "create-rule",
            None,
            "Primary libraries",
            &["C:/Photos".to_owned(), "D:/Backup".to_owned()],
            0,
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.rule, created.rule);
    assert!(matches!(
        db.save_preference_rule("create-rule", None, "Changed", &["C:/Photos".to_owned()], 0,),
        Err(PreferenceError::IdempotencyConflict { .. })
    ));
    let updated = db
        .save_preference_rule(
            "update-rule",
            Some(created.rule.id),
            "Primary libraries",
            &["D:/Backup".to_owned(), "C:/Photos".to_owned()],
            1,
        )
        .unwrap();
    assert_eq!(updated.rule.revision, 2);
    assert_eq!(updated.rule.roots[0], "D:/Backup");
    let historical_replay = db
        .save_preference_rule(
            "create-rule",
            None,
            "Primary libraries",
            &["C:/Photos".to_owned(), "D:/Backup".to_owned()],
            0,
        )
        .unwrap();
    assert!(historical_replay.replayed);
    assert_eq!(historical_replay.rule.revision, 1);
    assert_eq!(historical_replay.rule.roots[0], "C:/Photos");
    assert!(matches!(
        db.save_preference_rule(
            "stale-rule",
            Some(created.rule.id),
            "Primary libraries",
            &["C:/Photos".to_owned()],
            1,
        ),
        Err(PreferenceError::StaleRuleRevision { current: 2, .. })
    ));
    drop(db);

    let reopened = Database::open(path.to_str().unwrap()).unwrap();
    assert_eq!(
        reopened.get_preference_rule(created.rule.id).unwrap(),
        updated.rule
    );
    let (listed, total) = reopened.list_preference_rules(0, 200).unwrap();
    assert_eq!(total, 1);
    assert_eq!(listed[0].root_count, 2);
}

#[test]
fn preferred_root_preview_handles_ties_missing_roots_manual_precedence_and_physical_aliases() {
    let db = Database::open_in_memory().unwrap();
    let (_, run_id) = session_and_run(&db, "Preference preview", &["C:/Preferred", "D:/Backup"]);
    let mut files = vec![
        file(run_id, "C:/Preferred/top.bin", 100, 101),
        file(run_id, "C:/Preferred/top-tie.bin", 100, 101),
        file(run_id, "D:/Backup/alias.bin", 100, 101),
        file(run_id, "E:/Unranked/other.bin", 100, 101),
        file(run_id, "E:/Unranked/none.bin", 50, 202),
        file(run_id, "F:/Elsewhere/none.bin", 50, 202),
    ];
    for item in &mut files {
        item.root_path = item.canonical_path[..item.canonical_path.rfind('/').unwrap()].to_owned();
    }
    files[0].root_path = "C:/Preferred".to_owned();
    files[1].root_path = "C:/Preferred".to_owned();
    files[2].root_path = "D:/Backup".to_owned();
    files[3].root_path = "E:/Unranked".to_owned();
    files[4].root_path = "E:/Unranked".to_owned();
    files[5].root_path = "F:/Elsewhere".to_owned();
    files[0].file_identity = Some("physical-top".to_owned());
    files[2].file_identity = Some("physical-top".to_owned());
    files[1].file_identity = Some("physical-tie".to_owned());
    files[3].file_identity = Some("physical-other".to_owned());
    db.insert_scanned_files(&files).unwrap();
    db.insert_duplicate_groups(
        run_id,
        &[
            (
                101,
                100,
                files[..4]
                    .iter()
                    .map(|item| item.canonical_path.clone())
                    .collect(),
            ),
            (
                202,
                50,
                files[4..]
                    .iter()
                    .map(|item| item.canonical_path.clone())
                    .collect(),
            ),
        ],
    )
    .unwrap();
    db.complete_scan_run(run_id, 6, 500, 6, 2, 0, 350, 0)
        .unwrap();
    let group_id: i64 = db
        .connection()
        .query_row(
            "SELECT id FROM duplicate_group WHERE run_id = ?1 AND content_hash = 101",
            params![run_id],
            |row| row.get(0),
        )
        .unwrap();
    let manual_keep_file: i64 = db
        .connection()
        .query_row(
            "SELECT id FROM scanned_file WHERE run_id = ?1 AND canonical_path = 'D:/Backup/alias.bin'",
            params![run_id],
            |row| row.get(0),
        )
        .unwrap();
    db.set_review_decision(
        "manual-keep",
        run_id,
        group_id,
        manual_keep_file,
        ReviewDecisionKind::Keep,
        0,
    )
    .unwrap();
    let rule = db
        .save_preference_rule(
            "preview-rule",
            None,
            "Preferred roots",
            &[
                "C:/Preferred".to_owned(),
                "D:/Backup".to_owned(),
                "Z:/Missing".to_owned(),
            ],
            0,
        )
        .unwrap()
        .rule;

    let page = db
        .page_preference_preview(
            run_id,
            rule.id,
            rule.revision,
            1,
            &PreferencePreviewScope::CompletedRun,
            50,
            None,
        )
        .unwrap();
    assert_eq!(page.total, 1, "unranked-only set is not an affected row");
    assert_eq!(page.groups[0].group_id, group_id);
    assert_eq!(page.groups[0].tied_preferred_path_count, 2);
    assert_eq!(page.groups[0].proposed_remove_path_count, 1);
    assert_eq!(page.groups[0].proposed_remove_physical_item_count, 1);
    assert_eq!(page.groups[0].proposed_remove_bytes, 100);
    assert_eq!(page.groups[0].manual_keep_count, 1);
    assert_eq!(page.summary.no_ranked_root_group_count, 1);
    assert_eq!(page.summary.missing_rule_root_count, 1);
    assert_eq!(page.summary.tied_group_count, 1);
    assert_eq!(page.summary.scoped_physical_item_count, 5);
    assert_eq!(page.summary.proposed_remove_physical_item_count, 1);

    assert!(matches!(
        db.page_preference_preview(
            run_id,
            rule.id,
            rule.revision,
            0,
            &PreferencePreviewScope::SelectedSets(vec![group_id]),
            10,
            None,
        ),
        Err(PreferenceError::StaleReviewRevision { current: 1, .. })
    ));
}

#[test]
fn preferred_root_preview_reports_folder_keep_and_folder_survivor_conflicts() {
    let db = Database::open_in_memory().unwrap();
    let (run_id, original_group_id, folder_members, _) = completed_folder_review_fixture(&db);
    db.connection()
        .execute(
            "UPDATE scanned_file
             SET root_path = CASE
                 WHEN canonical_path LIKE '/root/Copy A/%' THEN '/root/Copy A'
                 ELSE '/root/Copy B' END
             WHERE run_id = ?1",
            params![run_id],
        )
        .unwrap();
    db.set_review_folder_decision(
        "protect-copy-b",
        run_id,
        db.connection()
            .query_row(
                "SELECT group_id FROM duplicate_folder_group_member WHERE id = ?1",
                params![folder_members[1]],
                |row| row.get(0),
            )
            .unwrap(),
        folder_members[1],
        ReviewDecisionKind::Keep,
        0,
    )
    .unwrap();
    let rule = db
        .save_preference_rule(
            "folder-keep-preview",
            None,
            "Prefer copy A",
            &["/root/Copy A".to_owned(), "/root/Copy B".to_owned()],
            0,
        )
        .unwrap()
        .rule;
    let keep_conflict = db
        .page_preference_preview(
            run_id,
            rule.id,
            rule.revision,
            1,
            &PreferencePreviewScope::CompletedRun,
            50,
            None,
        )
        .unwrap();
    assert_eq!(keep_conflict.groups[0].group_id, original_group_id);
    assert_eq!(keep_conflict.groups[0].status.as_str(), "blocked");
    assert_eq!(
        keep_conflict.groups[0].explanation_code,
        "manual_folder_keep_conflict"
    );
    assert_eq!(keep_conflict.summary.overlap_conflict_count, 1);

    let db = Database::open_in_memory().unwrap();
    let (run_id, original_group_id, _, _) = completed_folder_review_fixture(&db);
    let mut second_a = file(run_id, "/root/Copy A/second.bin", 80, 303);
    second_a.root_path = "/root/Copy A".to_owned();
    second_a.file_identity = Some("physical-second-a".to_owned());
    let mut second_b = file(run_id, "/root/Copy B/second.bin", 80, 303);
    second_b.root_path = "/root/Copy B".to_owned();
    second_b.file_identity = Some("physical-second-b".to_owned());
    db.insert_scanned_files(&[second_a, second_b]).unwrap();
    db.insert_duplicate_groups(
        run_id,
        &[(
            303,
            80,
            vec![
                "/root/Copy A/second.bin".to_owned(),
                "/root/Copy B/second.bin".to_owned(),
            ],
        )],
    )
    .unwrap();
    db.connection()
        .execute_batch(
            "UPDATE scanned_file
             SET root_path = CASE
                 WHEN canonical_path LIKE '/root/Copy A/%' THEN '/root/Copy A'
                 ELSE '/root/Copy B' END;
             UPDATE directory_node SET file_count = 2, total_size = 180;
             UPDATE duplicate_folder_group SET file_count = 2, total_size = 180;",
        )
        .unwrap();
    let second_group_id: i64 = db
        .connection()
        .query_row(
            "SELECT id FROM duplicate_group WHERE run_id = ?1 AND content_hash = 303",
            params![run_id],
            |row| row.get(0),
        )
        .unwrap();
    let second_a_id: i64 = db
        .connection()
        .query_row(
            "SELECT id FROM scanned_file WHERE run_id = ?1 AND canonical_path = '/root/Copy A/second.bin'",
            params![run_id],
            |row| row.get(0),
        )
        .unwrap();
    db.set_review_decision(
        "remove-second-a",
        run_id,
        second_group_id,
        second_a_id,
        ReviewDecisionKind::Remove,
        0,
    )
    .unwrap();
    let rule = db
        .save_preference_rule(
            "folder-survivor-preview",
            None,
            "Prefer copy A",
            &["/root/Copy A".to_owned(), "/root/Copy B".to_owned()],
            0,
        )
        .unwrap()
        .rule;
    let survivor_conflict = db
        .page_preference_preview(
            run_id,
            rule.id,
            rule.revision,
            1,
            &PreferencePreviewScope::CompletedRun,
            50,
            None,
        )
        .unwrap();
    let original = survivor_conflict
        .groups
        .iter()
        .find(|group| group.group_id == original_group_id)
        .unwrap();
    assert_eq!(original.status.as_str(), "blocked");
    assert_eq!(original.explanation_code, "folder_survivor_conflict");
    assert_eq!(survivor_conflict.summary.folder_survivor_conflict_count, 1);
}

#[test]
fn newer_schema_is_rejected_without_modification() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("future.db");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch("PRAGMA user_version = 99;")
        .unwrap();
    drop(connection);

    assert!(Database::open(path.to_str().unwrap()).is_err());
    let connection = Connection::open(&path).unwrap();
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 99);
}

#[test]
fn version_three_migrates_cloud_defaults_transactionally() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("v3.db");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "PRAGMA user_version = 3;
             CREATE TABLE scan_session (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL, roots_json TEXT NOT NULL,
                ignore_patterns_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
             );
             CREATE TABLE scan_run (
                id INTEGER PRIMARY KEY, session_id INTEGER NOT NULL REFERENCES scan_session(id),
                parameters_json TEXT NOT NULL, status TEXT NOT NULL, phase TEXT,
                created_at TEXT NOT NULL, started_at TEXT, completed_at TEXT,
                files_discovered INTEGER NOT NULL DEFAULT 0, bytes_discovered INTEGER NOT NULL DEFAULT 0,
                files_hashed INTEGER NOT NULL DEFAULT 0, duplicate_file_groups INTEGER NOT NULL DEFAULT 0,
                duplicate_folder_groups INTEGER NOT NULL DEFAULT 0, wasted_bytes INTEGER NOT NULL DEFAULT 0,
                warning_count INTEGER NOT NULL DEFAULT 0, error_message TEXT, engine_version TEXT NOT NULL
             );
             INSERT INTO scan_session VALUES (1, 'Migrated', '[\"C:/Data\"]', '[]', 'now', 'now');",
        )
        .unwrap();
    drop(connection);

    let db = Database::open(path.to_str().unwrap()).unwrap();
    let session = db.get_session(1).unwrap();

    assert_eq!(db.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    assert_eq!(session.cloud_policy, "exclude_registered_roots");
    assert_eq!(session.manual_location_exclusions_json, "[]");
    assert_eq!(session.registered_cloud_locations_json, "[]");
    assert_eq!(session.cloud_detection_status, "unavailable");
}

#[test]
fn session_names_are_case_insensitively_unique() {
    let db = Database::open_in_memory().unwrap();
    db.create_session("Photos", &["C:/Photos".into()], &[])
        .unwrap();
    let error = db
        .create_session("pHoToS", &["D:/Other".into()], &[])
        .unwrap_err();
    assert!(matches!(error, SqlError::SqliteFailure(_, _)));
}

#[test]
fn multiple_runs_keep_parameter_snapshots_after_session_edit() {
    let db = Database::open_in_memory().unwrap();
    let roots = vec!["C:/One".to_string()];
    let session = db
        .create_session("Reusable", &roots, &["*.tmp".into()])
        .unwrap();
    let first_params = parameters(&["C:/One"], &["*.tmp"]);
    let first = db.create_scan_run(session, &first_params, "test").unwrap();

    db.update_session(
        session,
        "Reusable renamed",
        &["D:/Two".into()],
        &["*.bak".into()],
    )
    .unwrap();
    let second_params = parameters(&["D:/Two"], &["*.bak"]);
    let second = db.create_scan_run(session, &second_params, "test").unwrap();

    assert_ne!(first, second);
    let first_snapshot: RunParameters =
        serde_json::from_str(&db.get_scan_run(first).unwrap().parameters_json).unwrap();
    let second_snapshot: RunParameters =
        serde_json::from_str(&db.get_scan_run(second).unwrap().parameters_json).unwrap();
    assert_eq!(first_snapshot, first_params);
    assert_eq!(second_snapshot, second_params);
    assert_eq!(db.get_session(session).unwrap().roots_json, "[\"D:/Two\"]");
}

#[test]
fn cloud_settings_are_immutable_in_run_snapshots() {
    let db = Database::open_in_memory().unwrap();
    let first_location = RegisteredCloudLocation {
        path: "C:/Cloud".to_owned(),
        provider_id: "provider-one".to_owned(),
        display_name: "Cloud one".to_owned(),
    };
    let session = db
        .create_session_with_cloud_settings(
            "Cloud-safe",
            &["C:/".to_owned()],
            &[],
            CloudPolicy::ExcludeRegisteredRoots,
            &["C:/Manual".to_owned()],
            std::slice::from_ref(&first_location),
            CloudDetectionStatus::Complete,
        )
        .unwrap();
    let first = RunParameters {
        roots: vec!["C:/".to_owned()],
        ignore_patterns: vec![],
        directory_similarity_threshold_millis: 500,
        cloud_policy: CloudPolicy::ExcludeRegisteredRoots,
        manual_location_exclusions: vec!["C:/Manual".to_owned()],
        registered_cloud_locations: vec![first_location],
        cloud_detection_status: CloudDetectionStatus::Complete,
    };
    let run = db.create_scan_run(session, &first, "test").unwrap();

    db.update_session_with_cloud_settings(
        session,
        "Cloud-safe",
        &["D:/".to_owned()],
        &[],
        CloudPolicy::ExcludeRegisteredRoots,
        &[],
        &[],
        CloudDetectionStatus::Complete,
    )
    .unwrap();

    assert_eq!(
        RunParameters::from_json(&db.get_scan_run(run).unwrap().parameters_json).unwrap(),
        first
    );
}

#[test]
fn run_exclusion_pages_are_bounded_and_run_owned() {
    let db = Database::open_in_memory().unwrap();
    let (_, run) = session_and_run(&db, "Excluded", &["/root"]);
    let (_, other_run) = session_and_run(&db, "Other", &["/other"]);
    db.replace_run_exclusions(
        run,
        &[
            RunExclusionInsert {
                path: "/root/cloud-a".to_owned(),
                reason_code: "registered_cloud_root_excluded".to_owned(),
                provider_id: Some("one".to_owned()),
                provider_name: Some("One".to_owned()),
            },
            RunExclusionInsert {
                path: "/root/cloud-b".to_owned(),
                reason_code: "registered_cloud_root_excluded".to_owned(),
                provider_id: Some("two".to_owned()),
                provider_name: Some("Two".to_owned()),
            },
        ],
    )
    .unwrap();

    let (first_page, total) = db.page_run_exclusions(run, 0, 1).unwrap();
    let (other_page, other_total) = db.page_run_exclusions(other_run, 0, 10).unwrap();

    assert_eq!(total, 2);
    assert_eq!(first_page.len(), 1);
    assert_eq!(db.get_scan_run(run).unwrap().excluded_subtree_count, 2);
    assert!(other_page.is_empty());
    assert_eq!(other_total, 0);
}

#[test]
fn result_rows_are_strictly_isolated_by_run() {
    let db = Database::open_in_memory().unwrap();
    let (session, first) = session_and_run(&db, "History", &["/root"]);
    let second = db
        .create_scan_run(session, &parameters(&["/root"], &[]), "test")
        .unwrap();
    db.start_scan_run(second).unwrap();

    for run_id in [first, second] {
        db.insert_scanned_files(&[
            file(run_id, "/root/a.txt", 100, 11),
            file(run_id, "/root/b.txt", 100, 11),
        ])
        .unwrap();
        db.insert_duplicate_groups(
            run_id,
            &[(11, 100, vec!["/root/a.txt".into(), "/root/b.txt".into()])],
        )
        .unwrap();
    }

    let first_group = db.get_duplicate_groups(first, 0, 10).unwrap().remove(0);
    let second_group = db.get_duplicate_groups(second, 0, 10).unwrap().remove(0);
    assert_ne!(first_group.id, second_group.id);
    assert!(db
        .get_files_in_group(first_group.id)
        .unwrap()
        .iter()
        .all(|snapshot| snapshot.run_id == first));
    assert!(db
        .get_files_in_group(second_group.id)
        .unwrap()
        .iter()
        .all(|snapshot| snapshot.run_id == second));

    db.connection()
        .execute(
            "UPDATE scanned_file SET file_size = 999 WHERE run_id = ?1",
            params![second],
        )
        .unwrap();
    let first_size: i64 = db
        .connection()
        .query_row(
            "SELECT file_size FROM scanned_file WHERE run_id = ?1 LIMIT 1",
            params![first],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(first_size, 100);
}

#[test]
fn duplicate_file_keyset_pages_are_stable_filtered_and_run_scoped() {
    let db = Database::open_in_memory().unwrap();
    let (session, first_run) = session_and_run(&db, "Paged", &["/root"]);
    let second_run = db
        .create_scan_run(session, &parameters(&["/root"], &[]), "test")
        .unwrap();
    db.start_scan_run(second_run).unwrap();

    for (run_id, prefix) in [(first_run, "first"), (second_run, "second")] {
        let mut files = [
            file(run_id, &format!("/root/{prefix}-alpha.txt"), 100, 11),
            file(run_id, &format!("/root/{prefix}-alpha-copy.txt"), 100, 11),
            file(run_id, &format!("/root/{prefix}-beta.bin"), 200, 22),
            file(run_id, &format!("/root/{prefix}-beta-copy.bin"), 200, 22),
            file(run_id, &format!("/root/{prefix}-beta-third.bin"), 200, 22),
            file(run_id, &format!("/root/{prefix}-gamma.bin"), 200, 33),
            file(run_id, &format!("/root/{prefix}-gamma-copy.bin"), 200, 33),
        ];
        files[1].root_path = "/selected-root".to_owned();
        files[1].relative_path = format!("{prefix}-alpha-copy.txt");
        files[1].drive_letter = "D:".to_owned();
        files[2].drive_letter = "D:".to_owned();
        files[3].drive_letter = "E:".to_owned();
        files[4].drive_letter = "D:".to_owned();
        files[5].drive_letter = "d:".to_owned();
        files[6].drive_letter = "D:".to_owned();
        db.insert_scanned_files(&files).unwrap();
        db.insert_duplicate_groups(
            run_id,
            &[
                (
                    11,
                    100,
                    vec![
                        format!("/root/{prefix}-alpha.txt"),
                        format!("/root/{prefix}-alpha-copy.txt"),
                    ],
                ),
                (
                    22,
                    200,
                    vec![
                        format!("/root/{prefix}-beta.bin"),
                        format!("/root/{prefix}-beta-copy.bin"),
                        format!("/root/{prefix}-beta-third.bin"),
                    ],
                ),
                (
                    33,
                    200,
                    vec![
                        format!("/root/{prefix}-gamma.bin"),
                        format!("/root/{prefix}-gamma-copy.bin"),
                    ],
                ),
            ],
        )
        .unwrap();
    }

    let base_query = DuplicateFileGroupPageQuery {
        run_id: first_run,
        limit: 2,
        sort_field: DuplicateFileGroupSortField::RecoverableBytes,
        sort_direction: SortDirection::Descending,
        filter: DuplicateFileGroupFilter {
            search: None,
            path_match: DuplicateFilePathMatchMode::Substring,
            extension_key: None,
            extension_match: DuplicateFileExtensionMatchMode::AnyMember,
            minimum_size: 0,
            minimum_copy_count: 2,
            across_drives: false,
            selected_root: None,
            selected_drive: None,
        },
        cursor: None,
    };
    let first_page = db.page_duplicate_file_groups(&base_query).unwrap();
    assert_eq!(first_page.total, 3);
    assert_eq!(first_page.summary.matching_group_count, 3);
    assert_eq!(first_page.summary.matching_copy_count, 7);
    assert_eq!(first_page.summary.potential_recoverable_bytes, 700);
    assert_eq!(first_page.summary.largest_recoverable_bytes, 400);
    assert_eq!(first_page.summary.distinct_selected_root_count, 2);
    assert_eq!(first_page.summary.distinct_drive_count, 2);
    assert_eq!(first_page.summary.across_drive_group_count, 1);
    assert_eq!(first_page.groups.len(), 2);
    assert!(first_page.has_more);
    assert!(first_page
        .groups
        .iter()
        .all(|group| group.run_id == first_run));
    assert_eq!(first_page.groups[0].recoverable_bytes, 400);
    assert_eq!(first_page.groups[1].recoverable_bytes, 200);
    assert!(first_page.groups[0].id < first_page.groups[1].id);

    let boundary = first_page.groups.last().unwrap();
    let second_page = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            cursor: Some(PageCursor {
                value: PageCursorValue::Integer(boundary.recoverable_bytes),
                id: boundary.id,
                before: false,
            }),
            ..base_query.clone()
        })
        .unwrap();
    assert_eq!(second_page.groups.len(), 1);
    assert_eq!(second_page.groups[0].recoverable_bytes, 100);
    assert!(!second_page.has_more);

    let backward_boundary = second_page.groups.first().unwrap();
    let previous_page = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            cursor: Some(PageCursor {
                value: PageCursorValue::Integer(backward_boundary.recoverable_bytes),
                id: backward_boundary.id,
                before: true,
            }),
            ..base_query.clone()
        })
        .unwrap();
    assert_eq!(
        previous_page
            .groups
            .iter()
            .map(|group| group.id)
            .collect::<Vec<_>>(),
        first_page
            .groups
            .iter()
            .map(|group| group.id)
            .collect::<Vec<_>>()
    );

    let filtered = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            limit: 10,
            filter: DuplicateFileGroupFilter {
                search: Some("alpha".to_owned()),
                path_match: DuplicateFilePathMatchMode::Substring,
                extension_key: None,
                extension_match: DuplicateFileExtensionMatchMode::AnyMember,
                minimum_size: 100,
                minimum_copy_count: 2,
                across_drives: false,
                selected_root: None,
                selected_drive: None,
            },
            cursor: None,
            ..base_query
        })
        .unwrap();
    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.summary.matching_group_count, 1);
    assert_eq!(filtered.summary.matching_copy_count, 2);
    assert_eq!(filtered.summary.potential_recoverable_bytes, 100);
    assert_eq!(filtered.summary.largest_recoverable_bytes, 100);
    assert_eq!(filtered.summary.distinct_selected_root_count, 2);
    assert_eq!(filtered.summary.distinct_drive_count, 1);
    assert_eq!(filtered.summary.across_drive_group_count, 0);
    let group = &filtered.groups[0];
    assert_eq!(group.distinct_selected_root_count, 2);
    assert_eq!(group.distinct_drive_count, 1);

    let one_copy_size = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            limit: 10,
            filter: DuplicateFileGroupFilter {
                minimum_size: 200,
                ..base_query.filter.clone()
            },
            cursor: None,
            ..base_query.clone()
        })
        .unwrap();
    assert_eq!(one_copy_size.total, 2);
    assert_eq!(one_copy_size.summary.matching_group_count, 2);
    assert_eq!(one_copy_size.summary.matching_copy_count, 5);
    assert_eq!(one_copy_size.summary.potential_recoverable_bytes, 600);
    assert_eq!(one_copy_size.summary.largest_recoverable_bytes, 400);
    assert!(one_copy_size
        .groups
        .iter()
        .all(|group| group.file_size >= 200));

    let one_copy_size_root_facets = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id: first_run,
            limit: 10,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: DuplicateFileGroupFilter {
                minimum_size: 200,
                ..base_query.filter.clone()
            },
            cursor: None,
        })
        .unwrap();
    assert_eq!(one_copy_size_root_facets.total, 1);
    assert_eq!(one_copy_size_root_facets.facets[0].matching_group_count, 2);

    let one_copy_size_drive_facets = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id: first_run,
            limit: 10,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: DuplicateFileGroupFilter {
                minimum_size: 200,
                ..base_query.filter.clone()
            },
            cursor: None,
        })
        .unwrap();
    assert_eq!(one_copy_size_drive_facets.total, 2);
    assert_eq!(one_copy_size_drive_facets.facets[0].matching_group_count, 2);
    assert_eq!(one_copy_size_drive_facets.facets[1].matching_group_count, 1);

    let across_drives = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            limit: 10,
            filter: DuplicateFileGroupFilter {
                search: None,
                path_match: DuplicateFilePathMatchMode::Substring,
                extension_key: None,
                extension_match: DuplicateFileExtensionMatchMode::AnyMember,
                minimum_size: 0,
                minimum_copy_count: 2,
                across_drives: true,
                selected_root: None,
                selected_drive: None,
            },
            cursor: None,
            ..base_query.clone()
        })
        .unwrap();
    assert_eq!(across_drives.total, 1);
    assert_eq!(across_drives.summary.matching_group_count, 1);
    assert_eq!(across_drives.summary.matching_copy_count, 3);
    assert_eq!(across_drives.summary.potential_recoverable_bytes, 400);
    assert_eq!(across_drives.summary.distinct_selected_root_count, 1);
    assert_eq!(across_drives.summary.distinct_drive_count, 2);
    assert_eq!(across_drives.summary.across_drive_group_count, 1);
    assert_eq!(across_drives.groups[0].distinct_drive_count, 2);

    let three_or_more = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            limit: 10,
            filter: DuplicateFileGroupFilter {
                minimum_copy_count: 3,
                ..base_query.filter.clone()
            },
            cursor: None,
            ..base_query.clone()
        })
        .unwrap();
    assert_eq!(three_or_more.total, 1);
    assert_eq!(three_or_more.summary.matching_group_count, 1);
    assert_eq!(three_or_more.summary.matching_copy_count, 3);
    assert_eq!(three_or_more.summary.potential_recoverable_bytes, 400);
    assert_eq!(three_or_more.groups[0].file_count, 3);

    let three_or_more_root_facets = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id: first_run,
            limit: 10,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: DuplicateFileGroupFilter {
                minimum_copy_count: 3,
                ..base_query.filter.clone()
            },
            cursor: None,
        })
        .unwrap();
    assert_eq!(three_or_more_root_facets.total, 1);
    assert_eq!(three_or_more_root_facets.facets[0].matching_group_count, 1);

    let three_or_more_drive_facets = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id: first_run,
            limit: 10,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: DuplicateFileGroupFilter {
                minimum_copy_count: 3,
                ..base_query.filter.clone()
            },
            cursor: None,
        })
        .unwrap();
    assert_eq!(three_or_more_drive_facets.total, 2);
    assert!(three_or_more_drive_facets
        .facets
        .iter()
        .all(|facet| facet.matching_group_count == 1));

    let root_facets = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id: first_run,
            limit: 1,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: DuplicateFileGroupFilter {
                search: None,
                path_match: DuplicateFilePathMatchMode::Substring,
                extension_key: None,
                extension_match: DuplicateFileExtensionMatchMode::AnyMember,
                minimum_size: 0,
                minimum_copy_count: 2,
                across_drives: false,
                selected_root: Some("/selected-root".to_owned()),
                selected_drive: None,
            },
            cursor: None,
        })
        .unwrap();
    assert_eq!(root_facets.total, 2);
    assert_eq!(root_facets.facets.len(), 1);
    assert_eq!(root_facets.facets[0].value, "/root");
    assert_eq!(root_facets.facets[0].matching_group_count, 3);
    assert!(root_facets.has_more);

    let root_facet_boundary = &root_facets.facets[0];
    let next_root_facets = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id: first_run,
            limit: 1,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: DuplicateFileGroupFilter {
                search: None,
                path_match: DuplicateFilePathMatchMode::Substring,
                extension_key: None,
                extension_match: DuplicateFileExtensionMatchMode::AnyMember,
                minimum_size: 0,
                minimum_copy_count: 2,
                across_drives: false,
                selected_root: None,
                selected_drive: None,
            },
            cursor: Some(PageCursor {
                value: PageCursorValue::Integer(root_facet_boundary.matching_group_count),
                id: root_facet_boundary.cursor_id,
                before: false,
            }),
        })
        .unwrap();
    assert_eq!(next_root_facets.facets[0].value, "/selected-root");
    assert_eq!(next_root_facets.facets[0].matching_group_count, 1);

    let drive_facets = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id: first_run,
            limit: 1,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: DuplicateFileGroupFilter {
                search: None,
                path_match: DuplicateFilePathMatchMode::Substring,
                extension_key: None,
                extension_match: DuplicateFileExtensionMatchMode::AnyMember,
                minimum_size: 0,
                minimum_copy_count: 2,
                across_drives: false,
                selected_root: None,
                selected_drive: Some("E:".to_owned()),
            },
            cursor: None,
        })
        .unwrap();
    assert_eq!(drive_facets.total, 2);
    assert_eq!(drive_facets.facets.len(), 1);
    assert_eq!(drive_facets.facets[0].value, "D:");
    assert_eq!(drive_facets.facets[0].matching_group_count, 3);
    assert!(drive_facets.has_more);

    let drive_facet_boundary = &drive_facets.facets[0];
    let next_drive_facets = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id: first_run,
            limit: 1,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: DuplicateFileGroupFilter {
                search: None,
                path_match: DuplicateFilePathMatchMode::Substring,
                extension_key: None,
                extension_match: DuplicateFileExtensionMatchMode::AnyMember,
                minimum_size: 0,
                minimum_copy_count: 2,
                across_drives: false,
                selected_root: None,
                selected_drive: None,
            },
            cursor: Some(PageCursor {
                value: PageCursorValue::Integer(drive_facet_boundary.matching_group_count),
                id: drive_facet_boundary.cursor_id,
                before: false,
            }),
        })
        .unwrap();
    assert_eq!(next_drive_facets.facets[0].value, "E:");
    assert_eq!(next_drive_facets.facets[0].matching_group_count, 1);

    let root_scoped_drive_facets = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id: first_run,
            limit: 10,
            sort_field: DuplicateFileDriveFacetSortField::Value,
            sort_direction: SortDirection::Ascending,
            filter: DuplicateFileGroupFilter {
                search: None,
                path_match: DuplicateFilePathMatchMode::Substring,
                extension_key: None,
                extension_match: DuplicateFileExtensionMatchMode::AnyMember,
                minimum_size: 0,
                minimum_copy_count: 2,
                across_drives: false,
                selected_root: Some("/SELECTED-ROOT".to_owned()),
                selected_drive: None,
            },
            cursor: None,
        })
        .unwrap();
    assert_eq!(root_scoped_drive_facets.total, 1);
    assert_eq!(root_scoped_drive_facets.facets[0].value, "D:");
    assert_eq!(root_scoped_drive_facets.facets[0].matching_group_count, 1);

    let selected_root_groups = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            run_id: first_run,
            limit: 10,
            sort_field: DuplicateFileGroupSortField::RecoverableBytes,
            sort_direction: SortDirection::Descending,
            filter: DuplicateFileGroupFilter {
                search: None,
                path_match: DuplicateFilePathMatchMode::Substring,
                extension_key: None,
                extension_match: DuplicateFileExtensionMatchMode::AnyMember,
                minimum_size: 0,
                minimum_copy_count: 2,
                across_drives: false,
                selected_root: Some("/SELECTED-ROOT".to_owned()),
                selected_drive: None,
            },
            cursor: None,
        })
        .unwrap();
    assert_eq!(selected_root_groups.total, 1);
    assert_eq!(selected_root_groups.summary.matching_group_count, 1);
    assert_eq!(selected_root_groups.summary.matching_copy_count, 2);
    assert!(selected_root_groups.groups[0]
        .representative_name
        .contains("alpha"));
    let selected_drive_groups = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            run_id: first_run,
            limit: 10,
            sort_field: DuplicateFileGroupSortField::RecoverableBytes,
            sort_direction: SortDirection::Descending,
            filter: DuplicateFileGroupFilter {
                search: None,
                path_match: DuplicateFilePathMatchMode::Substring,
                extension_key: None,
                extension_match: DuplicateFileExtensionMatchMode::AnyMember,
                minimum_size: 0,
                minimum_copy_count: 2,
                across_drives: false,
                selected_root: None,
                selected_drive: Some("e:".to_owned()),
            },
            cursor: None,
        })
        .unwrap();
    assert_eq!(selected_drive_groups.total, 1);
    assert_eq!(selected_drive_groups.summary.matching_group_count, 1);
    assert_eq!(selected_drive_groups.summary.distinct_drive_count, 2);
    let members = db
        .page_duplicate_file_members(&DuplicateFileMemberPageQuery {
            run_id: first_run,
            group_id: group.id,
            limit: 1,
            sort_field: DuplicateFileMemberSortField::Path,
            sort_direction: SortDirection::Ascending,
            filter: DuplicateFileMemberFilter {
                search: Some("copy".to_owned()),
            },
            cursor: None,
        })
        .unwrap();
    assert_eq!(members.total, 1);
    assert!(members.members[0]
        .canonical_path
        .contains("first-alpha-copy"));
    assert_eq!(members.members[0].root_path, "/selected-root");
    assert_eq!(members.members[0].relative_path, "first-alpha-copy.txt");
    assert_eq!(members.members[0].drive_letter, "D:");
    assert!(!db
        .duplicate_file_group_exists(second_run, group.id)
        .unwrap());
}

#[test]
fn exact_member_path_filter_is_unicode_case_normalized_and_shared_by_facets() {
    let db = Database::open_in_memory().unwrap();
    let (_, run_id) = session_and_run(&db, "Exact path", &["/root"]);
    let mut files = [
        file(run_id, "/root/Überraschung.TXT", 100, 11),
        file(run_id, "/root/copy-one.bin", 100, 11),
        file(run_id, "/root/archive/Überraschung.TXT", 200, 22),
        file(run_id, "/root/copy-two.bin", 200, 22),
    ];
    for file in &mut files {
        file.drive_letter = "D:".to_owned();
    }
    db.insert_scanned_files(&files).unwrap();
    db.insert_duplicate_groups(
        run_id,
        &[
            (
                11,
                100,
                vec![
                    "/root/Überraschung.TXT".to_owned(),
                    "/root/copy-one.bin".to_owned(),
                ],
            ),
            (
                22,
                200,
                vec![
                    "/root/archive/Überraschung.TXT".to_owned(),
                    "/root/copy-two.bin".to_owned(),
                ],
            ),
        ],
    )
    .unwrap();

    let filter = DuplicateFileGroupFilter {
        search: Some("/ROOT/überraschung.txt".to_owned()),
        path_match: DuplicateFilePathMatchMode::Exact,
        extension_key: None,
        extension_match: DuplicateFileExtensionMatchMode::AnyMember,
        minimum_size: 0,
        minimum_copy_count: 2,
        across_drives: false,
        selected_root: None,
        selected_drive: None,
    };
    let groups = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            run_id,
            limit: 10,
            sort_field: DuplicateFileGroupSortField::RecoverableBytes,
            sort_direction: SortDirection::Descending,
            filter: filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(groups.total, 1);
    assert_eq!(groups.summary.matching_group_count, 1);
    assert_eq!(groups.summary.matching_copy_count, 2);
    assert_eq!(groups.summary.potential_recoverable_bytes, 100);
    assert_eq!(groups.groups[0].file_size, 100);

    let root_facets = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(root_facets.total, 1);
    assert_eq!(root_facets.facets[0].matching_group_count, 1);

    let drive_facets = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter,
            cursor: None,
        })
        .unwrap();
    assert_eq!(drive_facets.total, 1);
    assert_eq!(drive_facets.facets[0].matching_group_count, 1);

    let exact_path_index: i64 = db
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'idx_file_run_path_unicode_nocase'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(exact_path_index, 1);
}

#[test]
fn extension_match_modes_use_persisted_filename_keys_and_shared_facets() {
    let db = Database::open_in_memory().unwrap();
    let (_, run_id) = session_and_run(&db, "Extension", &["/root"]);
    let mut files = [
        file(run_id, "/root/representative.bin", 100, 11),
        file(run_id, "/root/photo.JpG", 100, 11),
        file(run_id, "/root/.env", 200, 22),
        file(run_id, "/root/trailing.", 200, 22),
        file(run_id, "/root/config.env.local", 300, 33),
        file(run_id, "/root/copy.bin", 300, 33),
        file(run_id, "/root/unicode.CAFÉ", 400, 44),
        file(run_id, "/root/unicode-copy.bin", 400, 44),
        file(run_id, "/root/all-one.JPG", 500, 55),
        file(run_id, "/root/all-two.jpg", 500, 55),
    ];
    for file in &mut files {
        file.drive_letter = "D:".to_owned();
    }
    db.insert_scanned_files(&files).unwrap();
    db.insert_duplicate_groups(
        run_id,
        &[
            (
                11,
                100,
                vec![
                    "/root/representative.bin".to_owned(),
                    "/root/photo.JpG".to_owned(),
                ],
            ),
            (
                22,
                200,
                vec!["/root/.env".to_owned(), "/root/trailing.".to_owned()],
            ),
            (
                33,
                300,
                vec![
                    "/root/config.env.local".to_owned(),
                    "/root/copy.bin".to_owned(),
                ],
            ),
            (
                44,
                400,
                vec![
                    "/root/unicode.CAFÉ".to_owned(),
                    "/root/unicode-copy.bin".to_owned(),
                ],
            ),
            (
                55,
                500,
                vec![
                    "/root/all-one.JPG".to_owned(),
                    "/root/all-two.jpg".to_owned(),
                ],
            ),
        ],
    )
    .unwrap();

    let base_filter = DuplicateFileGroupFilter {
        search: None,
        path_match: DuplicateFilePathMatchMode::Substring,
        extension_key: Some("jpg".to_owned()),
        extension_match: DuplicateFileExtensionMatchMode::AnyMember,
        minimum_size: 0,
        minimum_copy_count: 2,
        across_drives: false,
        selected_root: None,
        selected_drive: None,
    };
    let groups = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            run_id,
            limit: 10,
            sort_field: DuplicateFileGroupSortField::RecoverableBytes,
            sort_direction: SortDirection::Descending,
            filter: base_filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(groups.total, 2);
    assert_eq!(groups.summary.matching_group_count, 2);
    assert_eq!(groups.summary.matching_copy_count, 4);
    assert_eq!(groups.summary.potential_recoverable_bytes, 600);
    assert!(groups.groups.iter().any(|group| group.file_size == 100));
    assert!(groups.groups.iter().any(|group| group.file_size == 500));

    let root_facets = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: base_filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(root_facets.total, 1);
    assert_eq!(root_facets.facets[0].matching_group_count, 2);
    let drive_facets = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: base_filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(drive_facets.total, 1);
    assert_eq!(drive_facets.facets[0].matching_group_count, 2);

    let all_members_filter = DuplicateFileGroupFilter {
        extension_match: DuplicateFileExtensionMatchMode::AllMembers,
        ..base_filter.clone()
    };
    let all_member_groups = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            run_id,
            limit: 10,
            sort_field: DuplicateFileGroupSortField::RecoverableBytes,
            sort_direction: SortDirection::Descending,
            filter: all_members_filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(all_member_groups.total, 1);
    assert_eq!(all_member_groups.summary.matching_group_count, 1);
    assert_eq!(all_member_groups.summary.matching_copy_count, 2);
    assert_eq!(all_member_groups.summary.potential_recoverable_bytes, 500);
    assert_eq!(all_member_groups.groups[0].file_size, 500);
    let all_member_root_facets = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: all_members_filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(all_member_root_facets.facets[0].matching_group_count, 1);
    let all_member_drive_facets = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: all_members_filter,
            cursor: None,
        })
        .unwrap();
    assert_eq!(all_member_drive_facets.facets[0].matching_group_count, 1);

    let no_extension = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: DuplicateFileGroupFilter {
                extension_key: Some(String::new()),
                ..base_filter.clone()
            },
            run_id,
            limit: 10,
            sort_field: DuplicateFileGroupSortField::RecoverableBytes,
            sort_direction: SortDirection::Descending,
            cursor: None,
        })
        .unwrap();
    assert_eq!(no_extension.total, 1);
    assert_eq!(no_extension.groups[0].file_size, 200);
    let all_members_without_extension = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: DuplicateFileGroupFilter {
                extension_key: Some(String::new()),
                extension_match: DuplicateFileExtensionMatchMode::AllMembers,
                ..base_filter.clone()
            },
            run_id,
            limit: 10,
            sort_field: DuplicateFileGroupSortField::RecoverableBytes,
            sort_direction: SortDirection::Descending,
            cursor: None,
        })
        .unwrap();
    assert_eq!(all_members_without_extension.total, 1);
    assert_eq!(all_members_without_extension.groups[0].file_size, 200);

    let multiple_suffix = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: DuplicateFileGroupFilter {
                extension_key: Some("local".to_owned()),
                ..base_filter.clone()
            },
            run_id,
            limit: 10,
            sort_field: DuplicateFileGroupSortField::RecoverableBytes,
            sort_direction: SortDirection::Descending,
            cursor: None,
        })
        .unwrap();
    assert_eq!(multiple_suffix.total, 1);
    assert_eq!(multiple_suffix.groups[0].file_size, 300);

    let unicode_case = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: DuplicateFileGroupFilter {
                extension_key: Some("café".to_owned()),
                ..base_filter.clone()
            },
            run_id,
            limit: 10,
            sort_field: DuplicateFileGroupSortField::RecoverableBytes,
            sort_direction: SortDirection::Descending,
            cursor: None,
        })
        .unwrap();
    assert_eq!(unicode_case.total, 1);
    assert_eq!(unicode_case.groups[0].file_size, 400);
    let different_unicode_form = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: DuplicateFileGroupFilter {
                extension_key: Some("cafe\u{301}".to_owned()),
                ..base_filter
            },
            run_id,
            limit: 10,
            sort_field: DuplicateFileGroupSortField::RecoverableBytes,
            sort_direction: SortDirection::Descending,
            cursor: None,
        })
        .unwrap();
    assert_eq!(different_unicode_form.total, 0);

    let extension_index: i64 = db
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'idx_file_run_extension_key'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(extension_index, 1);
}

#[test]
fn existing_schema_four_extension_keys_are_backfilled_without_filesystem_access() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("extension-backfill.db");
    let db = Database::open(path.to_str().unwrap()).unwrap();
    let (_, run_id) = session_and_run(&db, "Backfill", &["/root"]);
    db.insert_scanned_files(&[
        file(run_id, "/root/archive.tar.GZ", 100, 1),
        file(run_id, "/root/.env", 100, 1),
    ])
    .unwrap();
    db.connection()
        .execute("UPDATE scanned_file SET extension_key = NULL", [])
        .unwrap();
    drop(db);

    let reopened = Database::open(path.to_str().unwrap()).unwrap();
    let keys = reopened
        .connection()
        .prepare("SELECT extension_key FROM scanned_file ORDER BY canonical_path")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(keys, vec![String::new(), "gz".to_owned()]);
}

#[test]
fn version_four_migrates_review_tables_transactionally() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("review-v4.db");
    let db = Database::open(path.to_str().unwrap()).unwrap();
    let (_, run_id) = session_and_run(&db, "Preserved v4 run", &["/root"]);
    db.connection()
        .execute_batch(
            "DROP TABLE preference_rule_command;
             DROP TABLE preference_rule_root;
             DROP TABLE preference_rule;
             DROP INDEX idx_review_folder_command_plan_operation;
             DROP INDEX idx_review_folder_decision_plan_decision;
             DROP INDEX idx_review_folder_decision_plan_group;
             DROP TABLE review_folder_command;
             DROP TABLE review_folder_decision;
             DROP INDEX idx_review_command_plan_operation;
             DROP INDEX idx_review_decision_plan_decision;
             DROP INDEX idx_review_decision_plan_group;
             DROP INDEX idx_review_plan_one_active_run;
             DROP TABLE review_command;
             DROP TABLE review_decision;
             DROP TABLE review_plan;
             PRAGMA user_version = 4;",
        )
        .unwrap();
    drop(db);

    let migrated = Database::open(path.to_str().unwrap()).unwrap();
    assert_eq!(migrated.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    assert_eq!(migrated.get_scan_run(run_id).unwrap().status, "interrupted");
    for table in [
        "review_plan",
        "review_decision",
        "review_command",
        "review_folder_decision",
        "review_folder_command",
    ] {
        let exists: bool = migrated
            .connection()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                params![table],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "missing migrated table {table}");
    }
}

#[test]
fn version_five_migrates_folder_review_tables_transactionally() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("review-v5.db");
    let db = Database::open(path.to_str().unwrap()).unwrap();
    let (run_id, _, _) = completed_review_fixture(&db);
    db.connection()
        .execute_batch(
            "DROP TABLE preference_rule_command;
             DROP TABLE preference_rule_root;
             DROP TABLE preference_rule;
             DROP INDEX idx_review_folder_command_plan_operation;
             DROP INDEX idx_review_folder_decision_plan_decision;
             DROP INDEX idx_review_folder_decision_plan_group;
             DROP TABLE review_folder_command;
             DROP TABLE review_folder_decision;
             PRAGMA user_version = 5;",
        )
        .unwrap();
    drop(db);

    let migrated = Database::open(path.to_str().unwrap()).unwrap();
    assert_eq!(migrated.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    assert_eq!(migrated.get_scan_run(run_id).unwrap().status, "completed");
    for table in ["review_folder_decision", "review_folder_command"] {
        let exists: bool = migrated
            .connection()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                params![table],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "missing migrated table {table}");
    }
}

fn completed_review_fixture(db: &Database) -> (i64, i64, [i64; 3]) {
    let (_, run_id) = session_and_run(db, "Review", &["/root"]);
    let mut first = file(run_id, "/root/first.bin", 100, 77);
    first.file_identity = Some("volume-1:file-1".to_owned());
    let mut alias = file(run_id, "/root/first-alias.bin", 100, 77);
    alias.file_identity = Some("volume-1:file-1".to_owned());
    let mut second = file(run_id, "/root/second.bin", 100, 77);
    second.file_identity = Some("volume-1:file-2".to_owned());
    db.insert_scanned_files(&[first, alias, second]).unwrap();
    db.insert_duplicate_groups(
        run_id,
        &[(
            77,
            100,
            vec![
                "/root/first.bin".to_owned(),
                "/root/first-alias.bin".to_owned(),
                "/root/second.bin".to_owned(),
            ],
        )],
    )
    .unwrap();
    db.complete_scan_run(run_id, 3, 300, 3, 1, 0, 200, 0)
        .unwrap();
    let group_id = db
        .connection()
        .query_row(
            "SELECT id FROM duplicate_group WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )
        .unwrap();
    let mut statement = db
        .connection()
        .prepare("SELECT id FROM scanned_file WHERE run_id = ?1 ORDER BY canonical_path")
        .unwrap();
    let file_ids = statement
        .query_map(params![run_id], |row| row.get::<_, i64>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    (run_id, group_id, [file_ids[1], file_ids[0], file_ids[2]])
}

#[test]
fn manual_review_decisions_are_idempotent_persistent_and_preserve_a_physical_survivor() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("review-decisions.db");
    let db = Database::open(path.to_str().unwrap()).unwrap();
    let (run_id, group_id, [first, alias, second]) = completed_review_fixture(&db);

    let initial = db.get_review_plan_view(run_id).unwrap();
    assert!(initial.plan.is_none());
    assert_eq!(initial.summary.undecided_count, 3);
    assert_eq!(initial.summary.remaining_physical_copy_count, 2);

    let remove_first = db
        .set_review_decision(
            "remove-first",
            run_id,
            group_id,
            first,
            ReviewDecisionKind::Remove,
            0,
        )
        .unwrap();
    assert_eq!(remove_first.applied_revision, 1);
    assert!(!remove_first.replayed);
    let remove_alias = db
        .set_review_decision(
            "remove-alias",
            run_id,
            group_id,
            alias,
            ReviewDecisionKind::Remove,
            1,
        )
        .unwrap();
    assert_eq!(remove_alias.applied_revision, 2);

    let unsafe_error = db
        .set_review_decision(
            "remove-last-physical-copy",
            run_id,
            group_id,
            second,
            ReviewDecisionKind::Remove,
            2,
        )
        .unwrap_err();
    assert!(matches!(unsafe_error, ReviewError::UnsafeRemoval { .. }));

    let keep_second = db
        .set_review_decision(
            "keep-second",
            run_id,
            group_id,
            second,
            ReviewDecisionKind::Keep,
            2,
        )
        .unwrap();
    assert_eq!(keep_second.applied_revision, 3);
    let replay = db
        .set_review_decision(
            "keep-second",
            run_id,
            group_id,
            second,
            ReviewDecisionKind::Keep,
            2,
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.applied_revision, 3);
    let conflict = db
        .set_review_decision(
            "keep-second",
            run_id,
            group_id,
            first,
            ReviewDecisionKind::Keep,
            2,
        )
        .unwrap_err();
    assert!(matches!(conflict, ReviewError::IdempotencyConflict { .. }));
    let stale = db
        .set_review_decision(
            "stale",
            run_id,
            group_id,
            first,
            ReviewDecisionKind::Undecided,
            1,
        )
        .unwrap_err();
    assert!(matches!(
        stale,
        ReviewError::StaleRevision {
            expected: 1,
            actual: 3
        }
    ));
    db.set_review_decision(
        "clear-first",
        run_id,
        group_id,
        first,
        ReviewDecisionKind::Undecided,
        3,
    )
    .unwrap();

    let snapshot: (String, Option<String>, i64, i64, Option<i64>, String) = db
        .connection()
        .query_row(
            "SELECT snapshot_canonical_path, snapshot_file_identity, snapshot_file_size,
                    snapshot_last_modified, snapshot_content_hash, provenance
             FROM review_decision WHERE file_id = ?1",
            params![second],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(snapshot.0, "/root/second.bin");
    assert_eq!(snapshot.1.as_deref(), Some("volume-1:file-2"));
    assert_eq!(snapshot.2, 100);
    assert_eq!(snapshot.3, 1_700_000_000);
    assert_eq!(snapshot.4, Some(77));
    assert_eq!(snapshot.5, "manual");
    drop(db);

    let reopened = Database::open(path.to_str().unwrap()).unwrap();
    let persisted = reopened.get_review_plan_view(run_id).unwrap();
    assert_eq!(persisted.plan.unwrap().revision, 4);
    assert_eq!(persisted.summary.keep_count, 1);
    assert_eq!(persisted.summary.remove_count, 1);
    assert_eq!(persisted.summary.undecided_count, 1);
    assert_eq!(persisted.summary.planned_removal_bytes, 100);
    assert_eq!(persisted.summary.remaining_physical_copy_count, 2);
    let group = reopened.get_review_group_view(run_id, group_id).unwrap().1;
    assert_eq!(group.keep_count, 1);
    assert_eq!(group.remove_count, 1);
    assert_eq!(group.undecided_count, 1);
    assert_eq!(group.remaining_physical_copy_count, 2);
}

fn completed_folder_review_fixture(db: &Database) -> (i64, i64, [i64; 2], [i64; 2]) {
    let (_, run_id) = session_and_run(db, "Folder review", &["/root"]);
    let first_directory = db
        .insert_directory_node(run_id, "/root/Copy A", "Copy A", None, 100, 1, 1)
        .unwrap();
    let second_directory = db
        .insert_directory_node(run_id, "/root/Copy B", "Copy B", None, 100, 1, 1)
        .unwrap();
    let mut first = file(run_id, "/root/Copy A/item.bin", 100, 91);
    first.file_identity = Some("volume-1:shared-hard-link".to_owned());
    let mut second = file(run_id, "/root/Copy B/item.bin", 100, 91);
    second.file_identity = Some("volume-1:shared-hard-link".to_owned());
    db.insert_scanned_files(&[first, second]).unwrap();
    db.insert_duplicate_groups(
        run_id,
        &[(
            91,
            100,
            vec![
                "/root/Copy A/item.bin".to_owned(),
                "/root/Copy B/item.bin".to_owned(),
            ],
        )],
    )
    .unwrap();
    db.replace_exact_folder_groups(
        run_id,
        &[ExactFolderGroupInsert {
            structural_fingerprint: "structure".to_owned(),
            verified_fingerprint: "verified".to_owned(),
            total_size: 100,
            file_count: 1,
            directory_ids: vec![first_directory, second_directory],
            is_suppressed: false,
        }],
        &AtomicBool::new(false),
    )
    .unwrap();
    db.complete_scan_run(run_id, 2, 200, 2, 1, 1, 100, 0)
        .unwrap();
    let folder_group_id = db
        .connection()
        .query_row(
            "SELECT id FROM duplicate_folder_group WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )
        .unwrap();
    let mut member_statement = db
        .connection()
        .prepare(
            "SELECT member.id FROM duplicate_folder_group_member member
             JOIN directory_node directory ON directory.id = member.directory_id
             WHERE member.group_id = ?1 ORDER BY directory.path",
        )
        .unwrap();
    let folder_members = member_statement
        .query_map(params![folder_group_id], |row| row.get::<_, i64>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut file_statement = db
        .connection()
        .prepare("SELECT id FROM scanned_file WHERE run_id = ?1 ORDER BY canonical_path")
        .unwrap();
    let files = file_statement
        .query_map(params![run_id], |row| row.get::<_, i64>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    (
        run_id,
        folder_group_id,
        [folder_members[0], folder_members[1]],
        [files[0], files[1]],
    )
}

#[test]
fn manual_folder_decisions_share_revision_persist_snapshots_and_reject_file_overlap() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("folder-review.db");
    let db = Database::open(path.to_str().unwrap()).unwrap();
    let (run_id, folder_group_id, [first_copy, second_copy], [first_file, _]) =
        completed_folder_review_fixture(&db);

    let initial = db.get_review_plan_view(run_id).unwrap();
    assert_eq!(initial.summary.folder_undecided_count, 2);
    assert_eq!(initial.summary.intact_folder_copy_count, 2);
    let removed = db
        .set_review_folder_decision(
            "remove-folder-a",
            run_id,
            folder_group_id,
            first_copy,
            ReviewDecisionKind::Remove,
            0,
        )
        .unwrap();
    assert_eq!(removed.applied_revision, 1);
    assert!(!removed.replayed);
    let replay = db
        .set_review_folder_decision(
            "remove-folder-a",
            run_id,
            folder_group_id,
            first_copy,
            ReviewDecisionKind::Remove,
            0,
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.applied_revision, 1);
    let conflict = db
        .set_review_folder_decision(
            "remove-folder-a",
            run_id,
            folder_group_id,
            second_copy,
            ReviewDecisionKind::Keep,
            0,
        )
        .unwrap_err();
    assert!(matches!(conflict, ReviewError::IdempotencyConflict { .. }));
    let file_overlap = db
        .set_review_decision(
            "keep-contained-file",
            run_id,
            db.connection()
                .query_row(
                    "SELECT group_id FROM duplicate_group_member WHERE file_id = ?1",
                    params![first_file],
                    |row| row.get(0),
                )
                .unwrap(),
            first_file,
            ReviewDecisionKind::Keep,
            1,
        )
        .unwrap_err();
    assert!(matches!(file_overlap, ReviewError::Overlap { .. }));

    let view = db.get_review_plan_view(run_id).unwrap();
    assert_eq!(view.plan.as_ref().unwrap().revision, 1);
    assert_eq!(view.summary.folder_remove_count, 1);
    assert_eq!(view.summary.folder_undecided_count, 1);
    assert_eq!(view.summary.effective_removal_file_count, 1);
    assert_eq!(view.summary.planned_removal_physical_item_count, 1);
    assert_eq!(view.summary.planned_removal_bytes, 100);
    assert_eq!(view.summary.remaining_physical_copy_count, 1);
    assert_eq!(view.summary.intact_folder_copy_count, 1);
    let folder_summary = db
        .get_review_folder_group_view(run_id, folder_group_id)
        .unwrap()
        .1;
    assert_eq!(folder_summary.remove_count, 1);
    assert_eq!(folder_summary.undecided_count, 1);
    assert_eq!(folder_summary.intact_copy_count, 1);
    let snapshot: (String, i64, i64, String, String, String) = db
        .connection()
        .query_row(
            "SELECT snapshot_path, snapshot_total_size, snapshot_file_count,
                    snapshot_structural_fingerprint, snapshot_verified_fingerprint, provenance
             FROM review_folder_decision WHERE folder_member_id = ?1",
            params![first_copy],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(snapshot.0, "/root/Copy A");
    assert_eq!(snapshot.1, 100);
    assert_eq!(snapshot.2, 1);
    assert_eq!(snapshot.3, "structure");
    assert_eq!(snapshot.4, "verified");
    assert_eq!(snapshot.5, "manual");
    drop(db);

    let reopened = Database::open(path.to_str().unwrap()).unwrap();
    let persisted = reopened.get_review_plan_view(run_id).unwrap();
    assert_eq!(persisted.plan.unwrap().revision, 1);
    assert_eq!(persisted.summary.folder_remove_count, 1);
    assert_eq!(persisted.summary.effective_removal_file_count, 1);
}

#[test]
fn folder_review_protects_nested_and_suppressed_sets_and_rejects_nested_removal_overlap() {
    let db = Database::open_in_memory().unwrap();
    let (_, run_id) = session_and_run(&db, "Nested folder review", &["/root"]);
    let root_a = db
        .insert_directory_node(run_id, "/root/A", "A", None, 10, 1, 1)
        .unwrap();
    let nested_a = db
        .insert_directory_node(run_id, "/root/A/Nested", "Nested", Some(root_a), 10, 1, 2)
        .unwrap();
    let root_b = db
        .insert_directory_node(run_id, "/root/B", "B", None, 10, 1, 1)
        .unwrap();
    let nested_b = db
        .insert_directory_node(run_id, "/root/B/Nested", "Nested", Some(root_b), 10, 1, 2)
        .unwrap();
    let root_c = db
        .insert_directory_node(run_id, "/root/C", "C", None, 10, 1, 1)
        .unwrap();
    let nested_c = db
        .insert_directory_node(run_id, "/root/C/Nested", "Nested", Some(root_c), 10, 1, 2)
        .unwrap();
    let root_d = db
        .insert_directory_node(run_id, "/root/D", "D", None, 10, 1, 1)
        .unwrap();
    let nested_d = db
        .insert_directory_node(run_id, "/root/D/Nested", "Nested", Some(root_d), 10, 1, 2)
        .unwrap();
    db.insert_scanned_files(&[
        file(run_id, "/root/A/Nested/item.bin", 10, 1),
        file(run_id, "/root/B/Nested/item.bin", 10, 1),
        file(run_id, "/root/C/Nested/item.bin", 10, 1),
        file(run_id, "/root/D/Nested/item.bin", 10, 1),
    ])
    .unwrap();
    db.replace_exact_folder_groups(
        run_id,
        &[
            ExactFolderGroupInsert {
                structural_fingerprint: "outer".to_owned(),
                verified_fingerprint: "outer-v".to_owned(),
                total_size: 10,
                file_count: 1,
                directory_ids: vec![root_a, root_b],
                is_suppressed: false,
            },
            ExactFolderGroupInsert {
                structural_fingerprint: "inner-visible".to_owned(),
                verified_fingerprint: "inner-visible-v".to_owned(),
                total_size: 10,
                file_count: 1,
                directory_ids: vec![nested_a, nested_c],
                is_suppressed: false,
            },
            ExactFolderGroupInsert {
                structural_fingerprint: "inner-suppressed".to_owned(),
                verified_fingerprint: "inner-suppressed-v".to_owned(),
                total_size: 10,
                file_count: 1,
                directory_ids: vec![nested_b, nested_d],
                is_suppressed: true,
            },
        ],
        &AtomicBool::new(false),
    )
    .unwrap();
    db.complete_scan_run(run_id, 4, 40, 4, 0, 2, 0, 0).unwrap();
    let find_group = |fingerprint: &str| {
        db.connection()
            .query_row(
                "SELECT id FROM duplicate_folder_group
                 WHERE run_id = ?1 AND structural_fingerprint = ?2",
                params![run_id, fingerprint],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
    };
    let find_member = |group_id: i64, directory_id: i64| {
        db.connection()
            .query_row(
                "SELECT id FROM duplicate_folder_group_member
                 WHERE group_id = ?1 AND directory_id = ?2",
                params![group_id, directory_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
    };
    let outer_group = find_group("outer");
    let inner_group = find_group("inner-visible");
    let suppressed_group = find_group("inner-suppressed");
    let outer_a_member = find_member(outer_group, root_a);
    let outer_b_member = find_member(outer_group, root_b);
    let nested_a_member = find_member(inner_group, nested_a);
    let suppressed_member = find_member(suppressed_group, nested_b);

    db.set_review_folder_decision(
        "remove-outer-a",
        run_id,
        outer_group,
        outer_a_member,
        ReviewDecisionKind::Remove,
        0,
    )
    .unwrap();
    let nested_overlap = db
        .set_review_folder_decision(
            "remove-covered-nested-a",
            run_id,
            inner_group,
            nested_a_member,
            ReviewDecisionKind::Remove,
            1,
        )
        .unwrap_err();
    assert!(matches!(nested_overlap, ReviewError::Overlap { .. }));
    let suppressed = db
        .set_review_folder_decision(
            "suppressed-not-addressable",
            run_id,
            suppressed_group,
            suppressed_member,
            ReviewDecisionKind::Keep,
            1,
        )
        .unwrap_err();
    assert!(matches!(
        suppressed,
        ReviewError::FolderGroupNotFound { .. }
    ));
    let unsafe_outer = db
        .set_review_folder_decision(
            "remove-outer-b",
            run_id,
            outer_group,
            outer_b_member,
            ReviewDecisionKind::Remove,
            1,
        )
        .unwrap_err();
    assert!(matches!(
        unsafe_outer,
        ReviewError::UnsafeFolderRemoval { .. }
    ));
    let inner_summary = db
        .get_review_folder_group_view(run_id, inner_group)
        .unwrap()
        .1;
    assert_eq!(inner_summary.intact_copy_count, 1);
    assert_eq!(
        db.get_review_plan_view(run_id)
            .unwrap()
            .plan
            .unwrap()
            .revision,
        1
    );
}

#[test]
fn hundred_thousand_folder_review_groups_keep_summary_and_keyset_pages_bounded() {
    let _large_fixture_guard = LARGE_FIXTURE_TEST_LOCK.lock().unwrap();
    let db = Database::open_in_memory().unwrap();
    let (_, run_id) = session_and_run(&db, "Folder review scale", &["/root"]);
    let transaction = db.connection().unchecked_transaction().unwrap();
    let numbers = "WITH digits(d) AS (VALUES(0),(1),(2),(3),(4),(5),(6),(7),(8),(9)),
                   numbers(n) AS (
                       SELECT a.d * 10000 + b.d * 1000 + c.d * 100 + d.d * 10 + e.d
                       FROM digits a CROSS JOIN digits b CROSS JOIN digits c
                       CROSS JOIN digits d CROSS JOIN digits e
                   )";
    transaction
        .execute(
            &format!(
                "{numbers}
                 INSERT INTO directory_node
                    (id, run_id, path, name, parent_id, total_size, file_count, depth)
                 SELECT n * 2 + CASE suffix WHEN 'a' THEN 1 ELSE 2 END,
                        ?1, printf('/root/%06d-%s', n, suffix), suffix, NULL, 1, 1, 1
                 FROM numbers CROSS JOIN (SELECT 'a' AS suffix UNION ALL SELECT 'b')"
            ),
            params![run_id],
        )
        .unwrap();
    transaction
        .execute(
            &format!(
                "{numbers}
                 INSERT INTO duplicate_folder_group
                    (id, run_id, structural_fingerprint, verified_fingerprint, total_size,
                     file_count, folder_count, is_suppressed)
                 SELECT n + 1, ?1, printf('s-%d', n), printf('v-%d', n), 1, 1, 2, 0
                 FROM numbers"
            ),
            params![run_id],
        )
        .unwrap();
    transaction
        .execute(
            &format!(
                "{numbers}
                 INSERT INTO duplicate_folder_group_member (group_id, directory_id)
                 SELECT n + 1, n * 2 + CASE suffix WHEN 'a' THEN 1 ELSE 2 END
                 FROM numbers
                 CROSS JOIN (SELECT 'a' AS suffix UNION ALL SELECT 'b')"
            ),
            [],
        )
        .unwrap();
    transaction.commit().unwrap();
    db.complete_scan_run(run_id, 0, 0, 0, 0, 100_000, 0, 0)
        .unwrap();

    let started = std::time::Instant::now();
    let plan = db.get_review_plan_view(run_id).unwrap();
    let plan_elapsed = started.elapsed();
    let first = db.page_review_folder_groups(run_id, 200, None).unwrap();
    let first_elapsed = started.elapsed() - plan_elapsed;
    let second = db
        .page_review_folder_groups(
            run_id,
            200,
            first.groups.last().map(|group| group.folder_group_id),
        )
        .unwrap();
    let second_elapsed = started.elapsed() - plan_elapsed - first_elapsed;
    assert_eq!(plan.summary.folder_undecided_count, 200_000);
    assert_eq!(plan.summary.intact_folder_copy_count, 200_000);
    assert_eq!(first.total, 100_000);
    assert_eq!(first.groups.len(), 200);
    assert_eq!(second.groups.len(), 200);
    assert!(first.has_more);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "100,000-group folder review queries took {:?} (plan {:?}, first {:?}, second {:?})",
        started.elapsed(),
        plan_elapsed,
        first_elapsed,
        second_elapsed
    );

    #[cfg(windows)]
    let baseline_private_bytes = current_process_private_bytes();
    #[cfg(windows)]
    let mut peak_private_bytes = baseline_private_bytes;
    let sample_count = if cfg!(debug_assertions) { 5 } else { 100 };
    let mut plan_durations = Vec::with_capacity(sample_count);
    let mut folder_page_durations = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        let started = std::time::Instant::now();
        let plan = db.get_review_plan_view(run_id).unwrap();
        plan_durations.push(started.elapsed());
        assert_eq!(plan.summary.intact_folder_copy_count, 200_000);

        let started = std::time::Instant::now();
        let page = db.page_review_folder_groups(run_id, 200, None).unwrap();
        folder_page_durations.push(started.elapsed());
        assert_eq!(page.groups.len(), 200);

        #[cfg(windows)]
        {
            peak_private_bytes = peak_private_bytes.max(current_process_private_bytes());
        }
    }
    plan_durations.sort_unstable();
    folder_page_durations.sort_unstable();
    let percentile_95 = |durations: &[std::time::Duration]| {
        durations[((durations.len() * 95).div_ceil(100)).saturating_sub(1)]
    };
    let plan_p95 = percentile_95(&plan_durations);
    let folder_page_p95 = percentile_95(&folder_page_durations);
    #[cfg(windows)]
    let private_growth_bytes = peak_private_bytes.saturating_sub(baseline_private_bytes);
    #[cfg(not(windows))]
    let private_growth_bytes = 0_u64;
    eprintln!(
        "folder-review-profile samples={} plan-p95={:.2}ms folder-groups-p95={:.2}ms private-growth={} bytes",
        sample_count,
        plan_p95.as_secs_f64() * 1000.0,
        folder_page_p95.as_secs_f64() * 1000.0,
        private_growth_bytes,
    );
    #[cfg(windows)]
    assert!(
        private_growth_bytes < 32 * 1024 * 1024,
        "repeated bounded folder review pages grew private memory by {private_growth_bytes} bytes"
    );
}

fn hundred_thousand_group_fixture() -> (Database, i64, DuplicateFileGroupPageQuery) {
    let db = Database::open_in_memory().unwrap();
    let (_, run_id) = session_and_run(&db, "Scale", &["/root"]);
    let transaction = db.connection().unchecked_transaction().unwrap();
    {
        let mut insert_group = transaction
            .prepare_cached(
                "INSERT INTO duplicate_group
                    (run_id, content_hash, file_size, file_count, wasted_bytes)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .unwrap();
        let mut insert_file = transaction
            .prepare_cached(
                "INSERT INTO scanned_file
                    (run_id, root_path, canonical_path, relative_path, file_name, parent_dir,
                     extension_key, drive_letter, file_size, last_modified)
                 VALUES (?1, '/root', ?2, ?3, ?3, '/root', 'bin', ?4, ?5, 0)",
            )
            .unwrap();
        let mut insert_member = transaction
            .prepare_cached(
                "INSERT INTO duplicate_group_member (group_id, file_id) VALUES (?1, ?2)",
            )
            .unwrap();
        for index in 0..100_000_i64 {
            let file_size = (index % 4096) + 1;
            let copy_count = if index % 1000 == 0 { 3 } else { 2 };
            insert_group
                .execute(params![
                    run_id,
                    index + 1,
                    file_size,
                    copy_count,
                    file_size * (copy_count - 1)
                ])
                .unwrap();
            if index % 1000 == 0 {
                let group_id = transaction.last_insert_rowid();
                for (copy, drive) in [("a", "D:"), ("b", "E:"), ("c", "D:")] {
                    let relative_path = format!("cross-{index}-{copy}.bin");
                    let canonical_path = format!("/root/{relative_path}");
                    insert_file
                        .execute(params![
                            run_id,
                            canonical_path,
                            relative_path,
                            drive,
                            file_size
                        ])
                        .unwrap();
                    insert_member
                        .execute(params![group_id, transaction.last_insert_rowid()])
                        .unwrap();
                }
            }
        }
    }
    transaction.commit().unwrap();
    db.complete_scan_run(run_id, 300, 300_000, 300, 100_000, 0, 0, 0)
        .unwrap();

    let query = DuplicateFileGroupPageQuery {
        run_id,
        limit: 200,
        sort_field: DuplicateFileGroupSortField::RecoverableBytes,
        sort_direction: SortDirection::Descending,
        filter: DuplicateFileGroupFilter {
            search: None,
            path_match: DuplicateFilePathMatchMode::Substring,
            extension_key: None,
            extension_match: DuplicateFileExtensionMatchMode::AnyMember,
            minimum_size: 0,
            minimum_copy_count: 2,
            across_drives: false,
            selected_root: None,
            selected_drive: None,
        },
        cursor: None,
    };
    (db, run_id, query)
}

fn populate_preference_members_for_all_groups(db: &Database, run_id: i64) {
    let transaction = db.connection().unchecked_transaction().unwrap();
    for (suffix, root, drive) in [("preferred", "/root", "D:"), ("other", "/secondary", "E:")] {
        transaction
            .execute(
                "INSERT INTO scanned_file
                    (run_id, root_path, canonical_path, relative_path, file_name, parent_dir,
                     extension_key, drive_letter, file_size, last_modified, file_identity)
                 SELECT dg.run_id, ?2,
                        ?2 || '/preference-' || dg.id || '-' || ?3 || '.bin',
                        'preference-' || dg.id || '-' || ?3 || '.bin',
                        'preference-' || dg.id || '-' || ?3 || '.bin',
                        ?2, 'bin', ?4, dg.file_size, 0,
                        'preference-' || dg.id || '-' || ?3
                 FROM duplicate_group dg
                 WHERE dg.run_id = ?1 AND dg.file_count = 2",
                params![run_id, root, suffix, drive],
            )
            .unwrap();
        transaction
            .execute(
                "INSERT INTO duplicate_group_member (group_id, file_id)
                 SELECT dg.id, sf.id
                 FROM duplicate_group dg
                 JOIN scanned_file sf
                   ON sf.run_id = dg.run_id
                  AND sf.canonical_path = ?2 || '/preference-' || dg.id || '-' || ?3 || '.bin'
                 WHERE dg.run_id = ?1 AND dg.file_count = 2",
                params![run_id, root, suffix],
            )
            .unwrap();
    }
    transaction.commit().unwrap();
}

#[cfg(windows)]
fn current_process_private_bytes() -> u64 {
    let mut counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    let result = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters as *mut PROCESS_MEMORY_COUNTERS_EX as *mut PROCESS_MEMORY_COUNTERS,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        )
    };
    assert_ne!(result, 0, "GetProcessMemoryInfo failed");
    counters.PrivateUsage as u64
}

#[test]
fn hundred_thousand_group_preference_preview_pages_and_summary_stay_bounded() {
    let _large_fixture_guard = LARGE_FIXTURE_TEST_LOCK.lock().unwrap();
    let (db, run_id, _) = hundred_thousand_group_fixture();
    populate_preference_members_for_all_groups(&db, run_id);
    let rule = db
        .save_preference_rule(
            "scale-preview-rule",
            None,
            "Scale preferred root",
            &["/root".to_owned(), "/missing".to_owned()],
            0,
        )
        .unwrap()
        .rule;
    let started = std::time::Instant::now();
    let first = db
        .page_preference_preview(
            run_id,
            rule.id,
            rule.revision,
            0,
            &PreferencePreviewScope::CompletedRun,
            50,
            None,
        )
        .unwrap();
    let first_elapsed = started.elapsed();
    assert_eq!(first.summary.scoped_group_count, 100_000);
    assert_eq!(first.summary.scoped_logical_path_count, 200_100);
    assert_eq!(first.summary.affected_group_count, 100_000);
    assert_eq!(first.summary.missing_rule_root_count, 1);
    assert_eq!(first.groups.len(), 50);
    assert!(first.has_more);
    let second_started = std::time::Instant::now();
    let second = db
        .page_preference_preview(
            run_id,
            rule.id,
            rule.revision,
            0,
            &PreferencePreviewScope::CompletedRun,
            50,
            first.groups.last().map(|group| group.group_id),
        )
        .unwrap();
    let second_elapsed = second_started.elapsed();
    assert_eq!(second.groups.len(), 50);
    assert!(
        first_elapsed < std::time::Duration::from_secs(5),
        "100,000-set preview first page exceeded the five-second regression ceiling: {first_elapsed:?}"
    );
    assert!(
        second_elapsed < std::time::Duration::from_secs(5),
        "100,000-set preview next page exceeded the five-second regression ceiling: {second_elapsed:?}"
    );
    db.connection()
        .execute(
            "INSERT INTO duplicate_group
                (run_id, content_hash, file_size, file_count, wasted_bytes)
             VALUES (?1, 200001, 200001, 2, 200001)",
            params![run_id],
        )
        .unwrap();
    assert!(matches!(
        db.page_preference_preview(
            run_id,
            rule.id,
            rule.revision,
            0,
            &PreferencePreviewScope::CompletedRun,
            50,
            None,
        ),
        Err(PreferenceError::PreviewTooComplex {
            scoped_group_count: 100_001,
            maximum_group_count: 100_000,
            scoped_logical_path_count: None,
            maximum_logical_path_count: 500_000,
        })
    ));
}

#[test]
#[ignore = "operator performance profile; run optimized with --ignored --nocapture"]
fn profile_hundred_thousand_group_preference_preview_queries() {
    let _large_fixture_guard = LARGE_FIXTURE_TEST_LOCK.lock().unwrap();
    let (db, run_id, _) = hundred_thousand_group_fixture();
    populate_preference_members_for_all_groups(&db, run_id);
    let rule = db
        .save_preference_rule(
            "scale-preview-profile",
            None,
            "Scale preferred root",
            &["/root".to_owned()],
            0,
        )
        .unwrap()
        .rule;
    let warm = db
        .page_preference_preview(
            run_id,
            rule.id,
            rule.revision,
            0,
            &PreferencePreviewScope::CompletedRun,
            50,
            None,
        )
        .unwrap();
    assert_eq!(warm.groups.len(), 50);
    let mut durations = Vec::with_capacity(100);
    #[cfg(windows)]
    let baseline_private_bytes = current_process_private_bytes();
    #[cfg(windows)]
    let mut peak_private_bytes = baseline_private_bytes;
    for _ in 0..100 {
        let started = std::time::Instant::now();
        let page = db
            .page_preference_preview(
                run_id,
                rule.id,
                rule.revision,
                0,
                &PreferencePreviewScope::CompletedRun,
                50,
                None,
            )
            .unwrap();
        durations.push(started.elapsed());
        assert_eq!(page.groups.len(), 50);
        #[cfg(windows)]
        {
            peak_private_bytes = peak_private_bytes.max(current_process_private_bytes());
        }
    }
    durations.sort_unstable();
    let p95 = durations[((durations.len() * 95).div_ceil(100)).saturating_sub(1)];
    #[cfg(windows)]
    let private_growth_bytes = peak_private_bytes.saturating_sub(baseline_private_bytes);
    #[cfg(not(windows))]
    let private_growth_bytes = 0_u64;
    eprintln!(
        "preference-preview-profile samples=100 p95={:.2}ms private-growth={} bytes",
        p95.as_secs_f64() * 1000.0,
        private_growth_bytes
    );
    #[cfg(windows)]
    assert!(
        private_growth_bytes < 32 * 1024 * 1024,
        "repeated preview queries grew private memory by {private_growth_bytes} bytes"
    );
}

#[test]
fn hundred_thousand_group_first_and_keyset_pages_stay_bounded() {
    let _large_fixture_guard = LARGE_FIXTURE_TEST_LOCK.lock().unwrap();
    let (db, run_id, query) = hundred_thousand_group_fixture();
    let started = std::time::Instant::now();
    let first = db.page_duplicate_file_groups(&query).unwrap();
    assert_eq!(first.total, 100_000);
    assert_eq!(first.summary.matching_group_count, 100_000);
    assert_eq!(first.summary.matching_copy_count, 200_100);
    assert_eq!(first.summary.distinct_selected_root_count, 1);
    assert_eq!(first.summary.distinct_drive_count, 2);
    assert_eq!(first.summary.across_drive_group_count, 100);
    assert_eq!(first.groups.len(), 200);
    assert!(first.has_more);
    let boundary = first.groups.last().unwrap();
    let second = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            cursor: Some(PageCursor {
                value: PageCursorValue::Integer(boundary.recoverable_bytes),
                id: boundary.id,
                before: false,
            }),
            ..query.clone()
        })
        .unwrap();
    assert_eq!(second.groups.len(), 200);
    assert!(second.groups.iter().all(|group| {
        !first
            .groups
            .iter()
            .any(|first_group| first_group.id == group.id)
    }));
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "indexed 100,000-group paging took {:?}",
        started.elapsed()
    );

    let size_started = std::time::Instant::now();
    let size_filter = DuplicateFileGroupFilter {
        minimum_size: 4_000,
        ..query.filter.clone()
    };
    let large_groups = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: size_filter.clone(),
            ..query.clone()
        })
        .unwrap();
    assert_eq!(large_groups.total, 2_328);
    assert_eq!(large_groups.summary.matching_group_count, 2_328);
    assert_eq!(large_groups.summary.matching_copy_count, 4_659);
    assert!(large_groups
        .groups
        .iter()
        .all(|group| group.file_size >= 4_000));
    let large_root_facets = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: size_filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(large_root_facets.total, 1);
    assert_eq!(large_root_facets.facets[0].matching_group_count, 3);
    let large_drive_facets = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: size_filter,
            cursor: None,
        })
        .unwrap();
    assert_eq!(large_drive_facets.total, 2);
    assert!(large_drive_facets
        .facets
        .iter()
        .all(|facet| facet.matching_group_count == 3));
    assert!(
        size_started.elapsed() < std::time::Duration::from_secs(5),
        "100,000-group minimum-size group/facet queries took {:?}",
        size_started.elapsed()
    );

    let across_started = std::time::Instant::now();
    let across = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: DuplicateFileGroupFilter {
                search: None,
                path_match: DuplicateFilePathMatchMode::Substring,
                extension_key: None,
                extension_match: DuplicateFileExtensionMatchMode::AnyMember,
                minimum_size: 0,
                minimum_copy_count: 2,
                across_drives: true,
                selected_root: None,
                selected_drive: None,
            },
            ..query.clone()
        })
        .unwrap();
    assert_eq!(across.total, 100);
    assert_eq!(across.summary.matching_group_count, 100);
    assert_eq!(across.summary.matching_copy_count, 300);
    assert_eq!(across.summary.distinct_selected_root_count, 1);
    assert_eq!(across.summary.distinct_drive_count, 2);
    assert_eq!(across.summary.across_drive_group_count, 100);
    assert_eq!(across.groups.len(), 100);
    assert!(!across.has_more);
    assert!(across
        .groups
        .iter()
        .all(|group| group.distinct_drive_count == 2));
    assert!(
        across_started.elapsed() < std::time::Duration::from_secs(5),
        "100,000-group across-drives filter took {:?}",
        across_started.elapsed()
    );

    let copy_count_started = std::time::Instant::now();
    let three_or_more_filter = DuplicateFileGroupFilter {
        minimum_copy_count: 3,
        ..query.filter.clone()
    };
    let three_or_more = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: three_or_more_filter.clone(),
            ..query.clone()
        })
        .unwrap();
    assert_eq!(three_or_more.total, 100);
    assert_eq!(three_or_more.summary.matching_copy_count, 300);
    assert!(three_or_more
        .groups
        .iter()
        .all(|group| group.file_count >= 3));
    let three_or_more_roots = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: three_or_more_filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(three_or_more_roots.facets[0].matching_group_count, 100);
    let three_or_more_drives = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: three_or_more_filter,
            cursor: None,
        })
        .unwrap();
    assert_eq!(three_or_more_drives.total, 2);
    assert!(three_or_more_drives
        .facets
        .iter()
        .all(|facet| facet.matching_group_count == 100));
    assert!(
        copy_count_started.elapsed() < std::time::Duration::from_secs(5),
        "100,000-group minimum-copy-count group/facet queries took {:?}",
        copy_count_started.elapsed()
    );

    let exact_path_started = std::time::Instant::now();
    let exact_path_filter = DuplicateFileGroupFilter {
        search: Some("/ROOT/CROSS-0-A.BIN".to_owned()),
        path_match: DuplicateFilePathMatchMode::Exact,
        ..query.filter.clone()
    };
    let exact_path_groups = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: exact_path_filter.clone(),
            ..query.clone()
        })
        .unwrap();
    assert_eq!(exact_path_groups.total, 1);
    assert_eq!(exact_path_groups.summary.matching_copy_count, 3);
    let exact_path_roots = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: exact_path_filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(exact_path_roots.facets[0].matching_group_count, 1);
    let exact_path_drives = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: exact_path_filter,
            cursor: None,
        })
        .unwrap();
    assert_eq!(exact_path_drives.total, 2);
    assert!(exact_path_drives
        .facets
        .iter()
        .all(|facet| facet.matching_group_count == 1));
    assert!(
        exact_path_started.elapsed() < std::time::Duration::from_secs(5),
        "100,000-group exact-path group/facet queries took {:?}",
        exact_path_started.elapsed()
    );

    let extension_started = std::time::Instant::now();
    let extension_filter = DuplicateFileGroupFilter {
        extension_key: Some("bin".to_owned()),
        ..query.filter.clone()
    };
    let extension_groups = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: extension_filter.clone(),
            ..query.clone()
        })
        .unwrap();
    assert_eq!(extension_groups.total, 100);
    assert_eq!(extension_groups.summary.matching_copy_count, 300);
    let extension_roots = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: extension_filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(extension_roots.facets[0].matching_group_count, 100);
    let extension_drives = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: extension_filter,
            cursor: None,
        })
        .unwrap();
    assert_eq!(extension_drives.total, 2);
    assert!(extension_drives
        .facets
        .iter()
        .all(|facet| facet.matching_group_count == 100));
    let all_extension_filter = DuplicateFileGroupFilter {
        extension_key: Some("bin".to_owned()),
        extension_match: DuplicateFileExtensionMatchMode::AllMembers,
        ..query.filter.clone()
    };
    let all_extension_groups = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: all_extension_filter.clone(),
            ..query.clone()
        })
        .unwrap();
    assert_eq!(all_extension_groups.total, 100);
    assert_eq!(all_extension_groups.summary.matching_copy_count, 300);
    let all_extension_roots = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: all_extension_filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(all_extension_roots.facets[0].matching_group_count, 100);
    let all_extension_drives = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: all_extension_filter,
            cursor: None,
        })
        .unwrap();
    assert_eq!(all_extension_drives.total, 2);
    assert!(all_extension_drives
        .facets
        .iter()
        .all(|facet| facet.matching_group_count == 100));
    assert!(
        extension_started.elapsed() < std::time::Duration::from_secs(5),
        "100,000-group any/all extension group/facet queries took {:?}",
        extension_started.elapsed()
    );

    let facet_started = std::time::Instant::now();
    let root_facets = db
        .page_duplicate_file_selected_root_facets(&DuplicateFileSelectedRootFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: query.filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(root_facets.total, 1);
    assert_eq!(root_facets.facets[0].value, "/root");
    assert_eq!(root_facets.facets[0].matching_group_count, 100);
    assert!(
        facet_started.elapsed() < std::time::Duration::from_secs(5),
        "100,000-group selected-root facet took {:?}",
        facet_started.elapsed()
    );

    let selected_root = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: DuplicateFileGroupFilter {
                selected_root: Some("/ROOT".to_owned()),
                ..query.filter.clone()
            },
            ..query.clone()
        })
        .unwrap();
    assert_eq!(selected_root.total, 100);
    assert!(selected_root
        .groups
        .iter()
        .all(|group| group.run_id == run_id));

    let drive_facet_started = std::time::Instant::now();
    let drive_facets = db
        .page_duplicate_file_drive_facets(&DuplicateFileDriveFacetPageQuery {
            run_id,
            limit: 25,
            sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
            sort_direction: SortDirection::Descending,
            filter: query.filter.clone(),
            cursor: None,
        })
        .unwrap();
    assert_eq!(drive_facets.total, 2);
    assert_eq!(drive_facets.facets[0].matching_group_count, 100);
    assert!(
        drive_facet_started.elapsed() < std::time::Duration::from_secs(5),
        "100,000-group drive facet took {:?}",
        drive_facet_started.elapsed()
    );

    let selected_drive = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: DuplicateFileGroupFilter {
                selected_drive: Some("d:".to_owned()),
                ..query.filter.clone()
            },
            ..query
        })
        .unwrap();
    assert_eq!(selected_drive.total, 100);

    let review_started = std::time::Instant::now();
    let review_page = db.page_review_groups(run_id, 200, None).unwrap();
    assert_eq!(review_page.total, 100_000);
    assert_eq!(review_page.groups.len(), 200);
    assert!(review_page.has_more);
    let next_review_page = db
        .page_review_groups(
            run_id,
            200,
            review_page.groups.last().map(|group| group.group_id),
        )
        .unwrap();
    assert_eq!(next_review_page.groups.len(), 200);
    assert!(
        review_started.elapsed() < std::time::Duration::from_secs(5),
        "100,000-group review paging took {:?}",
        review_started.elapsed()
    );
}

#[test]
#[ignore = "run explicitly in Release on representative Windows hardware"]
fn representative_review_workspace_profile() {
    let _large_fixture_guard = LARGE_FIXTURE_TEST_LOCK.lock().unwrap();
    let (db, run_id, query) = hundred_thousand_group_fixture();
    let root_query = DuplicateFileSelectedRootFacetPageQuery {
        run_id,
        limit: 25,
        sort_field: DuplicateFileSelectedRootFacetSortField::MatchingGroupCount,
        sort_direction: SortDirection::Descending,
        filter: query.filter.clone(),
        cursor: None,
    };
    let drive_query = DuplicateFileDriveFacetPageQuery {
        run_id,
        limit: 25,
        sort_field: DuplicateFileDriveFacetSortField::MatchingGroupCount,
        sort_direction: SortDirection::Descending,
        filter: query.filter.clone(),
        cursor: None,
    };

    assert_eq!(
        db.page_duplicate_file_groups(&query).unwrap().groups.len(),
        200
    );
    assert_eq!(
        db.page_duplicate_file_selected_root_facets(&root_query)
            .unwrap()
            .facets
            .len(),
        1
    );
    assert_eq!(
        db.page_duplicate_file_drive_facets(&drive_query)
            .unwrap()
            .facets
            .len(),
        2
    );
    assert_eq!(
        db.get_review_plan_view(run_id).unwrap().summary.keep_count,
        0
    );
    assert_eq!(
        db.page_review_groups(run_id, 200, None)
            .unwrap()
            .groups
            .len(),
        200
    );

    #[cfg(windows)]
    let baseline_private_bytes = current_process_private_bytes();
    #[cfg(windows)]
    let mut peak_private_bytes = baseline_private_bytes;
    let mut group_durations = Vec::with_capacity(100);
    let mut root_facet_durations = Vec::with_capacity(100);
    let mut drive_facet_durations = Vec::with_capacity(100);
    let mut review_plan_durations = Vec::with_capacity(100);
    let mut review_group_durations = Vec::with_capacity(100);

    for _ in 0..100 {
        let started = std::time::Instant::now();
        let group_page = db.page_duplicate_file_groups(&query).unwrap();
        group_durations.push(started.elapsed());
        assert_eq!(group_page.groups.len(), 200);

        let started = std::time::Instant::now();
        let root_page = db
            .page_duplicate_file_selected_root_facets(&root_query)
            .unwrap();
        root_facet_durations.push(started.elapsed());
        assert_eq!(root_page.facets.len(), 1);

        let started = std::time::Instant::now();
        let drive_page = db.page_duplicate_file_drive_facets(&drive_query).unwrap();
        drive_facet_durations.push(started.elapsed());
        assert_eq!(drive_page.facets.len(), 2);

        let started = std::time::Instant::now();
        let review_plan = db.get_review_plan_view(run_id).unwrap();
        review_plan_durations.push(started.elapsed());
        assert_eq!(review_plan.summary.keep_count, 0);

        let started = std::time::Instant::now();
        let review_groups = db.page_review_groups(run_id, 200, None).unwrap();
        review_group_durations.push(started.elapsed());
        assert_eq!(review_groups.groups.len(), 200);

        #[cfg(windows)]
        {
            peak_private_bytes = peak_private_bytes.max(current_process_private_bytes());
        }
    }

    for durations in [
        &mut group_durations,
        &mut root_facet_durations,
        &mut drive_facet_durations,
        &mut review_plan_durations,
        &mut review_group_durations,
    ] {
        durations.sort_unstable();
    }
    let percentile_95 = |durations: &[std::time::Duration]| {
        durations[((durations.len() * 95).div_ceil(100)).saturating_sub(1)]
    };
    let percentile_99 = |durations: &[std::time::Duration]| {
        durations[((durations.len() * 99).div_ceil(100)).saturating_sub(1)]
    };
    let group_p95 = percentile_95(&group_durations);
    let root_facet_p95 = percentile_95(&root_facet_durations);
    let drive_facet_p95 = percentile_95(&drive_facet_durations);
    let review_plan_p95 = percentile_95(&review_plan_durations);
    let review_group_p95 = percentile_95(&review_group_durations);

    #[cfg(windows)]
    let private_growth_bytes = peak_private_bytes.saturating_sub(baseline_private_bytes);
    #[cfg(not(windows))]
    let private_growth_bytes = 0_u64;
    eprintln!(
        "review-profile samples=100 groups-p50={:.2}ms groups-p95={:.2}ms groups-p99={:.2}ms root-facets-p95={:.2}ms drive-facets-p95={:.2}ms review-plan-p95={:.2}ms review-groups-p95={:.2}ms private-growth={} bytes",
        group_durations[group_durations.len() / 2].as_secs_f64() * 1000.0,
        group_p95.as_secs_f64() * 1000.0,
        percentile_99(&group_durations).as_secs_f64() * 1000.0,
        root_facet_p95.as_secs_f64() * 1000.0,
        drive_facet_p95.as_secs_f64() * 1000.0,
        review_plan_p95.as_secs_f64() * 1000.0,
        review_group_p95.as_secs_f64() * 1000.0,
        private_growth_bytes,
    );

    #[cfg(windows)]
    assert!(
        private_growth_bytes < 32 * 1024 * 1024,
        "repeated bounded review pages grew private memory by {} bytes",
        private_growth_bytes
    );
    let warm_target = std::time::Duration::from_millis(100);
    assert!(
        group_p95 < warm_target,
        "warm group-page p95 {:?} exceeded {:?}",
        group_p95,
        warm_target
    );
    assert!(
        root_facet_p95 < warm_target,
        "warm selected-root facet p95 {:?} exceeded {:?}",
        root_facet_p95,
        warm_target
    );
    assert!(
        drive_facet_p95 < warm_target,
        "warm drive facet p95 {:?} exceeded {:?}",
        drive_facet_p95,
        warm_target
    );
    assert!(
        review_plan_p95 < warm_target,
        "warm review-plan p95 {:?} exceeded {:?}",
        review_plan_p95,
        warm_target
    );
    assert!(
        review_group_p95 < warm_target,
        "warm review-group p95 {:?} exceeded {:?}",
        review_group_p95,
        warm_target
    );
}

#[test]
fn lifecycle_terminal_states_and_counters_are_durable() {
    let db = Database::open_in_memory().unwrap();
    let session = db
        .create_session("Lifecycle", &["/root".into()], &[])
        .unwrap();

    let completed = db
        .create_scan_run(session, &parameters(&["/root"], &[]), "test")
        .unwrap();
    db.start_scan_run(completed).unwrap();
    db.complete_scan_run(completed, 9, 900, 7, 2, 0, 300, 4)
        .unwrap();
    let completed_row = db.get_scan_run(completed).unwrap();
    assert_eq!(completed_row.status, "completed");
    assert_eq!(completed_row.files_discovered, 9);
    assert_eq!(completed_row.bytes_discovered, 900);
    assert_eq!(completed_row.files_hashed, 7);
    assert_eq!(completed_row.duplicate_file_groups, 2);
    assert_eq!(completed_row.wasted_bytes, 300);
    assert_eq!(completed_row.warning_count, 4);

    let cancelled = db
        .create_scan_run(session, &parameters(&["/root"], &[]), "test")
        .unwrap();
    db.start_scan_run(cancelled).unwrap();
    db.mark_run_cancelling(cancelled).unwrap();
    db.cancel_scan_run(cancelled).unwrap();
    assert_eq!(db.get_scan_run(cancelled).unwrap().status, "cancelled");

    let failed = db
        .create_scan_run(session, &parameters(&["/root"], &[]), "test")
        .unwrap();
    db.start_scan_run(failed).unwrap();
    db.fail_scan_run(failed, "disk full").unwrap();
    let failed_row = db.get_scan_run(failed).unwrap();
    assert_eq!(failed_row.status, "failed");
    assert_eq!(failed_row.error_message.as_deref(), Some("disk full"));

    let interrupted = db
        .create_scan_run(session, &parameters(&["/root"], &[]), "test")
        .unwrap();
    db.start_scan_run(interrupted).unwrap();
    db.interrupt_scan_run(interrupted, "worker stopped")
        .unwrap();
    assert_eq!(db.get_scan_run(interrupted).unwrap().status, "interrupted");

    assert!(db.complete_scan_run(failed, 0, 0, 0, 0, 0, 0, 0).is_err());
}

#[test]
fn opening_database_reconciles_abandoned_runs() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("reconcile.db");
    let db = Database::open(path.to_str().unwrap()).unwrap();
    let session = db
        .create_session("Recovery", &["/root".into()], &[])
        .unwrap();
    let running = db
        .create_scan_run(session, &parameters(&["/root"], &[]), "test")
        .unwrap();
    db.start_scan_run(running).unwrap();
    let cancelling = db
        .create_scan_run(session, &parameters(&["/root"], &[]), "test")
        .unwrap();
    db.start_scan_run(cancelling).unwrap();
    db.mark_run_cancelling(cancelling).unwrap();
    drop(db);

    let reopened = Database::open(path.to_str().unwrap()).unwrap();
    for run_id in [running, cancelling] {
        let run = reopened.get_scan_run(run_id).unwrap();
        assert_eq!(run.status, "interrupted");
        assert!(run.completed_at.is_some());
        assert!(run.error_message.is_some());
    }
}

#[test]
fn directory_rows_are_run_scoped() {
    let db = Database::open_in_memory().unwrap();
    let (session, first) = session_and_run(&db, "Folders", &["/root"]);
    let second = db
        .create_scan_run(session, &parameters(&["/root"], &[]), "test")
        .unwrap();
    db.start_scan_run(second).unwrap();
    let first_dir = db
        .insert_directory_node(first, "/root", "root", None, 1, 1, 1)
        .unwrap();
    let second_dir = db
        .insert_directory_node(second, "/root", "root", None, 2, 2, 1)
        .unwrap();
    assert_ne!(first_dir, second_dir);
    assert_eq!(
        db.get_directory_children(first, None, 0, 10).unwrap().len(),
        1
    );
    assert_eq!(
        db.get_directory_children(second, None, 0, 10).unwrap()[0].total_size,
        2
    );
}

#[test]
fn deletion_plan_uses_run_file_snapshots() {
    let db = Database::open_in_memory().unwrap();
    let (_, run_id) = session_and_run(&db, "Delete", &["/root"]);
    db.insert_scanned_files(&[file(run_id, "/root/a.txt", 500, 1)])
        .unwrap();
    let file_id: i64 = db
        .connection()
        .query_row(
            "SELECT id FROM scanned_file WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )
        .unwrap();
    db.mark_file_for_deletion(file_id, Some("test")).unwrap();
    assert_eq!(db.get_deletion_plan_summary().unwrap(), (1, 500));
    db.unmark_file_for_deletion(file_id).unwrap();
    assert_eq!(db.get_deletion_plan().unwrap().len(), 0);
}

#[test]
fn forward_migration_from_v2_preserves_recoverable_history() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("legacy.db");
    let legacy = Connection::open(&path).unwrap();
    legacy
        .execute_batch(
            "PRAGMA user_version = 2;
             PRAGMA foreign_keys = ON;
             CREATE TABLE scan_session (
                id INTEGER PRIMARY KEY AUTOINCREMENT, started_at TEXT NOT NULL,
                completed_at TEXT, status TEXT NOT NULL, root_paths TEXT NOT NULL,
                root_paths_hash TEXT, files_scanned INTEGER, total_bytes INTEGER);
             CREATE TABLE scanned_file (
                id INTEGER PRIMARY KEY AUTOINCREMENT, canonical_path TEXT NOT NULL UNIQUE,
                file_name TEXT NOT NULL, parent_dir TEXT NOT NULL, drive_letter TEXT,
                file_size INTEGER NOT NULL, last_modified INTEGER NOT NULL,
                partial_hash INTEGER, content_hash INTEGER,
                last_seen_session_id INTEGER REFERENCES scan_session(id),
                marked_deleted INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE duplicate_group (
                id INTEGER PRIMARY KEY AUTOINCREMENT, session_id INTEGER NOT NULL,
                content_hash INTEGER NOT NULL, file_size INTEGER NOT NULL,
                file_count INTEGER NOT NULL, wasted_bytes INTEGER NOT NULL);
             CREATE TABLE duplicate_group_member (
                id INTEGER PRIMARY KEY AUTOINCREMENT, group_id INTEGER NOT NULL,
                file_id INTEGER NOT NULL);
             CREATE TABLE directory_node (
                id INTEGER PRIMARY KEY AUTOINCREMENT, path TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL, parent_id INTEGER, total_size INTEGER,
                file_count INTEGER, depth INTEGER);
             CREATE TABLE directory_fingerprint (
                id INTEGER PRIMARY KEY AUTOINCREMENT, directory_id INTEGER NOT NULL UNIQUE,
                content_fingerprint TEXT NOT NULL, file_hash_set TEXT NOT NULL);
             CREATE TABLE directory_similarity (
                id INTEGER PRIMARY KEY AUTOINCREMENT, dir_a_id INTEGER NOT NULL,
                dir_b_id INTEGER NOT NULL, similarity_score REAL NOT NULL,
                shared_bytes INTEGER NOT NULL, match_type TEXT NOT NULL);
             CREATE TABLE deletion_plan (
                id INTEGER PRIMARY KEY AUTOINCREMENT, file_id INTEGER NOT NULL UNIQUE,
                marked_at TEXT NOT NULL, strategy TEXT, executed_at TEXT,
                execution_result TEXT);
             INSERT INTO scan_session VALUES
                (1, '2025-01-01T00:00:00Z', '2025-01-01T00:01:00Z', 'completed',
                 '[\"C:/Data\"]', '[\"C:/Data\"]', 2, 200);
             INSERT INTO scanned_file VALUES
                (1, 'C:/Data/a.txt', 'a.txt', 'C:/Data', 'C:', 100, 1, NULL, 77, 1, 0),
                (2, 'C:/Data/b.txt', 'b.txt', 'C:/Data', 'C:', 100, 1, NULL, 77, 1, 0);
             INSERT INTO duplicate_group VALUES (1, 1, 77, 100, 2, 100);
             INSERT INTO duplicate_group_member VALUES (1, 1, 1), (2, 1, 2);",
        )
        .unwrap();
    drop(legacy);

    let migrated = Database::open(path.to_str().unwrap()).unwrap();
    assert_eq!(migrated.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    let session = migrated.get_session(1).unwrap();
    assert_eq!(session.name, "Imported scan 1");
    let run = migrated.get_scan_run(1).unwrap();
    assert_eq!(run.status, "completed");
    assert_eq!(run.files_discovered, 2);
    assert_eq!(run.bytes_discovered, 200);
    assert_eq!(migrated.get_duplicate_groups(1, 0, 10).unwrap().len(), 1);
    assert_eq!(migrated.get_files_in_group(1).unwrap().len(), 2);
}
