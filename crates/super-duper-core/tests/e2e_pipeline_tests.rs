use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use tempfile::tempdir;

use super_duper_core::analysis::{deletion_plan, dir_fingerprint, dir_similarity};
use super_duper_core::storage::Database;
use super_duper_core::telemetry::{
    CounterKind, ProgressObservation, ProgressReducer, StatusDatabase, METRICS_CONTRACT_VERSION,
};
use super_duper_core::{AppConfig, ProgressReporter, ScanEngine, SilentReporter};

#[derive(Default)]
struct RecordingProgress(std::sync::Mutex<Vec<ProgressObservation>>);

impl ProgressReporter for RecordingProgress {
    fn on_progress_observation(&self, observation: &ProgressObservation) {
        self.0.lock().unwrap().push(observation.clone());
    }
}

fn assert_last_progress_matches_durable_counters(
    status: &StatusDatabase,
    progress: &RecordingProgress,
) {
    let observations = progress.0.lock().unwrap();
    let final_observation = observations.last().expect("a final live observation");
    let status_run_id: i64 = status
        .connection()
        .query_row("SELECT id FROM status_run", [], |row| row.get(0))
        .unwrap();
    for kind in CounterKind::ALL {
        let durable: i64 = status
            .connection()
            .query_row(
                "SELECT value FROM status_counter
                 WHERE run_id = ?1 AND phase = 'overall' AND metric = ?2",
                rusqlite::params![status_run_id, kind.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            durable,
            final_observation.counters.value(kind) as i64,
            "live/durable terminal counter mismatch for {}",
            kind.as_str()
        );
    }
}

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
    let status_path = db_dir.path().join("test_e2e_status.db");

    let config = AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    };

    let engine = ScanEngine::new(config)
        .with_db_path(db_path.to_str().unwrap())
        .with_status_db_path(status_path.to_str().unwrap());
    let progress = RecordingProgress::default();
    let result = engine.scan(&progress).unwrap();

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
    let groups = db.get_duplicate_groups(result.run_id, 0, 100).unwrap();
    assert_eq!(groups.len(), 2);

    // Each group should have 2 files
    for group in &groups {
        assert_eq!(
            group.file_count, 2,
            "Each duplicate group should have 2 files"
        );
        let files = db.get_files_in_group(group.id).unwrap();
        assert_eq!(files.len(), 2);
    }

    // Verify the immutable run was recorded separately from its reusable session.
    let sessions: Vec<(i64, String)> = db
        .connection()
        .prepare("SELECT id, status FROM scan_run")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].1, "completed");
    assert_eq!(result.total_files_scanned, 6);
    assert_eq!(result.files_hashed, 6);
    assert_eq!(result.duplicate_files, 4);
    let run = db.get_scan_run(result.run_id).unwrap();
    assert_eq!(run.files_discovered, 6);
    assert_eq!(run.files_hashed, 6);
    assert_eq!(run.duplicate_file_groups, 2);
    assert_eq!(run.bytes_discovered as u64, result.total_bytes_discovered);

    let status = StatusDatabase::open_connection(status_path.to_str().unwrap()).unwrap();
    let (state, contract, last_sequence, replay_flushes): (String, i64, i64, i64) = status
        .connection()
        .query_row(
            "SELECT state, metrics_contract_version, last_sequence,
                    (SELECT COUNT(*) FROM status_flush WHERE run_id = status_run.id)
             FROM status_run WHERE product_run_id = ?1",
            [result.run_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(state, "completed");
    assert_eq!(contract, i64::from(METRICS_CONTRACT_VERSION));
    assert_eq!(last_sequence, 10);
    assert_eq!(replay_flushes, 0);
    let metric = |name: &str| -> i64 {
        status
            .connection()
            .query_row(
                "SELECT value FROM status_counter
                 WHERE run_id = (SELECT id FROM status_run WHERE product_run_id = ?1)
                   AND phase = 'overall' AND metric = ?2",
                rusqlite::params![result.run_id, name],
                |row| row.get(0),
            )
            .unwrap()
    };
    let observations = progress.0.lock().unwrap();
    assert!(
        observations.len() >= 10,
        "expected phase-boundary and incremental progress observations"
    );
    let mut reducer = ProgressReducer::new();
    for observation in observations.iter().cloned() {
        reducer.observe(observation).unwrap();
    }
    let final_observation = observations.last().unwrap();
    assert!(final_observation.candidate_totals_known);
    assert!(final_observation.final_results_complete);
    assert_eq!(
        final_observation.logical.hash_pipeline_resolved_files,
        final_observation.counters.candidate_files
    );
    assert_eq!(
        final_observation.logical.hash_pipeline_resolved_bytes,
        final_observation.counters.candidate_bytes
    );
    for kind in CounterKind::ALL {
        assert_eq!(
            metric(kind.as_str()),
            final_observation.counters.value(kind) as i64,
            "live/durable terminal counter mismatch for {}",
            kind.as_str()
        );
    }
    assert_eq!(metric("candidate_files"), 6);
    assert_eq!(metric("duplicate_candidate_files"), 6);
    assert_eq!(metric("partial_hashes_attempted"), 6);
    assert_eq!(metric("confirmed_duplicate_groups"), 2);
    let incomplete_phase_count: i64 = status
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM status_phase
             WHERE run_id = (SELECT id FROM status_run WHERE product_run_id = ?1)
               AND state <> 'completed'",
            [result.run_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(incomplete_phase_count, 0);
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

    let engine = ScanEngine::new(config).with_db_path(db_path.to_str().unwrap());
    let result = engine.scan(&SilentReporter).unwrap();

    // With folder_c ignored, only 1 duplicate group (shared.txt)
    assert_eq!(
        result.duplicate_groups, 1,
        "Expected 1 duplicate group with folder_c ignored, got {}",
        result.duplicate_groups
    );
}

#[test]
fn typed_discovery_progress_advances_before_candidate_totals_are_known() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("typed_discovery");
    fs::create_dir(&root).unwrap();
    for index in 1..=300 {
        fs::write(root.join(format!("{index}.bin")), vec![7_u8; index]).unwrap();
    }
    let product_path = tmp.path().join("typed-discovery.db");
    let progress = RecordingProgress::default();
    ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: Vec::new(),
    })
    .with_db_path(product_path.to_str().unwrap())
    .scan(&progress)
    .unwrap();

    let observations = progress.0.lock().unwrap();
    assert!(observations.iter().any(|observation| {
        observation.phase == super_duper_core::telemetry::TelemetryPhase::Discovering
            && !observation.candidate_totals_known
            && observation.counters.discovered_files >= 256
    }));
}

