use rusqlite::{params, Connection, Error as SqlError};
use super_duper_core::storage::models::{
    CloudDetectionStatus, CloudPolicy, DuplicateFileGroupFilter, DuplicateFileGroupPageQuery,
    DuplicateFileGroupSortField, DuplicateFileMemberFilter, DuplicateFileMemberPageQuery,
    DuplicateFileMemberSortField, PageCursor, PageCursorValue, RegisteredCloudLocation,
    RunExclusionInsert, RunParameters, ScannedFile, SortDirection,
};
use super_duper_core::storage::sqlite::CURRENT_SCHEMA_VERSION;
use super_duper_core::storage::Database;
use tempfile::tempdir;

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

    assert_eq!(db.schema_version().unwrap(), 4);
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
            file(run_id, &format!("/root/{prefix}-gamma.bin"), 200, 33),
            file(run_id, &format!("/root/{prefix}-gamma-copy.bin"), 200, 33),
        ];
        files[1].root_path = "/selected-root".to_owned();
        files[1].relative_path = format!("{prefix}-alpha-copy.txt");
        files[1].drive_letter = "D:".to_owned();
        files[2].drive_letter = "D:".to_owned();
        files[3].drive_letter = "E:".to_owned();
        files[4].drive_letter = "d:".to_owned();
        files[5].drive_letter = "D:".to_owned();
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
            minimum_size: 0,
            across_drives: false,
        },
        cursor: None,
    };
    let first_page = db.page_duplicate_file_groups(&base_query).unwrap();
    assert_eq!(first_page.total, 3);
    assert_eq!(first_page.summary.matching_group_count, 3);
    assert_eq!(first_page.summary.matching_copy_count, 6);
    assert_eq!(first_page.summary.potential_recoverable_bytes, 500);
    assert_eq!(first_page.summary.largest_recoverable_bytes, 200);
    assert_eq!(first_page.groups.len(), 2);
    assert!(first_page.has_more);
    assert!(first_page
        .groups
        .iter()
        .all(|group| group.run_id == first_run));
    assert_eq!(first_page.groups[0].recoverable_bytes, 200);
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
                minimum_size: 100,
                across_drives: false,
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
    let group = &filtered.groups[0];
    assert_eq!(group.distinct_selected_root_count, 2);
    assert_eq!(group.distinct_drive_count, 1);

    let across_drives = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            limit: 10,
            filter: DuplicateFileGroupFilter {
                search: None,
                minimum_size: 0,
                across_drives: true,
            },
            cursor: None,
            ..base_query.clone()
        })
        .unwrap();
    assert_eq!(across_drives.total, 1);
    assert_eq!(across_drives.summary.matching_group_count, 1);
    assert_eq!(across_drives.summary.matching_copy_count, 2);
    assert_eq!(across_drives.summary.potential_recoverable_bytes, 200);
    assert_eq!(across_drives.groups[0].distinct_drive_count, 2);
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
fn hundred_thousand_group_first_and_keyset_pages_stay_bounded() {
    let db = Database::open_in_memory().unwrap();
    let (_, run_id) = session_and_run(&db, "Scale", &["/root"]);
    let transaction = db.connection().unchecked_transaction().unwrap();
    {
        let mut insert_group = transaction
            .prepare_cached(
                "INSERT INTO duplicate_group
                    (run_id, content_hash, file_size, file_count, wasted_bytes)
                 VALUES (?1, ?2, ?3, 2, ?3)",
            )
            .unwrap();
        let mut insert_file = transaction
            .prepare_cached(
                "INSERT INTO scanned_file
                    (run_id, root_path, canonical_path, relative_path, file_name, parent_dir,
                     drive_letter, file_size, last_modified)
                 VALUES (?1, '/root', ?2, ?3, ?3, '/root', ?4, ?5, 0)",
            )
            .unwrap();
        let mut insert_member = transaction
            .prepare_cached(
                "INSERT INTO duplicate_group_member (group_id, file_id) VALUES (?1, ?2)",
            )
            .unwrap();
        for index in 0..100_000_i64 {
            let file_size = (index % 4096) + 1;
            insert_group
                .execute(params![run_id, index + 1, file_size])
                .unwrap();
            if index % 1000 == 0 {
                let group_id = transaction.last_insert_rowid();
                for (copy, drive) in [("a", "D:"), ("b", "E:")] {
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

    let query = DuplicateFileGroupPageQuery {
        run_id,
        limit: 200,
        sort_field: DuplicateFileGroupSortField::RecoverableBytes,
        sort_direction: SortDirection::Descending,
        filter: DuplicateFileGroupFilter {
            search: None,
            minimum_size: 0,
            across_drives: false,
        },
        cursor: None,
    };
    let started = std::time::Instant::now();
    let first = db.page_duplicate_file_groups(&query).unwrap();
    assert_eq!(first.total, 100_000);
    assert_eq!(first.summary.matching_group_count, 100_000);
    assert_eq!(first.summary.matching_copy_count, 200_000);
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

    let across_started = std::time::Instant::now();
    let across = db
        .page_duplicate_file_groups(&DuplicateFileGroupPageQuery {
            filter: DuplicateFileGroupFilter {
                search: None,
                minimum_size: 0,
                across_drives: true,
            },
            ..query
        })
        .unwrap();
    assert_eq!(across.total, 100);
    assert_eq!(across.summary.matching_group_count, 100);
    assert_eq!(across.summary.matching_copy_count, 200);
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
