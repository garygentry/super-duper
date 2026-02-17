use super_duper_core::storage::models::*;
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
        scan_session_id: session_id,
        marked_deleted: false,
    }
}

#[test]
fn test_create_and_complete_scan_session() {
    let db = Database::open_in_memory().unwrap();
    let session_id = db
        .create_scan_session(&["path/a".to_string(), "path/b".to_string()])
        .unwrap();
    assert!(session_id > 0);

    db.complete_scan_session(session_id, 42, 100_000).unwrap();

    let row: (String, i64, i64) = db
        .connection()
        .query_row(
            "SELECT status, files_scanned, total_bytes FROM scan_session WHERE id = ?1",
            rusqlite::params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(row.0, "completed");
    assert_eq!(row.1, 42);
    assert_eq!(row.2, 100_000);
}

#[test]
fn test_insert_and_query_scanned_files() {
    let db = Database::open_in_memory().unwrap();
    let session_id = db.create_scan_session(&["root".to_string()]).unwrap();

    let files = vec![
        make_test_scanned_file("/root/a.txt", 100, 111, session_id),
        make_test_scanned_file("/root/b.txt", 200, 222, session_id),
        make_test_scanned_file("/root/c.txt", 100, 111, session_id),
    ];
    let count = db.insert_scanned_files(&files).unwrap();
    assert_eq!(count, 3);

    // Verify via get_files_in_group after inserting a group
    let groups = vec![(111_i64, 100_i64, vec![
        "/root/a.txt".to_string(),
        "/root/c.txt".to_string(),
    ])];
    let gcount = db.insert_duplicate_groups(&groups).unwrap();
    assert_eq!(gcount, 1);

    let dg = db.get_duplicate_groups(0, 10).unwrap();
    assert_eq!(dg.len(), 1);
    let files_in_group = db.get_files_in_group(dg[0].id).unwrap();
    assert_eq!(files_in_group.len(), 2);
}

#[test]
fn test_insert_and_query_duplicate_groups() {
    let db = Database::open_in_memory().unwrap();
    let session_id = db.create_scan_session(&["root".to_string()]).unwrap();

    let files = vec![
        make_test_scanned_file("/a", 500, 10, session_id),
        make_test_scanned_file("/b", 500, 10, session_id),
        make_test_scanned_file("/c", 200, 20, session_id),
        make_test_scanned_file("/d", 200, 20, session_id),
        make_test_scanned_file("/e", 200, 20, session_id),
    ];
    db.insert_scanned_files(&files).unwrap();

    let groups = vec![
        (10_i64, 500_i64, vec!["/a".to_string(), "/b".to_string()]),
        (
            20_i64,
            200_i64,
            vec!["/c".to_string(), "/d".to_string(), "/e".to_string()],
        ),
    ];
    db.insert_duplicate_groups(&groups).unwrap();

    // Verify wasted_bytes DESC ordering
    let dg = db.get_duplicate_groups(0, 10).unwrap();
    assert_eq!(dg.len(), 2);
    // Group with wasted 500 (1*500) first, then 400 (2*200)
    assert!(dg[0].wasted_bytes >= dg[1].wasted_bytes);
}

#[test]
fn test_get_files_in_group() {
    let db = Database::open_in_memory().unwrap();
    let session_id = db.create_scan_session(&["root".to_string()]).unwrap();

    let files = vec![
        make_test_scanned_file("/x/file1.txt", 300, 99, session_id),
        make_test_scanned_file("/y/file2.txt", 300, 99, session_id),
    ];
    db.insert_scanned_files(&files).unwrap();

    let groups = vec![(
        99_i64,
        300_i64,
        vec!["/x/file1.txt".to_string(), "/y/file2.txt".to_string()],
    )];
    db.insert_duplicate_groups(&groups).unwrap();

    let dg = db.get_duplicate_groups(0, 10).unwrap();
    let files = db.get_files_in_group(dg[0].id).unwrap();
    assert_eq!(files.len(), 2);

    let paths: Vec<&str> = files.iter().map(|f| f.canonical_path.as_str()).collect();
    assert!(paths.contains(&"/x/file1.txt"));
    assert!(paths.contains(&"/y/file2.txt"));
}

#[test]
fn test_mark_and_unmark_file_for_deletion() {
    let db = Database::open_in_memory().unwrap();
    let session_id = db.create_scan_session(&["root".to_string()]).unwrap();

    let files = vec![make_test_scanned_file("/del/test.txt", 100, 1, session_id)];
    db.insert_scanned_files(&files).unwrap();

    // Get file id
    let file_id: i64 = db
        .connection()
        .query_row(
            "SELECT id FROM scanned_file WHERE canonical_path = '/del/test.txt'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    db.mark_file_for_deletion(file_id, Some("auto")).unwrap();
    let plan = db.get_deletion_plan().unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].file_id, file_id);
    assert_eq!(plan[0].strategy.as_deref(), Some("auto"));

    db.unmark_file_for_deletion(file_id).unwrap();
    let plan = db.get_deletion_plan().unwrap();
    assert_eq!(plan.len(), 0);
}

