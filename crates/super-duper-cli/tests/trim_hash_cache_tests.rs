//! CLI integration tests for `trim-hash-cache`. The trim mechanics themselves (which entries age
//! out, legacy stores without last-seen data, failing while a scan holds the store) are covered by
//! `super-duper-core`'s `hasher::repeat_cache` and `hasher::cache` unit tests; these only cover the
//! CLI's argument wiring and output formats against a fixture.

mod common;
use common::{cli, fixture_dir};
use std::process::Command;

#[test]
fn trim_hash_cache_json_reports_zero_for_a_missing_cache() {
    let dir = fixture_dir();
    let output = Command::new(env!("CARGO_BIN_EXE_super-duper-cli"))
        .args(["trim-hash-cache", "--format", "json"])
        .current_dir(dir.path())
        .env("HASH_CACHE_PATH", dir.path().join("content_hash_cache.db"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["liveEntriesBefore"], 0);
    assert_eq!(value["removed"], 0);
    assert_eq!(
        value["unseenScans"],
        super_duper_core::hasher::cache::DEFAULT_TRIM_UNSEEN_GENERATIONS
    );
}

#[test]
fn trim_hash_cache_honors_an_explicit_unseen_scans_bound() {
    let dir = fixture_dir();
    let output = Command::new(env!("CARGO_BIN_EXE_super-duper-cli"))
        .args(["trim-hash-cache", "--unseen-scans", "3", "--format", "json"])
        .current_dir(dir.path())
        .env("HASH_CACHE_PATH", dir.path().join("content_hash_cache.db"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["unseenScans"], 3);
}

#[test]
fn trim_hash_cache_text_mode_succeeds_and_prints_nothing_to_stdout() {
    let dir = fixture_dir();
    let (ok, stdout, stderr) = cli(&dir, &["trim-hash-cache"]);
    assert!(ok, "stderr: {stderr}");
    assert!(
        stdout.is_empty(),
        "stdout must stay protocol-only: {stdout}"
    );
}
