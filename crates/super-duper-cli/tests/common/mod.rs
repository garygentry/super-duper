//! Shared fixture for CLI integration tests: a temp directory containing a `Config.toml` and a
//! `super_duper.db` with one completed run, two duplicate files, and one duplicate group.

use std::process::Command;
use super_duper_core::storage::Database;
use super_duper_core::storage::models::{RunParameters, ScannedFile};
use tempfile::TempDir;

pub fn fixture_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    // main() loads Config.toml unconditionally, before dispatching any subcommand.
    std::fs::write(
        dir.path().join("Config.toml"),
        "root_paths = []\nignore_patterns = []\n",
    )
    .unwrap();
    let db_path = dir.path().join("super_duper.db");
    let db = Database::open(db_path.to_str().unwrap()).unwrap();

    let params = RunParameters {
        roots: vec!["D:\\Photos".to_owned()],
        ignore_patterns: vec!["**/node_modules/**".to_owned()],
        directory_similarity_threshold_millis: 500,
        repeat_cache_policy: Default::default(),
        cloud_policy: Default::default(),
        manual_location_exclusions: Vec::new(),
        registered_cloud_locations: Vec::new(),
        cloud_detection_status: Default::default(),
    };
    let session_id = db
        .create_session("Photos", &params.roots, &params.ignore_patterns)
        .unwrap();
    let run_id = db.create_scan_run(session_id, &params, "test").unwrap();
    db.start_scan_run(run_id).unwrap();
    db.complete_scan_run(run_id, 2, 2048, 2, 1, 0, 1024, 0)
        .unwrap();

    let member = |canonical_path: &str, relative_path: &str, parent_dir: &str| ScannedFile {
        id: 0,
        run_id,
        root_path: "D:\\Photos".to_owned(),
        canonical_path: canonical_path.to_owned(),
        relative_path: relative_path.to_owned(),
        file_name: "a.jpg".to_owned(),
        parent_dir: parent_dir.to_owned(),
        drive_letter: "D:".to_owned(),
        file_size: 1024,
        last_modified: 1_786_795_200_000_000_000,
        partial_hash: Some(1),
        content_hash: Some(42),
        file_identity: None,
        warning_message: None,
        marked_deleted: false,
    };
    db.insert_scanned_files(&[
        member(r"\\?\D:\Photos\a.jpg", "a.jpg", r"\\?\D:\Photos"),
        member(
            r"\\?\D:\Photos\Copy\a.jpg",
            "Copy\\a.jpg",
            r"\\?\D:\Photos\Copy",
        ),
    ])
    .unwrap();
    db.insert_duplicate_groups(
        run_id,
        &[(
            42,
            1024,
            vec![
                r"\\?\D:\Photos\a.jpg".to_owned(),
                r"\\?\D:\Photos\Copy\a.jpg".to_owned(),
            ],
        )],
    )
    .unwrap();

    dir
}

pub fn cli(dir: &TempDir, args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_super-duper-cli"))
        .args(args)
        .current_dir(dir.path())
        .output()
        .unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}