#[test]
fn test_scan_cancellation() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("scan_cancel");
    create_test_tree(&root);

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test_cancel.db");
    let status_path = db_dir.path().join("test_cancel_status.db");
    let db_path_str = db_path.to_str().unwrap().to_string();

    let config = AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    };

    let engine = ScanEngine::new(config)
        .with_db_path(&db_path_str)
        .with_status_db_path(status_path.to_str().unwrap());

    let cancel_token = engine.cancel_token();
    struct CancelOnStart<'a> {
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        progress: &'a RecordingProgress,
    }
    impl ProgressReporter for CancelOnStart<'_> {
        fn on_scan_start(&self) {
            self.cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }

        fn on_progress_observation(&self, observation: &ProgressObservation) {
            self.progress.on_progress_observation(observation);
        }
    }

    let progress = RecordingProgress::default();
    let result = engine.scan(&CancelOnStart {
        cancel: cancel_token,
        progress: &progress,
    });
    assert!(matches!(result, Err(super_duper_core::Error::Cancelled)));
    let db = Database::open(&db_path_str).unwrap();
    let (_, run_count) = db.list_runs(0, 10).unwrap();
    assert_eq!(run_count, 1);
    assert_eq!(db.list_runs(0, 10).unwrap().0[0].status, "cancelled");
    let status = StatusDatabase::open_connection(status_path.to_str().unwrap()).unwrap();
    let state: String = status
        .connection()
        .query_row("SELECT state FROM status_run", [], |row| row.get(0))
        .unwrap();
    assert_eq!(state, "cancelled");
    assert_last_progress_matches_durable_counters(&status, &progress);
}

