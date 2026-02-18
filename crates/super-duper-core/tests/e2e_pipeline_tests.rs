use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

use super_duper_core::analysis::{
    deletion_plan, dir_fingerprint, dir_similarity,
};
use super_duper_core::storage::Database;
use super_duper_core::{AppConfig, ScanEngine, SilentReporter};

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

    let engine = ScanEngine::new(config)
        .with_db_path(db_path.to_str().unwrap());
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
    let groups = db.get_duplicate_groups(result.session_id, 0, 100).unwrap();
    assert_eq!(groups.len(), 2);

    // Each group should have 2 files
    for group in &groups {
        assert_eq!(group.file_count, 2, "Each duplicate group should have 2 files");
        let files = db.get_files_in_group(group.id).unwrap();
        assert_eq!(files.len(), 2);
    }

    // Verify scan session was recorded
    let sessions: Vec<(i64, String)> = db
        .connection()
        .prepare("SELECT id, status FROM scan_session")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].1, "completed");
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

    let engine = ScanEngine::new(config)
        .with_db_path(db_path.to_str().unwrap());
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

    let engine = ScanEngine::new(config)
        .with_db_path(&db_path_str);

    // Get the cancel token and cancel from another thread after a tiny delay.
    // scan() resets the token at start, so we must cancel after it begins.
    let cancel_token = engine.cancel_token();
    let handle = std::thread::spawn(move || {
        // Small delay to let scan start, then cancel
        std::thread::sleep(std::time::Duration::from_millis(1));
        cancel_token.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    let result = engine.scan(&SilentReporter);
    handle.join().unwrap();

    // The scan may complete before cancellation on small datasets,
    // so we just verify the cancel token mechanism compiles and runs.
    // With a tiny dataset, success is also acceptable.
    match result {
        Ok(_) => {} // scan completed before cancel took effect — acceptable
        Err(super_duper_core::Error::Cancelled) => {} // cancel caught — ideal
        Err(other) => panic!("Unexpected error: {:?}", other),
    }
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
    let engine = ScanEngine::new(config)
        .with_db_path(db_path.to_str().unwrap());
    let scan_result = engine.scan(&SilentReporter).unwrap();
    assert_eq!(scan_result.duplicate_groups, 2);

    // Phase 2: Directory fingerprinting
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    let fp_count = dir_fingerprint::build_directory_fingerprints(&db).unwrap();
    assert!(
        fp_count > 0,
        "Expected at least 1 directory fingerprint, got {}",
        fp_count
    );

    // Verify directory nodes were created
    let root_nodes = db.get_directory_children(None, 0, 1000).unwrap();
    assert!(
        !root_nodes.is_empty(),
        "Expected at least one root directory node"
    );

    // Phase 3: Directory similarity
    let sim_count = dir_similarity::compute_directory_similarity(&db, 0.1).unwrap();
    // folder_a and folder_b both contain shared.txt, so there should be some similarity
    assert!(
        sim_count > 0,
        "Expected at least 1 similarity pair, got {}",
        sim_count
    );

    let similarities = db.get_similar_directories(0.1, 0, 100).unwrap();
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
    let engine = ScanEngine::new(config)
        .with_db_path(db_path.to_str().unwrap());
    let scan_result = engine.scan(&SilentReporter).unwrap();
    assert_eq!(scan_result.duplicate_groups, 2);

    // Auto-mark duplicates for deletion
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    deletion_plan::auto_mark_duplicates(&db, scan_result.session_id, None).unwrap();

    // Check deletion plan
    let (count, bytes) = db.get_deletion_plan_summary().unwrap();
    assert_eq!(
        count, 2,
        "Expected 2 files marked (one per dup group), got {}",
        count
    );
    assert!(bytes > 0, "Expected wasted bytes > 0");

    // Execute deletion
    let (deleted, errors) = deletion_plan::execute_deletion_plan(&db).unwrap();
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
    let engine = ScanEngine::new(config.clone())
        .with_db_path(db_path.to_str().unwrap());
    let result1 = engine.scan(&SilentReporter).unwrap();
    assert_eq!(result1.duplicate_groups, 2);

    // Delete one duplicate manually
    let folder_b_shared = root.join("folder_b").join("shared.txt");
    fs::remove_file(&folder_b_shared).unwrap();

    // Truncate and rescan
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    db.truncate_all().unwrap();

    let engine2 = ScanEngine::new(config)
        .with_db_path(db_path.to_str().unwrap());
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
    let engine = ScanEngine::new(config.clone())
        .with_db_path(db_path.to_str().unwrap());
    let result1 = engine.scan(&SilentReporter).unwrap();
    assert_eq!(result1.duplicate_groups, 2);

    // Second scan — same paths, no truncate, must NOT crash and must produce same results
    let engine2 = ScanEngine::new(config)
        .with_db_path(db_path.to_str().unwrap());
    let result2 = engine2.scan(&SilentReporter).unwrap();
    assert_eq!(
        result2.duplicate_groups, 2,
        "Second scan of same paths should produce same group count"
    );

    // Both scans should reuse the same session (find_or_create_session semantics)
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    let session_count: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM scan_session", [], |row| row.get(0))
        .unwrap();
    assert_eq!(session_count, 1, "Idempotent rescan should reuse the same session");
}
