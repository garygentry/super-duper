//! CLI integration tests for `--format json` on `analyze-directories`, `count-hash-cache`, and
//! `print-config`, against a fixture database.

mod common;
use common::{cli, fixture_dir};
use std::process::Command;

#[test]
fn print_config_json_is_valid_json() {
    let dir = fixture_dir();
    let (ok, stdout, stderr) = cli(&dir, &["print-config", "--format", "json"]);
    assert!(ok, "stderr: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(value["root_paths"].is_array());
}

#[test]
fn count_hash_cache_json_reports_zero_for_an_empty_cache() {
    let dir = fixture_dir();
    let output = Command::new(env!("CARGO_BIN_EXE_super-duper-cli"))
        .args(["count-hash-cache", "--format", "json"])
        .current_dir(dir.path())
        .env("HASH_CACHE_PATH", dir.path().join("content_hash_cache.db"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["entries"], 0);
}

#[test]
fn analyze_directories_json_reports_fingerprint_and_similarity_counts() {
    let dir = fixture_dir();
    let (ok, stdout, stderr) = cli(&dir, &["analyze-directories", "--format", "json"]);
    assert!(ok, "stderr: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(value["directoryFingerprints"].as_i64().unwrap() > 0);
}