#[cfg(target_os = "windows")]
#[test]
fn telemetry_heartbeat_samples_during_a_phase_without_progress_callbacks() {
    struct PauseAtDiscoveryStart {
        status_path: PathBuf,
        observed_samples: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }
    impl ProgressReporter for PauseAtDiscoveryStart {
        fn on_scan_start(&self) {
            std::thread::sleep(std::time::Duration::from_millis(120));
            let connection = rusqlite::Connection::open(&self.status_path).unwrap();
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM status_host_sample", [], |row| {
                    row.get(0)
                })
                .unwrap();
            self.observed_samples
                .store(count as usize, std::sync::atomic::Ordering::Relaxed);
        }
    }

    let tmp = tempdir().unwrap();
    let root = tmp.path().join("heartbeat");
    create_test_tree(&root);
    let product_path = tmp.path().join("heartbeat-product.db");
    let status_path = tmp.path().join("heartbeat-status.db");
    let observed_samples = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: Vec::new(),
    })
    .with_db_path(product_path.to_str().unwrap())
    .with_status_db_path(status_path.to_str().unwrap())
    .with_status_sampling(std::time::Duration::from_millis(10), 100)
    .scan(&PauseAtDiscoveryStart {
        status_path: status_path.clone(),
        observed_samples: observed_samples.clone(),
    })
    .unwrap();

    let status = StatusDatabase::open_connection(status_path.to_str().unwrap()).unwrap();
    let (status_run_id, last_sequence): (i64, i64) = status
        .connection()
        .query_row(
            "SELECT id, last_sequence FROM status_run WHERE product_run_id = ?1",
            [result.run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let host_samples: i64 = status
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM status_host_sample WHERE run_id = ?1",
            [status_run_id],
            |row| row.get(0),
        )
        .unwrap();
    let device_samples: i64 = status
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM status_device_sample WHERE run_id = ?1",
            [status_run_id],
            |row| row.get(0),
        )
        .unwrap();
    let (descriptors, unavailable): (i64, i64) = status
        .connection()
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM status_device WHERE run_id = ?1),
                (SELECT value FROM status_counter
                 WHERE run_id = ?1 AND phase = 'overall' AND metric = 'unavailable_counters')",
            [status_run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(
        last_sequence >= 14,
        "last: {last_sequence}, host: {host_samples}, device: {device_samples}, descriptors: {descriptors}, unavailable: {unavailable}"
    );
    assert!(
        observed_samples.load(std::sync::atomic::Ordering::Relaxed) >= 4,
        "samples observed during phase: {}",
        observed_samples.load(std::sync::atomic::Ordering::Relaxed)
    );
    assert!(
        host_samples >= 4,
        "last: {last_sequence}, host: {host_samples}, device: {device_samples}, descriptors: {descriptors}, unavailable: {unavailable}"
    );
    assert_eq!(device_samples, host_samples);
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "SOP1 operator overhead profile; run optimized with --ignored --nocapture"]
fn telemetry_observer_overhead_profile() {
    fn process_cpu_nanos() -> u64 {
        use winapi::shared::minwindef::FILETIME;
        use winapi::um::processthreadsapi::{GetCurrentProcess, GetProcessTimes};
        let mut creation: FILETIME = unsafe { std::mem::zeroed() };
        let mut exit: FILETIME = unsafe { std::mem::zeroed() };
        let mut kernel: FILETIME = unsafe { std::mem::zeroed() };
        let mut user: FILETIME = unsafe { std::mem::zeroed() };
        let result = unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        };
        assert_ne!(result, 0);
        let value =
            |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
        value(kernel)
            .saturating_add(value(user))
            .saturating_mul(100)
    }

    fn median(values: &mut [u64]) -> u64 {
        values.sort_unstable();
        values[values.len() / 2]
    }

    fn overhead_basis_points(instrumented: u64, baseline: u64) -> i64 {
        if baseline == 0 {
            return 0;
        }
        ((instrumented as i128 - baseline as i128) * 10_000 / baseline as i128) as i64
    }

    let temp = tempdir().unwrap();
    let root = temp.path().join("overhead-fixture");
    fs::create_dir(&root).unwrap();
    let content = (0..12_001)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    for index in 1..=12_000 {
        fs::write(root.join(format!("item-{index:05}.bin")), &content[..index]).unwrap();
    }
    let config = AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: Vec::new(),
    };
    ScanEngine::new(config.clone())
        .with_db_path(temp.path().join("warmup.db").to_str().unwrap())
        .scan(&SilentReporter)
        .unwrap();

    let mut baseline_wall = Vec::new();
    let mut baseline_cpu = Vec::new();
    let mut instrumented_wall = Vec::new();
    let mut instrumented_cpu = Vec::new();
    for index in 0..6 {
        let instrumented = index % 2 == 1;
        let product = temp.path().join(format!("profile-{index}.db"));
        let status = temp.path().join(format!("profile-{index}-status.db"));
        let mut engine = ScanEngine::new(config.clone()).with_db_path(product.to_str().unwrap());
        if instrumented {
            engine = engine.with_status_db_path(status.to_str().unwrap());
        }
        let cpu_start = process_cpu_nanos();
        let wall_start = std::time::Instant::now();
        let result = engine.scan(&SilentReporter).unwrap();
        let wall = wall_start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let cpu = process_cpu_nanos().saturating_sub(cpu_start);
        assert_eq!(result.total_files_scanned, 12_000);
        if instrumented {
            instrumented_wall.push(wall);
            instrumented_cpu.push(cpu);
        } else {
            baseline_wall.push(wall);
            baseline_cpu.push(cpu);
        }
    }
    let baseline_wall = median(&mut baseline_wall);
    let baseline_cpu = median(&mut baseline_cpu);
    let instrumented_wall = median(&mut instrumented_wall);
    let instrumented_cpu = median(&mut instrumented_cpu);
    let wall_overhead = overhead_basis_points(instrumented_wall, baseline_wall);
    let cpu_overhead = overhead_basis_points(instrumented_cpu, baseline_cpu);
    println!(
        "{{\"fixtureFiles\":12000,\"runsPerMode\":3,\"baselineWallNanos\":{baseline_wall},\"instrumentedWallNanos\":{instrumented_wall},\"wallOverheadBasisPoints\":{wall_overhead},\"baselineCpuNanos\":{baseline_cpu},\"instrumentedCpuNanos\":{instrumented_cpu},\"cpuOverheadBasisPoints\":{cpu_overhead}}}"
    );
    assert!(
        wall_overhead <= 100,
        "wall overhead exceeded 1%: {wall_overhead} bp"
    );
    assert!(
        cpu_overhead <= 100,
        "CPU overhead exceeded 1%: {cpu_overhead} bp"
    );
}

