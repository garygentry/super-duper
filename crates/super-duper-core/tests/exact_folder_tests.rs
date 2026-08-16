use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use super_duper_core::analysis::{dir_fingerprint, exact_folders};
use super_duper_core::storage::models::{
    DuplicateFolderGroupFilter, DuplicateFolderGroupPageQuery, DuplicateFolderGroupSortField,
    RunParameters, ScannedFile, SortDirection,
};
use super_duper_core::storage::Database;
use super_duper_core::{AppConfig, ScanEngine, SilentReporter};
use tempfile::TempDir;

fn create_run(db: &Database, root: &Path, name: &str) -> i64 {
    let root = root.to_string_lossy().into_owned();
    let session = db
        .create_session(name, std::slice::from_ref(&root), &[])
        .unwrap();
    let run = db
        .create_scan_run(
            session,
            &RunParameters {
                roots: vec![root],
                ignore_patterns: vec![],
                directory_similarity_threshold_millis: 500,
            },
            "test",
        )
        .unwrap();
    db.start_scan_run(run).unwrap();
    run
}

fn file(run_id: i64, root: &Path, relative: &str, size: i64, hash: i64) -> ScannedFile {
    let path = relative
        .split('/')
        .fold(PathBuf::from(root), |path, part| path.join(part));
    ScannedFile {
        id: 0,
        run_id,
        root_path: root.to_string_lossy().into_owned(),
        canonical_path: path.to_string_lossy().into_owned(),
        relative_path: relative.to_owned(),
        file_name: path.file_name().unwrap().to_string_lossy().into_owned(),
        parent_dir: path.parent().unwrap().to_string_lossy().into_owned(),
        drive_letter: String::new(),
        file_size: size,
        last_modified: 1,
        partial_hash: None,
        content_hash: Some(hash),
        file_identity: None,
        warning_message: None,
        marked_deleted: false,
    }
}

fn analyze(db: &Database, run_id: i64) -> exact_folders::ExactFolderAnalysis {
    dir_fingerprint::build_directory_fingerprints(db, run_id).unwrap();
    exact_folders::analyze_exact_folders_cancellable(
        db,
        run_id,
        &AtomicBool::new(false),
        &SilentReporter,
    )
    .unwrap()
}

fn page(db: &Database, run_id: i64) -> super_duper_core::storage::models::DuplicateFolderGroupPage {
    db.page_duplicate_folder_groups(&DuplicateFolderGroupPageQuery {
        run_id,
        limit: 100,
        sort_field: DuplicateFolderGroupSortField::TotalBytes,
        sort_direction: SortDirection::Descending,
        filter: DuplicateFolderGroupFilter {
            search: None,
            minimum_size: 0,
        },
        cursor: None,
    })
    .unwrap()
}

#[test]
fn root_names_are_ignored_and_redundant_nested_matches_are_suppressed() {
    let temp = TempDir::new().unwrap();
    let db = Database::open_in_memory().unwrap();
    let run = create_run(&db, temp.path(), "nested");
    db.insert_scanned_files(&[
        file(run, temp.path(), "original/top.txt", 10, 1),
        file(run, temp.path(), "original/nested/item.bin", 20, 2),
        file(run, temp.path(), "renamed/top.txt", 10, 1),
        file(run, temp.path(), "renamed/nested/item.bin", 20, 2),
    ])
    .unwrap();

    let result = analyze(&db, run);
    let visible = page(&db, run);
    let retained: i64 = db
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM duplicate_folder_group WHERE run_id = ?1",
            rusqlite::params![run],
            |row| row.get(0),
        )
        .unwrap();
    let suppressed: i64 = db
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM duplicate_folder_group WHERE run_id = ?1 AND is_suppressed = 1",
            rusqlite::params![run],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(result.visible_groups, 1);
    assert_eq!(visible.total, 1);
    assert_eq!(visible.groups[0].file_count, 2);
    assert_eq!(retained, 2);
    assert_eq!(suppressed, 1);
}

#[test]
fn relative_paths_multiplicity_extra_files_and_changed_content_reject_candidates() {
    let temp = TempDir::new().unwrap();
    let db = Database::open_in_memory().unwrap();
    let run = create_run(&db, temp.path(), "verification");
    db.insert_scanned_files(&[
        file(run, temp.path(), "good-a/one.txt", 10, 1),
        file(run, temp.path(), "good-a/two.txt", 10, 1),
        file(run, temp.path(), "good-b/one.txt", 10, 1),
        file(run, temp.path(), "good-b/two.txt", 10, 1),
        // Same hashes and multiplicity, but different relative paths.
        file(run, temp.path(), "wrong-path/one.txt", 10, 1),
        file(run, temp.path(), "wrong-path/moved.txt", 10, 1),
        // Same structural shape, but one corresponding file changed.
        file(run, temp.path(), "changed/one.txt", 10, 1),
        file(run, temp.path(), "changed/two.txt", 10, 9),
        // A candidate with an extra occurrence of otherwise repeated content.
        file(run, temp.path(), "extra/one.txt", 10, 1),
        file(run, temp.path(), "extra/two.txt", 10, 1),
        file(run, temp.path(), "extra/three.txt", 10, 1),
    ])
    .unwrap();

    analyze(&db, run);
    let visible = page(&db, run);
    assert_eq!(visible.total, 1);
    assert_eq!(visible.groups[0].folder_count, 2);
    assert_eq!(visible.groups[0].file_count, 2);
}

#[test]
fn exact_folder_results_are_run_scoped() {
    let temp = TempDir::new().unwrap();
    let db = Database::open_in_memory().unwrap();
    let first = create_run(&db, temp.path(), "first");
    db.insert_scanned_files(&[
        file(first, temp.path(), "a/item.txt", 10, 1),
        file(first, temp.path(), "b/item.txt", 10, 1),
    ])
    .unwrap();
    analyze(&db, first);

    let second_root = temp.path().join("other");
    let second = create_run(&db, &second_root, "second");
    db.insert_scanned_files(&[file(second, &second_root, "only/item.txt", 10, 1)])
        .unwrap();
    analyze(&db, second);

    assert_eq!(page(&db, first).total, 1);
    assert_eq!(page(&db, second).total, 0);
}

#[test]
fn scanner_does_not_traverse_directory_links_or_reparse_points() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("linked.txt"), b"must not be scanned").unwrap();
    let link = root.join("linked-directory");

    #[cfg(windows)]
    {
        let status = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .status()
            .unwrap();
        assert!(status.success(), "test junction creation failed");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    let db_path = temp.path().join("links.db");
    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    })
    .with_db_path(db_path.to_str().unwrap())
    .scan(&SilentReporter)
    .unwrap();

    assert_eq!(result.total_files_scanned, 0);
}
