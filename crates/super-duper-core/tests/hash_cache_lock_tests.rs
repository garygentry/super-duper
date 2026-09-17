//! Standalone exact-folder analysis must not take a process-lifetime lock on the hash cache that a
//! later engine run in the same long-lived process needs.
//!
//! This file is its own test binary (its own process) so no other test can open the store first.

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use super_duper_core::analysis::exact_folders;
use super_duper_core::storage::models::{RunParameters, ScannedFile};
use super_duper_core::storage::Database;
use super_duper_core::telemetry::ProgressObservation;
use super_duper_core::{AppConfig, ProgressReporter, ScanEngine, SilentReporter};
use tempfile::tempdir;

#[derive(Default)]
struct LastObservation(Mutex<Option<ProgressObservation>>);

impl ProgressReporter for LastObservation {
    fn on_progress_observation(&self, observation: &ProgressObservation) {
        *self.0.lock().unwrap() = Some(observation.clone());
    }
}

fn unhashed_file(run_id: i64, root: &Path, folder: &str, byte: u8) -> ScannedFile {
    let path = root.join(folder).join("item.bin");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, vec![byte; 4096]).unwrap();
    ScannedFile {
        id: 0,
        run_id,
        root_path: root.to_string_lossy().into_owned(),
        canonical_path: path.to_string_lossy().into_owned(),
        relative_path: format!("{folder}/item.bin"),
        file_name: "item.bin".into(),
        parent_dir: path.parent().unwrap().to_string_lossy().into_owned(),
        drive_letter: String::new(),
        file_size: 4096,
        last_modified: 0,
        partial_hash: None,
        content_hash: None,
        file_identity: None,
        warning_message: None,
        marked_deleted: false,
    }
}

#[test]
fn standalone_exact_folder_analysis_does_not_lock_out_a_later_engine_run() {
    let temp = tempdir().unwrap();
    let cache_path = temp.path().join("content_hash_cache.db");
    std::env::set_var("HASH_CACHE_PATH", &cache_path);

    let fixture_root = temp.path().join("fixture");
    let fixture = Database::open_in_memory().unwrap();
    let root_text = fixture_root.to_string_lossy().into_owned();
    let session = fixture
        .create_session("standalone", std::slice::from_ref(&root_text), &[])
        .unwrap();
    let run = fixture
        .create_scan_run(
            session,
            &RunParameters {
                roots: vec![root_text],
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
    fixture.start_scan_run(run).unwrap();
    fixture
        .insert_scanned_files(&[
            unhashed_file(run, &fixture_root, "left", 3),
            unhashed_file(run, &fixture_root, "right", 3),
        ])
        .unwrap();
    let standalone = exact_folders::analyze_exact_folders_cancellable(
        &fixture,
        run,
        &AtomicBool::new(false),
        &SilentReporter,
    )
    .unwrap();
    assert_eq!(standalone.visible_groups, 1);
    assert_eq!(standalone.warning_count, 0);

    let scan_root = temp.path().join("scan");
    for (folder, byte) in [("left", 1u8), ("right", 2u8)] {
        fs::create_dir_all(scan_root.join(folder)).unwrap();
        fs::write(scan_root.join(folder).join("item.bin"), vec![byte; 4096]).unwrap();
    }
    let db_path = temp.path().join("super_duper.db");
    let scan = || {
        let progress = LastObservation::default();
        let result = ScanEngine::new(AppConfig {
            root_paths: vec![scan_root.to_string_lossy().into_owned()],
            ignore_patterns: vec![],
        })
        .with_db_path(db_path.to_str().unwrap())
        .with_hash_cache_path(&cache_path)
        .with_verified_repeat_cache_reuse(true)
        .scan(&progress)
        .unwrap();
        let counters = progress.0.into_inner().unwrap().unwrap().counters;
        (result, counters)
    };

    let (first, _) = scan();
    assert_eq!(first.warning_count, 0, "repeat cache must open: {first:#?}");
    let (second, counters) = scan();
    assert_eq!(second.warning_count, 0);
    assert_eq!(counters.partial_hash_cache_hits, 2);
}