#[test]
fn test_pipeline_failure_is_persisted_as_failed() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("scan_failure");
    create_test_tree(&root);
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("test_failure.db");
    let status_path = db_dir.path().join("test_failure_status.db");

    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    db.connection()
        .execute_batch(
            "CREATE TRIGGER force_result_failure BEFORE INSERT ON scanned_file
             BEGIN SELECT RAISE(ABORT, 'forced persistence failure'); END;",
        )
        .unwrap();
    drop(db);

    let progress = RecordingProgress::default();
    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    })
    .with_db_path(db_path.to_str().unwrap())
    .with_status_db_path(status_path.to_str().unwrap())
    .scan(&progress);
    assert!(result.is_err());

    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    let run = db.list_runs(0, 10).unwrap().0.remove(0);
    assert_eq!(run.status, "failed");
    assert!(run.completed_at.is_some());
    assert!(run
        .error_message
        .as_deref()
        .unwrap_or_default()
        .contains("forced persistence failure"));
    let status = StatusDatabase::open_connection(status_path.to_str().unwrap()).unwrap();
    let (state, code): (String, Option<String>) = status
        .connection()
        .query_row("SELECT state, error_code FROM status_run", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(state, "failed");
    assert_eq!(code.as_deref(), Some("scan_failed"));
    assert_last_progress_matches_durable_counters(&status, &progress);
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
    let engine = ScanEngine::new(config).with_db_path(db_path.to_str().unwrap());
    let scan_result = engine.scan(&SilentReporter).unwrap();
    assert_eq!(scan_result.duplicate_groups, 2);

    // Phase 2: Directory fingerprinting
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    let fp_count = dir_fingerprint::build_directory_fingerprints(&db, scan_result.run_id).unwrap();
    assert!(
        fp_count > 0,
        "Expected at least 1 directory fingerprint, got {}",
        fp_count
    );

    // Verify directory nodes were created
    let root_nodes = db
        .get_directory_children(scan_result.run_id, None, 0, 1000)
        .unwrap();
    assert!(
        !root_nodes.is_empty(),
        "Expected at least one root directory node"
    );

    // Phase 3: Directory similarity
    let sim_count =
        dir_similarity::compute_directory_similarity(&db, scan_result.run_id, 0.1).unwrap();
    // folder_a and folder_b both contain shared.txt, so there should be some similarity
    assert!(
        sim_count > 0,
        "Expected at least 1 similarity pair, got {}",
        sim_count
    );

    let similarities = db
        .get_similar_directories(scan_result.run_id, 0.1, 0, 100)
        .unwrap();
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
    let engine = ScanEngine::new(config).with_db_path(db_path.to_str().unwrap());
    let scan_result = engine.scan(&SilentReporter).unwrap();
    assert_eq!(scan_result.duplicate_groups, 2);

    // Auto-mark duplicates for deletion
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    deletion_plan::auto_mark_duplicates(&db, scan_result.run_id, None).unwrap();

    // Check deletion plan
    let (count, bytes) = db.get_deletion_plan_summary().unwrap();
    assert_eq!(
        count, 2,
        "Expected 2 files marked (one per dup group), got {}",
        count
    );
    assert!(bytes > 0, "Expected wasted bytes > 0");

    // Execute deletion
    let (deleted, errors) = deletion_plan::execute_deletion_plan(&db, false).unwrap();
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
    let engine = ScanEngine::new(config.clone()).with_db_path(db_path.to_str().unwrap());
    let result1 = engine.scan(&SilentReporter).unwrap();
    assert_eq!(result1.duplicate_groups, 2);

    // Delete one duplicate manually
    let folder_b_shared = root.join("folder_b").join("shared.txt");
    fs::remove_file(&folder_b_shared).unwrap();

    // Truncate and rescan
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    db.truncate_all().unwrap();

    let engine2 = ScanEngine::new(config).with_db_path(db_path.to_str().unwrap());
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
    let engine = ScanEngine::new(config.clone()).with_db_path(db_path.to_str().unwrap());
    let result1 = engine.scan(&SilentReporter).unwrap();
    assert_eq!(result1.duplicate_groups, 2);

    // Second scan — same paths, no truncate, must NOT crash and must produce same results
    let engine2 = ScanEngine::new(config).with_db_path(db_path.to_str().unwrap());
    let result2 = engine2.scan(&SilentReporter).unwrap();
    assert_eq!(
        result2.duplicate_groups, 2,
        "Second scan of same paths should produce same group count"
    );

    // Both scans reuse one definition but preserve two immutable executions.
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    let session_count: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM scan_session", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        session_count, 1,
        "Idempotent rescan should reuse the same session"
    );
    let run_count: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM scan_run", [], |row| row.get(0))
        .unwrap();
    assert_eq!(run_count, 2);
    assert_ne!(result1.run_id, result2.run_id);
}

