use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use tempfile::tempdir;

use super_duper_core::analysis::{deletion_plan, dir_fingerprint, dir_similarity};
use super_duper_core::storage::Database;
use super_duper_core::{AppConfig, ProgressReporter, ScanEngine, SilentReporter};

fn count_files_recursive(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files_recursive(&path);
            } else if path.is_file() {
                count += 1;
            }
        }
    }
    count
}

/// Create a temp directory tree with known duplicates.
/// Layout:
///   root/
///     folder_a/
///       unique_a.txt     ("unique content a")
///       shared.txt       ("shared content xyz")
///     folder_b/
///       unique_b.txt     ("unique content b")
///       shared.txt       ("shared content xyz")  ← duplicate of folder_a/shared.txt
///     folder_c/
///       large_dup_1.bin  (4KB of 0xAA)
///       large_dup_2.bin  (4KB of 0xAA)            ← duplicate within same folder
fn create_test_tree(root: &std::path::Path) {
    let folder_a = root.join("folder_a");
    let folder_b = root.join("folder_b");
    let folder_c = root.join("folder_c");
    fs::create_dir_all(&folder_a).unwrap();
    fs::create_dir_all(&folder_b).unwrap();
    fs::create_dir_all(&folder_c).unwrap();

    // Unique files
    fs::write(folder_a.join("unique_a.txt"), "unique content a").unwrap();
    fs::write(folder_b.join("unique_b.txt"), "unique content b").unwrap();

    // Cross-folder duplicates
    fs::write(folder_a.join("shared.txt"), "shared content xyz").unwrap();
    fs::write(folder_b.join("shared.txt"), "shared content xyz").unwrap();

    // Same-folder duplicates (larger, to exercise full-hash path)
    let large_content = vec![0xAAu8; 4096];
    let mut f1 = fs::File::create(folder_c.join("large_dup_1.bin")).unwrap();
    f1.write_all(&large_content).unwrap();
    let mut f2 = fs::File::create(folder_c.join("large_dup_2.bin")).unwrap();
    f2.write_all(&large_content).unwrap();
}

#[test]
fn test_full_scan_pipeline() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("scan_root");
    create_test_tree(&root);

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test_e2e.db");

    let config = AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    };

    let engine = ScanEngine::new(config).with_db_path(db_path.to_str().unwrap());
    let result = engine.scan(&SilentReporter).unwrap();

    // We expect at least 6 files scanned (2 unique + 2 shared + 2 large)
    assert!(
        result.total_files_scanned >= 6,
        "Expected at least 6 files, got {}",
        result.total_files_scanned
    );

    // We expect 2 duplicate groups:
    // 1) shared.txt (folder_a + folder_b)
    // 2) large_dup_1.bin + large_dup_2.bin
    assert_eq!(
        result.duplicate_groups, 2,
        "Expected 2 duplicate groups, got {}",
        result.duplicate_groups
    );

    // Verify we can read back from the database
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    let groups = db.get_duplicate_groups(result.run_id, 0, 100).unwrap();
    assert_eq!(groups.len(), 2);

    // Each group should have 2 files
    for group in &groups {
        assert_eq!(
            group.file_count, 2,
            "Each duplicate group should have 2 files"
        );
        let files = db.get_files_in_group(group.id).unwrap();
        assert_eq!(files.len(), 2);
    }

    // Verify the immutable run was recorded separately from its reusable session.
    let sessions: Vec<(i64, String)> = db
        .connection()
        .prepare("SELECT id, status FROM scan_run")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].1, "completed");
    assert_eq!(result.total_files_scanned, 6);
    assert_eq!(result.files_hashed, 6);
    assert_eq!(result.duplicate_files, 4);
    let run = db.get_scan_run(result.run_id).unwrap();
    assert_eq!(run.files_discovered, 6);
    assert_eq!(run.files_hashed, 6);
    assert_eq!(run.duplicate_file_groups, 2);
    assert_eq!(run.bytes_discovered as u64, result.total_bytes_discovered);
}

