use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use super_duper_core::analysis::exact_folders;
use super_duper_core::progress::{FolderAnalysisSubstage, ProgressReporter};
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
                repeat_cache_policy: Default::default(),
                cloud_policy: Default::default(),
                manual_location_exclusions: vec![],
                registered_cloud_locations: vec![],
                cloud_detection_status: Default::default(),
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
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, vec![hash as u8; size as usize]).unwrap();
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
        last_modified: 0,
        partial_hash: None,
        content_hash: Some(hash),
        file_identity: None,
        warning_message: None,
        marked_deleted: false,
    }
}

fn analyze(db: &Database, run_id: i64) -> exact_folders::ExactFolderAnalysis {
    exact_folders::analyze_exact_folders_cancellable(
        db,
        run_id,
        &AtomicBool::new(false),
        &SilentReporter,
    )
    .unwrap()
}

#[test]
fn streaming_tree_is_single_pass_bottom_up_and_batched() {
    let temp = TempDir::new().unwrap();
    let db = Database::open_in_memory().unwrap();
    let run = create_run(&db, temp.path(), "streaming");
    let mut files = Vec::new();
    for index in 0..1_030 {
        files.push(file(
            run,
            temp.path(),
            &format!("wide-{index:04}/item.bin"),
            1,
            7,
        ));
    }
    for copy in ["deep-a", "deep-b"] {
        files.push(file(
            run,
            temp.path(),
            &format!("{copy}/a/b/c/d/e/f/g/h/item.bin"),
            2,
            9,
        ));
    }
    db.insert_scanned_files(&files).unwrap();

    let result = analyze(&db, run);
    let node_count: i64 = db
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM directory_node WHERE run_id = ?1",
            rusqlite::params![run],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(result.scanned_file_passes, 1);
    assert_eq!(result.largest_persistence_batch, 1_024);
    assert_eq!(result.directory_fingerprints as i64, node_count);
    assert_eq!(page(&db, run).total, 2);
}

#[derive(Default)]
struct FolderProgressRecorder {
    updates: Mutex<Vec<(FolderAnalysisSubstage, usize, usize)>>,
}

impl ProgressReporter for FolderProgressRecorder {
    fn on_dir_analysis_substage(
        &self,
        substage: FolderAnalysisSubstage,
        completed: usize,
        total: usize,
    ) {
        self.updates
            .lock()
            .unwrap()
            .push((substage, completed, total));
    }
}

#[test]
fn folder_substage_progress_is_ordered_monotonic_complete_and_bounded() {
    let temp = TempDir::new().unwrap();
    let db = Database::open_in_memory().unwrap();
    let run = create_run(&db, temp.path(), "folder-progress");
    db.insert_scanned_files(&[
        file(run, temp.path(), "left/nested/item.bin", 2, 9),
        file(run, temp.path(), "right/nested/item.bin", 2, 9),
    ])
    .unwrap();
    let progress = FolderProgressRecorder::default();

    exact_folders::analyze_exact_folders_cancellable(&db, run, &AtomicBool::new(false), &progress)
        .unwrap();

    let updates = progress.updates.into_inner().unwrap();
    assert!(updates.len() <= 16);
    assert!(updates.windows(2).all(|pair| pair[0].0 <= pair[1].0
        && (pair[0].0 != pair[1].0 || (pair[0].2 == pair[1].2 && pair[0].1 <= pair[1].1))));
    for substage in [
        FolderAnalysisSubstage::Hierarchy,
        FolderAnalysisSubstage::StructuralCandidates,
        FolderAnalysisSubstage::Verification,
        FolderAnalysisSubstage::Persistence,
    ] {
        let stage = updates
            .iter()
            .filter(|update| update.0 == substage)
            .collect::<Vec<_>>();
        assert_eq!(stage.first().unwrap().1, 0);
        assert_eq!(stage.last().unwrap().1, stage.last().unwrap().2);
        assert!(stage.iter().all(|update| update.1 <= update.2));
    }
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

#[test]
fn hard_link_aliases_do_not_form_recoverable_file_or_folder_copies() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("root");
    let first = root.join("first");
    let second = root.join("second");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    let original = first.join("item.bin");
    std::fs::write(&original, b"one physical file").unwrap();
    std::fs::hard_link(&original, second.join("item.bin")).unwrap();
    let db_path = temp.path().join("hard-link-folders.db");

    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    })
    .with_db_path(db_path.to_str().unwrap())
    .scan(&SilentReporter)
    .unwrap();

    assert_eq!(result.duplicate_groups, 0);
    assert_eq!(result.duplicate_folder_groups, 0);
    assert_eq!(result.dir_similarity_pairs, 0);

    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    let similarity_rows: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM directory_similarity", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(similarity_rows, 0);
}