#[test]
fn hard_links_are_snapshotted_but_not_counted_as_recoverable_copies() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("hard-links");
    fs::create_dir(&root).unwrap();
    let original = root.join("original.bin");
    let linked = root.join("linked.bin");
    fs::write(&original, b"one physical allocation").unwrap();
    fs::hard_link(&original, &linked).unwrap();
    let db_path = tmp.path().join("hard-links.db");

    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    })
    .with_db_path(db_path.to_str().unwrap())
    .scan(&SilentReporter)
    .unwrap();

    assert_eq!(result.total_files_scanned, 2);
    assert_eq!(result.duplicate_groups, 0);
    let db = Database::open_connection(db_path.to_str().unwrap()).unwrap();
    let identities = db
        .get_scanned_files(result.run_id)
        .unwrap()
        .into_iter()
        .map(|file| file.file_identity)
        .collect::<Vec<_>>();
    assert!(identities.iter().all(Option::is_some));
    assert_eq!(identities[0], identities[1]);
}

#[test]
fn files_changed_or_removed_after_discovery_become_warnings_not_false_results() {
    struct MutateAfterDiscovery {
        change: PathBuf,
        remove: PathBuf,
    }

    impl ProgressReporter for MutateAfterDiscovery {
        fn on_scan_complete(&self, _total_files: usize, _duration_secs: f64) {
            fs::write(&self.change, b"changed-data").unwrap();
            fs::remove_file(&self.remove).unwrap();
        }
    }

    let tmp = tempdir().unwrap();
    let root = tmp.path().join("volatile");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("stable-a.bin"), b"stable").unwrap();
    fs::write(root.join("stable-b.bin"), b"stable").unwrap();
    let changed = root.join("changed.bin");
    let removed = root.join("removed.bin");
    fs::write(&changed, b"initial-data").unwrap();
    fs::write(&removed, b"initial-data").unwrap();
    let db_path = tmp.path().join("volatile.db");

    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    })
    .with_db_path(db_path.to_str().unwrap())
    .scan(&MutateAfterDiscovery {
        change: changed,
        remove: removed,
    })
    .unwrap();

    assert_eq!(result.duplicate_groups, 1);
    assert!(result.warning_count >= 2, "result: {result:#?}");
    let db = Database::open_connection(db_path.to_str().unwrap()).unwrap();
    let warning_snapshots = db
        .get_scanned_files(result.run_id)
        .unwrap()
        .into_iter()
        .filter(|file| file.warning_message.is_some())
        .collect::<Vec<_>>();
    assert!(warning_snapshots.len() >= 2);
    assert!(warning_snapshots
        .iter()
        .all(|file| file.content_hash.is_none()));
    let grouped_names = db
        .get_duplicate_groups(result.run_id, 0, 100)
        .unwrap()
        .into_iter()
        .flat_map(|group| db.get_files_in_group(group.id).unwrap())
        .map(|file| file.file_name)
        .collect::<Vec<_>>();
    assert!(!grouped_names.iter().any(|name| name == "changed.bin"));
    assert!(!grouped_names.iter().any(|name| name == "removed.bin"));
}

