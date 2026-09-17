use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::UNIX_EPOCH;

use super_duper_core::analysis::deletion_plan;
use super_duper_core::hasher::xxhash::hash_file_streaming;
use super_duper_core::platform;
use super_duper_core::storage::models::{RunParameters, ScannedFile};
use super_duper_core::storage::Database;

fn create_run(db: &Database) -> i64 {
    let roots = vec!["root".to_string()];
    let session_id = db.create_session("Test", &roots, &[]).unwrap();
    create_run_in_session(db, session_id)
}

fn create_run_in_session(db: &Database, session_id: i64) -> i64 {
    let roots = vec!["root".to_string()];
    let run_id = db
        .create_scan_run(
            session_id,
            &RunParameters {
                roots,
                ignore_patterns: vec![],
                directory_similarity_threshold_millis: 500,
                repeat_cache_policy: Default::default(),
                cloud_policy: Default::default(),
                manual_location_exclusions: vec![],
                registered_cloud_locations: vec![],
                cloud_detection_status: Default::default(),
            },
            "test",
        )
        .unwrap();
    db.start_scan_run(run_id).unwrap();
    run_id
}

/// A database-only row; the path does not need to exist.
fn synthetic_file(run_id: i64, parent_dir: &str, file_name: &str, separator: char) -> ScannedFile {
    let canonical_path = format!("{parent_dir}{separator}{file_name}");
    ScannedFile {
        id: 0,
        run_id,
        root_path: String::new(),
        canonical_path: canonical_path.clone(),
        relative_path: canonical_path,
        file_name: file_name.to_string(),
        parent_dir: parent_dir.to_string(),
        drive_letter: String::new(),
        file_size: 1,
        last_modified: 1,
        partial_hash: None,
        content_hash: Some(1),
        file_identity: None,
        warning_message: None,
        marked_deleted: false,
    }
}

/// A row that matches the real file on disk, as a scan would record it.
fn snapshot_file(run_id: i64, path: &Path) -> ScannedFile {
    let canonical = std::fs::canonicalize(path).unwrap();
    let metadata = std::fs::metadata(&canonical).unwrap();
    let modified = metadata
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64;
    ScannedFile {
        id: 0,
        run_id,
        root_path: String::new(),
        canonical_path: canonical.to_string_lossy().into_owned(),
        relative_path: canonical.to_string_lossy().into_owned(),
        file_name: canonical
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        parent_dir: canonical.parent().unwrap().to_string_lossy().into_owned(),
        drive_letter: String::new(),
        file_size: metadata.len() as i64,
        last_modified: modified,
        partial_hash: None,
        content_hash: Some(
            hash_file_streaming(&canonical, &AtomicBool::new(false)).unwrap() as i64,
        ),
        file_identity: platform::file_identity(&canonical).unwrap(),
        warning_message: None,
        marked_deleted: false,
    }
}

fn file_id(db: &Database, run_id: i64, canonical_path: &str) -> i64 {
    db.connection()
        .query_row(
            "SELECT id FROM scanned_file WHERE run_id = ?1 AND canonical_path = ?2",
            rusqlite::params![run_id, canonical_path],
            |row| row.get(0),
        )
        .unwrap()
}

fn marked_paths(db: &Database) -> Vec<String> {
    let mut stmt = db
        .connection()
        .prepare(
            "SELECT sf.canonical_path FROM deletion_plan dp
             JOIN scanned_file sf ON sf.id = dp.file_id
             WHERE dp.executed_at IS NULL ORDER BY sf.canonical_path",
        )
        .unwrap();
    let rows = stmt.query_map([], |row| row.get(0)).unwrap();
    rows.collect::<Result<Vec<String>, _>>().unwrap()
}

fn execution_result(db: &Database, file_id: i64) -> Option<String> {
    db.connection()
        .query_row(
            "SELECT execution_result FROM deletion_plan WHERE file_id = ?1",
            rusqlite::params![file_id],
            |row| row.get(0),
        )
        .unwrap()
}

/// Two real duplicate files plus their scan rows and duplicate group.
struct DuplicatePair {
    _dir: tempfile::TempDir,
    db: Database,
    keep: ScannedFile,
    remove: ScannedFile,
}

impl DuplicatePair {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let keep_path = dir.path().join("keep.txt");
        let remove_path = dir.path().join("remove.txt");
        std::fs::write(&keep_path, b"duplicate content").unwrap();
        std::fs::write(&remove_path, b"duplicate content").unwrap();

