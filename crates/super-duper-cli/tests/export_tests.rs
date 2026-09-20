//! CLI integration tests for `export`, against a fixture database
//! (`docs/export-format-v1.md`).

mod common;
use common::{cli, fixture_dir};

#[test]
fn export_duplicate_groups_json_has_the_versioned_envelope_and_plain_paths() {
    let dir = fixture_dir();
    let (ok, stdout, stderr) = cli(&dir, &["export", "duplicate-groups", "--format", "json"]);
    assert!(ok, "stderr: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["formatVersion"], 1);
    assert_eq!(value["groups"][0]["contentHash"], "000000000000002a");
    assert!(
        !stdout.contains(r"\\?\"),
        "verbatim prefix leaked: {stdout}"
    );
}

#[test]
fn export_duplicate_groups_csv_defaults_to_csv_with_one_row_per_member() {
    let dir = fixture_dir();
    let (ok, stdout, stderr) = cli(&dir, &["export", "duplicate-groups"]);
    assert!(ok, "stderr: {stderr}");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().unwrap(),
        "groupId,contentHash,groupFileSize,groupFileCount,wastedBytes,path,fileName,parentDir,rootPath,relativePath,driveLetter,fileSize,lastModifiedUnixNanos"
    );
    assert_eq!(lines.count(), 2, "one row per member: {stdout}");
}

#[test]
fn export_duplicate_groups_rejects_an_unknown_run() {
    let dir = fixture_dir();
    let (ok, _stdout, stderr) = cli(&dir, &["export", "duplicate-groups", "--run", "999"]);
    assert!(!ok);
    assert!(stderr.contains("999"), "stderr: {stderr}");
}

#[test]
fn export_sessions_json_includes_run_history() {
    let dir = fixture_dir();
    let (ok, stdout, stderr) = cli(&dir, &["export", "sessions", "--format", "json"]);
    assert!(ok, "stderr: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["sessions"][0]["name"], "Photos");
    assert_eq!(value["sessions"][0]["runs"][0]["filesDiscovered"], 2);
}