#[cfg(windows)]
#[test]
fn windows_long_paths_scan_without_truncation() {
    let tmp = tempdir().unwrap();
    let mut root = tmp.path().join("long-path");
    while root.as_os_str().len() < 280 {
        root.push("segment-0123456789abcdef");
    }
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("copy-a.bin"), b"long path duplicate").unwrap();
    fs::write(root.join("copy-b.bin"), b"long path duplicate").unwrap();
    let db_path = tmp.path().join("long-path.db");

    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    })
    .with_db_path(db_path.to_str().unwrap())
    .scan(&SilentReporter)
    .unwrap();

    assert_eq!(result.total_files_scanned, 2);
    assert_eq!(result.duplicate_groups, 1);
}

#[cfg(windows)]
#[test]
fn windows_sharing_violations_are_recoverable_scan_warnings() {
    use std::os::windows::fs::OpenOptionsExt;

    let tmp = tempdir().unwrap();
    let root = tmp.path().join("locked-file");
    fs::create_dir(&root).unwrap();
    let locked_path = root.join("locked.bin");
    fs::write(&locked_path, b"locked content").unwrap();
    let _exclusive = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&locked_path)
        .unwrap();
    let db_path = tmp.path().join("locked-file.db");

    let result = ScanEngine::new(AppConfig {
        root_paths: vec![root.to_string_lossy().into_owned()],
        ignore_patterns: vec![],
    })
    .with_db_path(db_path.to_str().unwrap())
    .scan(&SilentReporter)
    .unwrap();

    assert!(result.warning_count >= 1);
    let db = Database::open_connection(db_path.to_str().unwrap()).unwrap();
    assert_eq!(db.get_scan_run(result.run_id).unwrap().status, "completed");
}