#[test]
fn test_scan_with_ignore_patterns() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("scan_ignore");
    create_test_tree(&root);

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test_ignore.db");

    // Ignore folder_c entirely
    let config = AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec!["**/folder_c/**".to_string()],
    };

    let engine = ScanEngine::new(config).with_db_path(db_path.to_str().unwrap());
    let result = engine.scan(&SilentReporter).unwrap();

    // With folder_c ignored, only 1 duplicate group (shared.txt)
    assert_eq!(
        result.duplicate_groups, 1,
        "Expected 1 duplicate group with folder_c ignored, got {}",
        result.duplicate_groups
    );
}

#[test]
fn test_scan_cancellation() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("scan_cancel");
    create_test_tree(&root);

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test_cancel.db");
    let db_path_str = db_path.to_str().unwrap().to_string();

    let config = AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    };

    let engine = ScanEngine::new(config).with_db_path(&db_path_str);

    let cancel_token = engine.cancel_token();
    struct CancelOnStart(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl ProgressReporter for CancelOnStart {
        fn on_scan_start(&self) {
            self.0.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    let result = engine.scan(&CancelOnStart(cancel_token));
    assert!(matches!(result, Err(super_duper_core::Error::Cancelled)));
    let db = Database::open(&db_path_str).unwrap();
    let (_, run_count) = db.list_runs(0, 10).unwrap();
    assert_eq!(run_count, 1);
    assert_eq!(db.list_runs(0, 10).unwrap().0[0].status, "cancelled");
}

#[test]
fn test_pipeline_failure_is_persisted_as_failed() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("scan_failure");
    create_test_tree(&root);
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test_failure.db");

    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    db.connection()
        .execute_batch(
            "CREATE TRIGGER force_result_failure BEFORE INSERT ON scanned_file
             BEGIN SELECT RAISE(ABORT, 'forced persistence failure'); END;",
        )
        .unwrap();
    drop(db);

    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    })
    .with_db_path(db_path.to_str().unwrap())
    .scan(&SilentReporter);
    assert!(result.is_err());

    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    let run = db.list_runs(0, 10).unwrap().0.remove(0);
    assert_eq!(run.status, "failed");
    assert!(run.completed_at.is_some());
    assert!(run
        .error_message
        .as_deref()
        .unwrap_or_default()
        .contains("forced persistence failure"));
}

#[test]
fn test_full_pipeline_with_directory_analysis() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("scan_diranalysis");
    create_test_tree(&root);

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test_diranalysis.db");

    let config = AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    };

    // Phase 1: Run scan
    let engine = ScanEngine::new(config).with_db_path(db_path.to_str().unwrap());
    let scan_result = engine.scan(&SilentReporter).unwrap();
    assert_eq!(scan_result.duplicate_groups, 2);

    // Phase 2: Directory fingerprinting
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    let fp_count = dir_fingerprint::build_directory_fingerprints(&db, scan_result.run_id).unwrap();
    assert!(
        fp_count > 0,
        "Expected at least 1 directory fingerprint, got {}",
        fp_count
    );

    // Verify directory nodes were created
    let root_nodes = db
        .get_directory_children(scan_result.run_id, None, 0, 1000)
        .unwrap();
    assert!(
        !root_nodes.is_empty(),
        "Expected at least one root directory node"
    );

    // Phase 3: Directory similarity
    let sim_count =
        dir_similarity::compute_directory_similarity(&db, scan_result.run_id, 0.1).unwrap();
    // folder_a and folder_b both contain shared.txt, so there should be some similarity
    assert!(
        sim_count > 0,
        "Expected at least 1 similarity pair, got {}",
        sim_count
    );

    let similarities = db
        .get_similar_directories(scan_result.run_id, 0.1, 0, 100)
        .unwrap();
    assert!(!similarities.is_empty());
    for sim in &similarities {
        assert!(sim.similarity_score >= 0.1);
    }
}

