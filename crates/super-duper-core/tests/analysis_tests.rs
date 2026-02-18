use super_duper_core::analysis::{deletion_plan, dir_fingerprint, dir_similarity};
use super_duper_core::storage::models::ScannedFile;
use super_duper_core::storage::Database;

fn make_test_scanned_file(path: &str, size: i64, hash: i64, session_id: i64) -> ScannedFile {
    ScannedFile {
        id: 0,
        canonical_path: path.to_string(),
        file_name: path.rsplit('/').next().unwrap_or(path).to_string(),
        parent_dir: path
            .rsplit_once('/')
            .map(|(p, _)| p.to_string())
            .unwrap_or_default(),
        drive_letter: String::new(),
        file_size: size,
        last_modified: 1700000000,
        partial_hash: None,
        content_hash: Some(hash),
        last_seen_session_id: Some(session_id),
        marked_deleted: false,
    }
}

fn setup_db_with_files(dirs_and_files: &[(&str, i64, i64)]) -> (Database, i64) {
    let db = Database::open_in_memory().unwrap();
    let session_id = db.create_scan_session(&["root".to_string()]).unwrap();

    let files: Vec<ScannedFile> = dirs_and_files
        .iter()
        .map(|(path, size, hash)| make_test_scanned_file(path, *size, *hash, session_id))
        .collect();
    db.insert_scanned_files(&files).unwrap();
    (db, session_id)
}