        let db = Database::open_in_memory().unwrap();
        let run_id = create_run(&db);
        let mut keep = snapshot_file(run_id, &keep_path);
        let mut remove = snapshot_file(run_id, &remove_path);
        assert_eq!(keep.content_hash, remove.content_hash);
        assert!(
            keep.file_identity.is_some(),
            "platform must report file identity"
        );
        db.insert_scanned_files(&[keep.clone(), remove.clone()])
            .unwrap();
        db.insert_duplicate_groups(
            run_id,
            &[(
                keep.content_hash.unwrap(),
                keep.file_size,
                vec![keep.canonical_path.clone(), remove.canonical_path.clone()],
            )],
        )
        .unwrap();
        keep.id = file_id(&db, run_id, &keep.canonical_path);
        remove.id = file_id(&db, run_id, &remove.canonical_path);
        DuplicatePair {
            _dir: dir,
            db,
            keep,
            remove,
        }
    }
}

#[test]
fn mark_directory_treats_underscore_literally_and_requires_separator_boundary() {
    let db = Database::open_in_memory().unwrap();
    let run_id = create_run(&db);
    db.insert_scanned_files(&[
        synthetic_file(run_id, r"C:\Data\job_1", "a.txt", '\\'),
        synthetic_file(run_id, r"C:\Data\job_1\nested", "b.txt", '\\'),
        synthetic_file(run_id, r"C:\Data\job21", "c.txt", '\\'),
        synthetic_file(run_id, r"C:\Data\job_10", "d.txt", '\\'),
        synthetic_file(run_id, r"C:\Data\job_1x", "e.txt", '\\'),
    ])
    .unwrap();

    let marked =
        deletion_plan::mark_directory_for_deletion(&db, run_id, r"C:\Data\job_1", None).unwrap();

    assert_eq!(marked, 2);
    assert_eq!(
        marked_paths(&db),
        vec![
            r"C:\Data\job_1\a.txt".to_string(),
            r"C:\Data\job_1\nested\b.txt".to_string()
        ]
    );
}

#[test]
fn mark_directory_treats_percent_literally() {
    let db = Database::open_in_memory().unwrap();
    let run_id = create_run(&db);
    db.insert_scanned_files(&[
        synthetic_file(run_id, "/data/100%", "a.txt", '/'),
        synthetic_file(run_id, "/data/100%/sub", "b.txt", '/'),
        synthetic_file(run_id, "/data/100", "c.txt", '/'),
        synthetic_file(run_id, "/data/100percent", "d.txt", '/'),
        synthetic_file(run_id, "/data/100%-old", "e.txt", '/'),
    ])
    .unwrap();

    // A trailing separator on the requested directory is accepted.
    let marked =
        deletion_plan::mark_directory_for_deletion(&db, run_id, "/data/100%/", None).unwrap();

    assert_eq!(marked, 2);
    assert_eq!(
        marked_paths(&db),
        vec![
            "/data/100%/a.txt".to_string(),
            "/data/100%/sub/b.txt".to_string()
        ]
    );
}

#[test]
fn mark_directory_is_scoped_to_run() {
    let db = Database::open_in_memory().unwrap();
    let first_run = create_run(&db);
    db.fail_scan_run(first_run, "superseded").unwrap();
    let session_id: i64 = db
        .connection()
        .query_row(
            "SELECT session_id FROM scan_run WHERE id = ?1",
            rusqlite::params![first_run],
            |row| row.get(0),
        )
        .unwrap();
    let second_run = create_run_in_session(&db, session_id);
    db.insert_scanned_files(&[
        synthetic_file(first_run, "/target", "a.txt", '/'),
        synthetic_file(second_run, "/target", "a.txt", '/'),
    ])
    .unwrap();

    let marked =
        deletion_plan::mark_directory_for_deletion(&db, second_run, "/target", None).unwrap();

    assert_eq!(marked, 1);
    let plan = db.get_deletion_plan().unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].file_id, file_id(&db, second_run, "/target/a.txt"));
}

#[test]
fn mark_directory_rejects_empty_path() {
    let db = Database::open_in_memory().unwrap();
    let run_id = create_run(&db);
    assert!(deletion_plan::mark_directory_for_deletion(&db, run_id, "  ", None).is_err());
}

#[test]
fn execute_removes_duplicate_when_survivor_is_verified() {
    let pair = DuplicatePair::new();
    pair.db
        .mark_file_for_deletion(pair.remove.id, None)
        .unwrap();

    let (success, errors) = deletion_plan::execute_deletion_plan(&pair.db, false).unwrap();

    assert_eq!((success, errors), (1, 0));
    assert!(!Path::new(&pair.remove.canonical_path).exists());
    assert!(Path::new(&pair.keep.canonical_path).exists());
    assert_eq!(
        execution_result(&pair.db, pair.remove.id).as_deref(),
        Some("success")
    );
}

