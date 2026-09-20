//! A scan process must use one hash-cache store. RocksDB locks its directory even against a second
//! open from the same process, so any parallel handle degrades one of the two users.
//!
//! This file is its own test binary (its own process) so no other test can open the store first.

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use super_duper_core::storage::Database;
use super_duper_core::telemetry::ProgressObservation;
use super_duper_core::{AppConfig, ProgressReporter, ScanEngine};
use tempfile::tempdir;

#[derive(Default)]
struct LastObservation(Mutex<Option<ProgressObservation>>);

impl ProgressReporter for LastObservation {
    fn on_progress_observation(&self, observation: &ProgressObservation) {
        *self.0.lock().unwrap() = Some(observation.clone());
    }
}

fn warning_codes(db_path: &Path, run_id: i64) -> Vec<(String, String, i64)> {
    let db = Database::open_connection(db_path.to_str().unwrap()).unwrap();
    let mut statement = db
        .connection()
        .prepare(
            "SELECT code, message, occurrence_count FROM run_warning_aggregate
             WHERE run_id = ?1 ORDER BY id",
        )
        .unwrap();

    statement
        .query_map(rusqlite::params![run_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

#[test]
fn exact_folder_verification_shares_the_engine_hash_cache_across_repeated_runs() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("root");
    // Same name and size make the folders structural candidates; different leading bytes make the
    // partial hashes unique, so the hash pipeline never assigns a content hash and exact-folder
    // verification must hash both files on demand.
    for (folder, byte) in [("left", 1u8), ("right", 2u8)] {
        fs::create_dir_all(root.join(folder)).unwrap();
        fs::write(root.join(folder).join("item.bin"), vec![byte; 4096]).unwrap();
    }
    let cache_path = temp.path().join("content_hash_cache.db");
    // Any process-global cache that reads the environment resolves to the engine's store.
    // SAFETY: this file is its own test binary and holds exactly one test, so nothing else in
    // this process reads or writes the environment while this runs.
    unsafe { std::env::set_var("HASH_CACHE_PATH", &cache_path) };
    let db_path = temp.path().join("super_duper.db");

    let scan = || {
        let progress = LastObservation::default();
        let result = ScanEngine::new(AppConfig {
            root_paths: vec![root.to_string_lossy().into_owned()],
            ignore_patterns: vec![],
            ..Default::default()
        })
        .with_db_path(db_path.to_str().unwrap())
        .with_hash_cache_path(&cache_path)
        .with_verified_repeat_cache_reuse(true)
        .scan(&progress)
        .unwrap();
        let counters = progress
            .0
            .into_inner()
            .unwrap()
            .expect("a terminal progress observation")
            .counters;
        (result, counters)
    };

    let (first, _) = scan();
    assert_eq!(
        warning_codes(&db_path, first.run_id),
        Vec::<(String, String, i64)>::new(),
        "first run must verify exact folders without a hash-cache warning"
    );
    assert_eq!(first.warning_count, 0);

    let (second, counters) = scan();
    assert_eq!(
        warning_codes(&db_path, second.run_id),
        Vec::<(String, String, i64)>::new(),
        "second run in the same process must still reach the hash cache"
    );
    assert_eq!(second.warning_count, 0);
    assert_eq!(
        counters.partial_hash_cache_hits, 2,
        "second run must reuse partial hashes stored by the first run"
    );
}