#[test]
fn test_build_directory_fingerprints_empty_db() {
    let db = Database::open_in_memory().unwrap();
    let count = dir_fingerprint::build_directory_fingerprints(&db).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_build_directory_fingerprints_single_dir() {
    let (db, _) = setup_db_with_files(&[
        ("/dir/a.txt", 100, 111),
        ("/dir/b.txt", 200, 222),
        ("/dir/c.txt", 300, 333),
    ]);

    let count = dir_fingerprint::build_directory_fingerprints(&db).unwrap();
    assert!(count > 0);

    // Verify directory node exists
    let nodes = db.get_directory_children(None, 0, 100).unwrap();
    assert!(!nodes.is_empty());

    // Verify fingerprint stored
    let fp_count: i64 = db
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM directory_fingerprint",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(fp_count > 0);
}

#[test]
fn test_build_directory_fingerprints_nested_dirs() {
    let (db, _) = setup_db_with_files(&[
        ("/root/sub1/a.txt", 100, 111),
        ("/root/sub1/b.txt", 200, 222),
        ("/root/sub2/c.txt", 300, 333),
    ]);

    let count = dir_fingerprint::build_directory_fingerprints(&db).unwrap();
    assert!(count > 0);

    // Verify hierarchy: /root should be parent of /root/sub1 and /root/sub2
    let root_node: Option<i64> = db
        .connection()
        .query_row(
            "SELECT id FROM directory_node WHERE path = '/root'",
            [],
            |row| row.get(0),
        )
        .ok();

    if let Some(root_id) = root_node {
        let children = db.get_directory_children(Some(root_id), 0, 10).unwrap();
        assert!(children.len() >= 2);
    }

    // /root fingerprint should include hashes from both sub-directories
    let root_fp: Option<String> = db
        .connection()
        .query_row(
            "SELECT df.file_hash_set FROM directory_fingerprint df \
             JOIN directory_node dn ON df.directory_id = dn.id \
             WHERE dn.path = '/root'",
            [],
            |row| row.get(0),
        )
        .ok();

    if let Some(hash_set_json) = root_fp {
        let hashes: Vec<i64> = serde_json::from_str(&hash_set_json).unwrap();
        // Should contain all 3 unique hashes (111, 222, 333)
        assert_eq!(hashes.len(), 3);
    }
}

#[test]
fn test_fingerprint_determinism() {
    // Same hashes in different insertion order should produce the same fingerprint
    let db1 = Database::open_in_memory().unwrap();
    let s1 = db1.create_scan_session(&["root".to_string()]).unwrap();
    db1.insert_scanned_files(&[
        make_test_scanned_file("/d/z.txt", 100, 333, s1),
        make_test_scanned_file("/d/a.txt", 100, 111, s1),
        make_test_scanned_file("/d/m.txt", 100, 222, s1),
    ])
    .unwrap();
    dir_fingerprint::build_directory_fingerprints(&db1).unwrap();

    let db2 = Database::open_in_memory().unwrap();
    let s2 = db2.create_scan_session(&["root".to_string()]).unwrap();
    db2.insert_scanned_files(&[
        make_test_scanned_file("/d/a.txt", 100, 111, s2),
        make_test_scanned_file("/d/m.txt", 100, 222, s2),
        make_test_scanned_file("/d/z.txt", 100, 333, s2),
    ])
    .unwrap();
    dir_fingerprint::build_directory_fingerprints(&db2).unwrap();

    let fp1: String = db1
        .connection()
        .query_row(
            "SELECT df.content_fingerprint FROM directory_fingerprint df \
             JOIN directory_node dn ON df.directory_id = dn.id \
             WHERE dn.path = '/d'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    let fp2: String = db2
        .connection()
        .query_row(
            "SELECT df.content_fingerprint FROM directory_fingerprint df \
             JOIN directory_node dn ON df.directory_id = dn.id \
             WHERE dn.path = '/d'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(fp1, fp2);
}

#[test]
fn test_compute_similarity_exact_match() {
    let (db, _) = setup_db_with_files(&[
        ("/dir_a/x.txt", 100, 111),
        ("/dir_a/y.txt", 200, 222),
        ("/dir_b/x.txt", 100, 111),
        ("/dir_b/y.txt", 200, 222),
    ]);

    dir_fingerprint::build_directory_fingerprints(&db).unwrap();
    let count = dir_similarity::compute_directory_similarity(&db, 0.5).unwrap();
    assert!(count > 0);

    let pairs = db.get_similar_directories(1.0, 0, 10).unwrap();
    // dir_a and dir_b should be exact matches
    let exact_pair = pairs
        .iter()
        .find(|p| p.match_type == "exact" && p.similarity_score >= 1.0);
    assert!(exact_pair.is_some());
}

#[test]
fn test_compute_similarity_partial_overlap() {
    let (db, _) = setup_db_with_files(&[
        ("/dir_a/shared.txt", 100, 111),
        ("/dir_a/unique_a.txt", 200, 222),
        ("/dir_b/shared.txt", 100, 111),
        ("/dir_b/unique_b.txt", 200, 333),
    ]);

    dir_fingerprint::build_directory_fingerprints(&db).unwrap();
    dir_similarity::compute_directory_similarity(&db, 0.1).unwrap();

    let pairs = db.get_similar_directories(0.1, 0, 10).unwrap();
    // Should find dir_a vs dir_b with Jaccard ~0.33 (1 shared / 3 total unique hashes)
    let dir_pair = pairs.iter().find(|p| {
        p.similarity_score > 0.2 && p.similarity_score < 0.9
    });
    assert!(dir_pair.is_some(), "Expected partial overlap pair, got: {:?}", pairs);
}

#[test]
fn test_compute_similarity_below_threshold() {
    let (db, _) = setup_db_with_files(&[
        // dir_a has 4 unique hashes, dir_b has 4 unique hashes, 1 shared
        // Jaccard = 1/7 ≈ 0.14
        ("/dir_a/a1.txt", 100, 1),
        ("/dir_a/a2.txt", 100, 2),
        ("/dir_a/a3.txt", 100, 3),
        ("/dir_a/shared.txt", 100, 10),
        ("/dir_b/b1.txt", 100, 4),
        ("/dir_b/b2.txt", 100, 5),
        ("/dir_b/b3.txt", 100, 6),
        ("/dir_b/shared.txt", 100, 10),
    ]);

    dir_fingerprint::build_directory_fingerprints(&db).unwrap();
    dir_similarity::compute_directory_similarity(&db, 0.5).unwrap();

    // With threshold 0.5, this pair (Jaccard ≈ 0.14) should NOT be stored
    let pairs = db.get_similar_directories(0.5, 0, 10).unwrap();
    let dir_ab_pair = pairs.iter().find(|p| p.similarity_score < 0.5);
    assert!(dir_ab_pair.is_none());
}

#[test]
fn test_compute_similarity_subset() {
    let (db, _) = setup_db_with_files(&[
        ("/big/a.txt", 100, 111),
        ("/big/b.txt", 200, 222),
        ("/big/c.txt", 300, 333),
        ("/small/a.txt", 100, 111),
        ("/small/b.txt", 200, 222),
    ]);

    dir_fingerprint::build_directory_fingerprints(&db).unwrap();
    dir_similarity::compute_directory_similarity(&db, 0.5).unwrap();

    let pairs = db.get_similar_directories(0.5, 0, 10).unwrap();
    let subset_pair = pairs.iter().find(|p| p.match_type == "subset");
    assert!(subset_pair.is_some(), "Expected subset pair, got: {:?}", pairs);
}

#[test]
fn test_mark_directory_for_deletion() {
    let (db, _) = setup_db_with_files(&[
        ("/target/a.txt", 100, 111),
        ("/target/b.txt", 200, 222),
        ("/other/c.txt", 300, 333),
    ]);

    let marked = deletion_plan::mark_directory_for_deletion(&db, "/target", None).unwrap();
    assert_eq!(marked, 2);

    let plan = db.get_deletion_plan().unwrap();
    assert_eq!(plan.len(), 2);
}

#[test]
fn test_auto_mark_duplicates() {
    let (db, session_id) = setup_db_with_files(&[
        ("/z/beta.txt", 100, 111),
        ("/a/alpha.txt", 100, 111),
    ]);

    let groups = vec![(
        111_i64,
        100_i64,
        vec!["/z/beta.txt".to_string(), "/a/alpha.txt".to_string()],
    )];
    db.insert_duplicate_groups(session_id, &groups).unwrap();

    let marked = deletion_plan::auto_mark_duplicates(&db, session_id, Some("auto")).unwrap();
    assert_eq!(marked, 1);

    // /a/alpha.txt is first alphabetically, so /z/beta.txt should be marked
    let plan = db.get_deletion_plan().unwrap();
    assert_eq!(plan.len(), 1);
    let marked_file_id = plan[0].file_id;
    let marked_path: String = db
        .connection()
        .query_row(
            "SELECT canonical_path FROM scanned_file WHERE id = ?1",
            rusqlite::params![marked_file_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(marked_path, "/z/beta.txt");
}

#[test]
fn test_execute_deletion_plan_real_files() {
    use std::io::Write;

    let tmp = tempfile::tempdir().unwrap();
    let file_a = tmp.path().join("a.txt");
    let file_b = tmp.path().join("b.txt");

    std::fs::File::create(&file_a)
        .unwrap()
        .write_all(b"hello")
        .unwrap();
    std::fs::File::create(&file_b)
        .unwrap()
        .write_all(b"world")
        .unwrap();

    let db = Database::open_in_memory().unwrap();
    let session_id = db.create_scan_session(&["root".to_string()]).unwrap();

    let files = vec![
        make_test_scanned_file(file_a.to_str().unwrap(), 5, 1, session_id),
        make_test_scanned_file(file_b.to_str().unwrap(), 5, 2, session_id),
    ];
    db.insert_scanned_files(&files).unwrap();

    // Get file IDs
    let id_a: i64 = db
        .connection()
        .query_row(
            "SELECT id FROM scanned_file WHERE canonical_path = ?1",
            rusqlite::params![file_a.to_str().unwrap()],
            |row| row.get(0),
        )
        .unwrap();
    let id_b: i64 = db
        .connection()
        .query_row(
            "SELECT id FROM scanned_file WHERE canonical_path = ?1",
            rusqlite::params![file_b.to_str().unwrap()],
            |row| row.get(0),
        )
        .unwrap();

    db.mark_file_for_deletion(id_a, None).unwrap();
    db.mark_file_for_deletion(id_b, None).unwrap();

    let (success, errors) = deletion_plan::execute_deletion_plan(&db).unwrap();
    assert_eq!(success, 2);
    assert_eq!(errors, 0);

    assert!(!file_a.exists());
    assert!(!file_b.exists());
}
