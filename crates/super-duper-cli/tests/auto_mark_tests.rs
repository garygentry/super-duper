//! CLI integration tests for `auto-mark`, against a fixture database with one duplicate group
//! (`\\?\D:\Photos\a.jpg` and `\\?\D:\Photos\Copy\a.jpg`, same content hash and modified time).

mod common;
use common::{cli, fixture_dir};
use super_duper_core::storage::Database;

fn marked_paths(dir: &tempfile::TempDir) -> Vec<String> {
    let db = Database::open(dir.path().join("super_duper.db").to_str().unwrap()).unwrap();
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

#[test]
fn auto_mark_keep_first_json_reports_the_marked_count() {
    let dir = fixture_dir();
    let (ok, stdout, stderr) = cli(&dir, &["auto-mark", "--format", "json"]);
    assert!(ok, "stderr: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["markedCount"], 1);

    // "\\?\D:\Photos\Copy\a.jpg" sorts before "\\?\D:\Photos\a.jpg" ('C' < 'a'), so it survives.
    assert_eq!(marked_paths(&dir), vec![r"\\?\D:\Photos\a.jpg".to_string()]);
}

#[test]
fn auto_mark_preferred_path_prefix_keeps_the_matching_file() {
    let dir = fixture_dir();
    let (ok, _stdout, stderr) = cli(
        &dir,
        &[
            "auto-mark",
            "--strategy",
            "preferred-path-prefix",
            "--prefix",
            r"\\?\D:\Photos\Copy",
        ],
    );
    assert!(ok, "stderr: {stderr}");

    assert_eq!(marked_paths(&dir), vec![r"\\?\D:\Photos\a.jpg".to_string()]);
}

#[test]
fn auto_mark_preferred_path_prefix_requires_a_prefix() {
    let dir = fixture_dir();
    let (ok, stdout, _stderr) = cli(&dir, &["auto-mark", "--strategy", "preferred-path-prefix"]);
    assert!(!ok);
    assert!(stdout.is_empty());
}

#[test]
fn auto_mark_rejects_an_unknown_run() {
    let dir = fixture_dir();
    let (ok, _stdout, stderr) = cli(&dir, &["auto-mark", "--run", "999"]);
    assert!(!ok);
    assert!(
        stderr.contains("999") || stderr.contains("run"),
        "stderr: {stderr}"
    );
}