#[test]
fn test_get_deletion_plan_summary() {
    let db = Database::open_in_memory().unwrap();
    let session_id = db.create_scan_session(&["root".to_string()]).unwrap();

    let files = vec![
        make_test_scanned_file("/s/a.txt", 500, 1, session_id),
        make_test_scanned_file("/s/b.txt", 300, 2, session_id),
    ];
    db.insert_scanned_files(&files).unwrap();

    let id_a: i64 = db
        .connection()
        .query_row(
            "SELECT id FROM scanned_file WHERE canonical_path = '/s/a.txt'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let id_b: i64 = db
        .connection()
        .query_row(
            "SELECT id FROM scanned_file WHERE canonical_path = '/s/b.txt'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    db.mark_file_for_deletion(id_a, None).unwrap();
    db.mark_file_for_deletion(id_b, None).unwrap();

    let (count, bytes) = db.get_deletion_plan_summary().unwrap();
    assert_eq!(count, 2);
    assert_eq!(bytes, 800);
}

#[test]
fn test_truncate_all() {
    let db = Database::open_in_memory().unwrap();
    let session_id = db.create_scan_session(&["root".to_string()]).unwrap();

    let files = vec![make_test_scanned_file("/t.txt", 100, 1, session_id)];
    db.insert_scanned_files(&files).unwrap();

    db.truncate_all().unwrap();

    let count: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM scanned_file", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);

    let session_count: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM scan_session", [], |row| row.get(0))
        .unwrap();
    assert_eq!(session_count, 0);
}

#[test]
fn test_insert_directory_node_and_get_children() {
    let db = Database::open_in_memory().unwrap();

    let root_id = db
        .insert_directory_node("/root", "root", None, 1000, 10, 1)
        .unwrap();
    let child_a = db
        .insert_directory_node("/root/a", "a", Some(root_id), 400, 4, 2)
        .unwrap();
    let child_b = db
        .insert_directory_node("/root/b", "b", Some(root_id), 600, 6, 2)
        .unwrap();

    let children = db.get_directory_children(Some(root_id), 0, 10).unwrap();
    assert_eq!(children.len(), 2);
    // Ordered by total_size DESC
    assert_eq!(children[0].id, child_b);
    assert_eq!(children[1].id, child_a);

    // Root nodes (parent_id IS NULL)
    let roots = db.get_directory_children(None, 0, 10).unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].id, root_id);
}

#[test]
fn test_insert_and_query_directory_similarity() {
    let db = Database::open_in_memory().unwrap();

    let dir_a = db
        .insert_directory_node("/da", "da", None, 100, 5, 1)
        .unwrap();
    let dir_b = db
        .insert_directory_node("/db", "db", None, 100, 5, 1)
        .unwrap();

    db.insert_directory_similarity(dir_a, dir_b, 0.85, 80, "threshold")
        .unwrap();

    let pairs = db.get_similar_directories(0.5, 0, 10).unwrap();
    assert_eq!(pairs.len(), 1);
    assert!((pairs[0].similarity_score - 0.85).abs() < f64::EPSILON);
    assert_eq!(pairs[0].match_type, "threshold");

    // Below threshold
    let pairs = db.get_similar_directories(0.9, 0, 10).unwrap();
    assert_eq!(pairs.len(), 0);
}
