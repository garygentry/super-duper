use rusqlite::{params, Connection, Error as SqlError};
use super_duper_core::storage::models::{RunParameters, ScannedFile};
use super_duper_core::storage::sqlite::CURRENT_SCHEMA_VERSION;
use super_duper_core::storage::Database;
use tempfile::tempdir;

fn parameters(roots: &[&str], ignores: &[&str]) -> RunParameters {
    RunParameters {
        roots: roots.iter().map(|value| value.to_string()).collect(),
        ignore_patterns: ignores.iter().map(|value| value.to_string()).collect(),
        directory_similarity_threshold_millis: 500,
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