#[test]
fn execute_skips_when_survivor_was_removed_externally() {
    let pair = DuplicatePair::new();
    pair.db
        .mark_file_for_deletion(pair.remove.id, None)
        .unwrap();
    std::fs::remove_file(&pair.keep.canonical_path).unwrap();

    let (success, errors) = deletion_plan::execute_deletion_plan(&pair.db, false).unwrap();

    assert_eq!((success, errors), (0, 1));
    assert!(
        Path::new(&pair.remove.canonical_path).exists(),
        "the last copy must not be deleted"
    );
    assert_eq!(
        execution_result(&pair.db, pair.remove.id).as_deref(),
        Some("skipped:survivor_missing")
    );
}

#[test]
fn execute_skips_when_survivor_content_changed() {
    let pair = DuplicatePair::new();
    pair.db
        .mark_file_for_deletion(pair.remove.id, None)
        .unwrap();
    std::fs::write(&pair.keep.canonical_path, b"different content").unwrap();

    let (success, errors) = deletion_plan::execute_deletion_plan(&pair.db, false).unwrap();

    assert_eq!((success, errors), (0, 1));
    assert!(Path::new(&pair.remove.canonical_path).exists());
    assert_eq!(
        execution_result(&pair.db, pair.remove.id).as_deref(),
        Some("skipped:survivor_missing")
    );
}

#[test]
fn execute_skips_target_whose_content_changed_since_scan() {
    let pair = DuplicatePair::new();
    pair.db
        .mark_file_for_deletion(pair.remove.id, None)
        .unwrap();
    // Same length, different bytes: only the content hash (or timestamp) can detect it.
    std::fs::write(&pair.remove.canonical_path, b"DUPLICATE CONTENT").unwrap();

    let (success, errors) = deletion_plan::execute_deletion_plan(&pair.db, false).unwrap();

    assert_eq!((success, errors), (0, 1));
    assert_eq!(
        std::fs::read(&pair.remove.canonical_path).unwrap(),
        b"DUPLICATE CONTENT"
    );
    let result = execution_result(&pair.db, pair.remove.id).unwrap();
    assert!(
        result == "skipped:content_hash_changed" || result == "skipped:timestamp_changed",
        "unexpected result {result}"
    );
}

#[test]
fn execute_skips_target_replaced_by_different_file() {
    let pair = DuplicatePair::new();
    pair.db
        .mark_file_for_deletion(pair.remove.id, None)
        .unwrap();
    std::fs::remove_file(&pair.remove.canonical_path).unwrap();
    std::fs::write(&pair.remove.canonical_path, b"duplicate content").unwrap();

    let (success, errors) = deletion_plan::execute_deletion_plan(&pair.db, false).unwrap();

    assert_eq!((success, errors), (0, 1));
    assert!(Path::new(&pair.remove.canonical_path).exists());
    let result = execution_result(&pair.db, pair.remove.id).unwrap();
    assert!(result.starts_with("skipped:"), "unexpected result {result}");
}

#[test]
fn execute_never_removes_every_member_of_a_group() {
    let pair = DuplicatePair::new();
    pair.db.mark_file_for_deletion(pair.keep.id, None).unwrap();
    pair.db
        .mark_file_for_deletion(pair.remove.id, None)
        .unwrap();

    let (success, errors) = deletion_plan::execute_deletion_plan(&pair.db, false).unwrap();

    assert_eq!((success, errors), (1, 1));
    let remaining = [&pair.keep, &pair.remove]
        .iter()
        .filter(|file| Path::new(&file.canonical_path).exists())
        .count();
    assert_eq!(remaining, 1);
}

#[test]
fn execute_skips_file_without_duplicate_group() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unique.txt");
    std::fs::write(&path, b"unique").unwrap();
    let db = Database::open_in_memory().unwrap();
    let run_id = create_run(&db);
    let file = snapshot_file(run_id, &path);
    db.insert_scanned_files(std::slice::from_ref(&file))
        .unwrap();
    let id = file_id(&db, run_id, &file.canonical_path);
    db.mark_file_for_deletion(id, None).unwrap();

    let (success, errors) = deletion_plan::execute_deletion_plan(&db, false).unwrap();

    assert_eq!((success, errors), (0, 1));
    assert!(path.exists());
    assert_eq!(
        execution_result(&db, id).as_deref(),
        Some("skipped:no_duplicate_group")
    );
}