#[test]
fn test_full_pipeline_with_deletion() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("scan_deletion");
    create_test_tree(&root);

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test_deletion.db");

    let config = AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    };

    // Scan
    let engine = ScanEngine::new(config).with_db_path(db_path.to_str().unwrap());
    let scan_result = engine.scan(&SilentReporter).unwrap();
    assert_eq!(scan_result.duplicate_groups, 2);

    // Auto-mark duplicates for deletion
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    deletion_plan::auto_mark_duplicates(&db, scan_result.run_id, None).unwrap();

    // Check deletion plan
    let (count, bytes) = db.get_deletion_plan_summary().unwrap();
    assert_eq!(
        count, 2,
        "Expected 2 files marked (one per dup group), got {}",
        count
    );
    assert!(bytes > 0, "Expected wasted bytes > 0");

    // Execute deletion
    let (deleted, errors) = deletion_plan::execute_deletion_plan(&db, false).unwrap();
    assert_eq!(deleted, 2);
    assert_eq!(errors, 0);

    // Verify files are gone
    // auto_mark keeps first alphabetically, so the later-sorted duplicates were deleted
    let remaining_files = count_files_recursive(&root);

    // Started with 6 files, deleted 2 → 4 remaining
    assert_eq!(
        remaining_files, 4,
        "Expected 4 remaining files after deletion, got {}",
        remaining_files
    );
}

#[test]
fn test_rescan_after_deletion() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("scan_rescan");
    create_test_tree(&root);

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test_rescan.db");

    let config = AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    };

    // First scan
    let engine = ScanEngine::new(config.clone()).with_db_path(db_path.to_str().unwrap());
    let result1 = engine.scan(&SilentReporter).unwrap();
    assert_eq!(result1.duplicate_groups, 2);

    // Delete one duplicate manually
    let folder_b_shared = root.join("folder_b").join("shared.txt");
    fs::remove_file(&folder_b_shared).unwrap();

    // Truncate and rescan
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    db.truncate_all().unwrap();

    let engine2 = ScanEngine::new(config).with_db_path(db_path.to_str().unwrap());
    let result2 = engine2.scan(&SilentReporter).unwrap();

    // Now only 1 duplicate group (the large files in folder_c)
    assert_eq!(
        result2.duplicate_groups, 1,
        "After removing one shared.txt, expected 1 dup group, got {}",
        result2.duplicate_groups
    );
}

#[test]
fn test_idempotent_rescan() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("scan_idempotent");
    create_test_tree(&root);

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test_idempotent.db");

    let config = AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    };

    // First scan
    let engine = ScanEngine::new(config.clone()).with_db_path(db_path.to_str().unwrap());
    let result1 = engine.scan(&SilentReporter).unwrap();
    assert_eq!(result1.duplicate_groups, 2);

    // Second scan — same paths, no truncate, must NOT crash and must produce same results
    let engine2 = ScanEngine::new(config).with_db_path(db_path.to_str().unwrap());
    let result2 = engine2.scan(&SilentReporter).unwrap();
    assert_eq!(
        result2.duplicate_groups, 2,
        "Second scan of same paths should produce same group count"
    );

    // Both scans reuse one definition but preserve two immutable executions.
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    let session_count: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM scan_session", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        session_count, 1,
        "Idempotent rescan should reuse the same session"
    );
    let run_count: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM scan_run", [], |row| row.get(0))
        .unwrap();
    assert_eq!(run_count, 2);
    assert_ne!(result1.run_id, result2.run_id);
}

#[test]
fn hard_links_are_snapshotted_but_not_counted_as_recoverable_copies() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("hard-links");
    fs::create_dir(&root).unwrap();
    let original = root.join("original.bin");
    let linked = root.join("linked.bin");
    fs::write(&original, b"one physical allocation").unwrap();
    fs::hard_link(&original, &linked).unwrap();
    let db_path = tmp.path().join("hard-links.db");

    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    })
    .with_db_path(db_path.to_str().unwrap())
    .scan(&SilentReporter)
    .unwrap();

    assert_eq!(result.total_files_scanned, 2);
    assert_eq!(result.duplicate_groups, 0);
    let db = Database::open_connection(db_path.to_str().unwrap()).unwrap();
    let identities = db
        .get_scanned_files(result.run_id)
        .unwrap()
        .into_iter()
        .map(|file| file.file_identity)
        .collect::<Vec<_>>();
    assert!(identities.iter().all(Option::is_some));
    assert_eq!(identities[0], identities[1]);
}

#[test]
fn files_changed_or_removed_after_discovery_become_warnings_not_false_results() {
    struct MutateAfterDiscovery {
        change: PathBuf,
        remove: PathBuf,
    }

    impl ProgressReporter for MutateAfterDiscovery {
        fn on_scan_complete(&self, _total_files: usize, _duration_secs: f64) {
            fs::write(&self.change, b"changed-data").unwrap();
            fs::remove_file(&self.remove).unwrap();
        }
    }

    let tmp = tempdir().unwrap();
    let root = tmp.path().join("volatile");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("stable-a.bin"), b"stable").unwrap();
    fs::write(root.join("stable-b.bin"), b"stable").unwrap();
    let changed = root.join("changed.bin");
    let removed = root.join("removed.bin");
    fs::write(&changed, b"initial-data").unwrap();
    fs::write(&removed, b"initial-data").unwrap();
    let db_path = tmp.path().join("volatile.db");

    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    })
    .with_db_path(db_path.to_str().unwrap())
    .scan(&MutateAfterDiscovery {
        change: changed,
        remove: removed,
    })
    .unwrap();

    assert_eq!(result.duplicate_groups, 1);
    assert!(result.warning_count >= 2, "result: {result:#?}");
    let db = Database::open_connection(db_path.to_str().unwrap()).unwrap();
    let warning_snapshots = db
        .get_scanned_files(result.run_id)
        .unwrap()
        .into_iter()
        .filter(|file| file.warning_message.is_some())
        .collect::<Vec<_>>();
    assert!(warning_snapshots.len() >= 2);
    assert!(warning_snapshots
        .iter()
        .all(|file| file.content_hash.is_none()));
    let grouped_names = db
        .get_duplicate_groups(result.run_id, 0, 100)
        .unwrap()
        .into_iter()
        .flat_map(|group| db.get_files_in_group(group.id).unwrap())
        .map(|file| file.file_name)
        .collect::<Vec<_>>();
    assert!(!grouped_names.iter().any(|name| name == "changed.bin"));
    assert!(!grouped_names.iter().any(|name| name == "removed.bin"));
}

#[cfg(windows)]
#[test]
fn windows_long_paths_scan_without_truncation() {
    let tmp = tempdir().unwrap();
    let mut root = tmp.path().join("long-path");
    while root.as_os_str().len() < 280 {
        root.push("segment-0123456789abcdef");
    }
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("copy-a.bin"), b"long path duplicate").unwrap();
    fs::write(root.join("copy-b.bin"), b"long path duplicate").unwrap();
    let db_path = tmp.path().join("long-path.db");

    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    })
    .with_db_path(db_path.to_str().unwrap())
    .scan(&SilentReporter)
    .unwrap();

    assert_eq!(result.total_files_scanned, 2);
    assert_eq!(result.duplicate_groups, 1);
}

#[cfg(windows)]
#[test]
fn windows_sharing_violations_are_recoverable_scan_warnings() {
    use std::os::windows::fs::OpenOptionsExt;

    let tmp = tempdir().unwrap();
    let root = tmp.path().join("locked-file");
    fs::create_dir(&root).unwrap();
    let locked_path = root.join("locked.bin");
    fs::write(&locked_path, b"locked content").unwrap();
    let _exclusive = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&locked_path)
        .unwrap();
    let db_path = tmp.path().join("locked-file.db");

    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    })
    .with_db_path(db_path.to_str().unwrap())
    .scan(&SilentReporter)
    .unwrap();

    assert!(result.warning_count >= 1);
    let db = Database::open_connection(db_path.to_str().unwrap()).unwrap();
    assert_eq!(db.get_scan_run(result.run_id).unwrap().status, "completed");
}
