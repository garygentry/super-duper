use rusqlite::params;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::hash::Hasher;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use super_duper_core::analysis::exact_folders;
use super_duper_core::storage::models::{
    DuplicateFolderGroupFilter, DuplicateFolderGroupPageQuery, DuplicateFolderGroupSortField,
    RunParameters, ScannedFile, SortDirection,
};
use super_duper_core::storage::Database;
use super_duper_core::SilentReporter;
use tempfile::TempDir;
use twox_hash::XxHash64;

#[cfg(windows)]
use winapi::um::processthreadsapi::GetCurrentProcess;
#[cfg(windows)]
use winapi::um::psapi::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};

const FILE_COUNT: usize = 3_550_000;
const BULK_FILE_COUNT: usize = FILE_COUNT - 8;
const BULK_DIRECTORY_COUNT: usize = 632_974;
const EXPECTED_DIRECTORY_COUNT: usize = 633_000;
const MAXIMUM_FOLDER_ANALYSIS_SECONDS: u64 = 30 * 60;
const MAXIMUM_PRIVATE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[test]
#[ignore = "SOP10f first-result Release generated-store profile"]
fn sop10_release_scale_profile() {
    #[allow(clippy::assertions_on_constants)]
    {
        assert!(!cfg!(debug_assertions), "SOP10f must run in Release");
    }
    let evidence_path = required_new_path("SOP10_SCALE_EVIDENCE_PATH");
    let journal_path = required_new_path("SOP10_SCALE_JOURNAL_PATH");
    let build_commit =
        std::env::var("SOP10_SCALE_BUILD_COMMIT").expect("SOP10_SCALE_BUILD_COMMIT is required");
    append_new_journal(
        &journal_path,
        &json!({"event":"reserved","buildCommit":build_commit}),
        true,
    );

    let temp = TempDir::new().unwrap();
    let database_path = temp.path().join("sop10-scale.db");
    let db = Database::open(database_path.to_str().unwrap()).unwrap();
    let run_id = create_run(&db, temp.path());
    append_new_journal(
        &journal_path,
        &json!({"event":"fixture_started","fileCount":FILE_COUNT,"directoryCount":EXPECTED_DIRECTORY_COUNT}),
        false,
    );
    generate_bulk_rows(&db, run_id);
    insert_correctness_rows(&db, run_id, temp.path());
    assert_eq!(scanned_file_count(&db, run_id), FILE_COUNT);
    append_new_journal(
        &journal_path,
        &json!({"event":"fixture_ready","databaseBytes":file_bytes(&database_path)}),
        false,
    );

    let sampler_running = Arc::new(AtomicBool::new(true));
    let peak_private_bytes = Arc::new(AtomicU64::new(current_process_private_bytes()));
    let sampler = {
        let running = sampler_running.clone();
        let peak = peak_private_bytes.clone();
        std::thread::spawn(move || {
            while running.load(Ordering::Acquire) {
                peak.fetch_max(current_process_private_bytes(), Ordering::AcqRel);
                std::thread::sleep(Duration::from_millis(25));
            }
            peak.fetch_max(current_process_private_bytes(), Ordering::AcqRel);
        })
    };
    append_new_journal(&journal_path, &json!({"event":"analysis_started"}), false);
    let started = Instant::now();
    let analysis = exact_folders::analyze_exact_folders_cancellable(
        &db,
        run_id,
        &AtomicBool::new(false),
        &SilentReporter,
    );
    let elapsed = started.elapsed();
    sampler_running.store(false, Ordering::Release);
    sampler.join().unwrap();
    let peak_private_bytes = peak_private_bytes.load(Ordering::Acquire);

    let analysis_error = analysis.as_ref().err().map(ToString::to_string);
    let analysis = analysis.ok();
    let directory_count = directory_count(&db, run_id);
    let similarity_pairs = similarity_pair_count(&db, run_id);
    let result_digest = exact_result_digest(&db, run_id, temp.path());
    let cancellation_passed = cancellation_preflight();
    let checks = json!({
        "analysisCompleted": analysis.is_some(),
        "fileCountExact": scanned_file_count(&db, run_id) == FILE_COUNT,
        "directoryCountExact": directory_count == EXPECTED_DIRECTORY_COUNT,
        "orderedScannedFilePassBound": analysis.as_ref().is_some_and(|value| value.scanned_file_passes <= 3),
        "zeroJaccardPairs": similarity_pairs == 0,
        "exactVisibleGroupCount": analysis.as_ref().is_some_and(|value| value.visible_groups == 1),
        "exactRetainedGroupCount": analysis.as_ref().is_some_and(|value| value.retained_groups == 9),
        "warningTruth": analysis.as_ref().is_some_and(|value| value.warning_count == 1),
        "cancellationPreflight": cancellation_passed,
        "elapsedUnderThirtyMinutes": elapsed < Duration::from_secs(MAXIMUM_FOLDER_ANALYSIS_SECONDS),
        "peakPrivateBelowTwoGiB": peak_private_bytes < MAXIMUM_PRIVATE_BYTES,
    });
    let valid = checks
        .as_object()
        .unwrap()
        .values()
        .all(|value| value == &json!(true));
    let evidence = json!({
        "schemaVersion": 1,
        "gate": "SOP10f-release-scale-acceptance",
        "profile": "sop10_release_scale_profile",
        "status": if valid { "valid" } else { "invalid" },
        "configuration": "Release",
        "buildCommit": build_commit,
        "firstResultRetained": true,
        "favorableRerunAllowed": false,
        "input": {
            "scannedFileCount": FILE_COUNT,
            "expectedDirectoryCount": EXPECTED_DIRECTORY_COUNT,
            "bulkDirectoryCount": BULK_DIRECTORY_COUNT,
            "includesDeepWideTrees": true,
            "includesStructuralCollisions": true,
            "includesExactDuplicateFolders": true,
            "includesNonduplicates": true,
            "includesHardLinkIdentityAliases": true,
            "includesWarning": true,
        },
        "analysis": {
            "elapsedMilliseconds": elapsed.as_millis() as u64,
            "peakPrivateBytes": peak_private_bytes,
            "directoryCount": directory_count,
            "scannedFilePasses": analysis.as_ref().map(|value| value.scanned_file_passes),
            "largestPersistenceBatch": analysis.as_ref().map(|value| value.largest_persistence_batch),
            "visibleGroups": analysis.as_ref().map(|value| value.visible_groups),
            "retainedGroups": analysis.as_ref().map(|value| value.retained_groups),
            "warningCount": analysis.as_ref().map(|value| value.warning_count),
            "similarityPairs": similarity_pairs,
            "resultDigestXxHash64": result_digest,
            "error": analysis_error,
        },
        "cacheAcceptance": {
            "qualifiedHitFixturesRunSeparatelyByNamedVerifier": true,
            "legacyCapScaleFixtureEntries": 1_500_002,
            "sameProcessAndReopenedHitsRequired": true,
            "normalLiveTargetEntries": 5_000_000,
            "postPruneTargetEntries": 4_500_000,
            "activeHardHighWaterEntries": 10_000_000,
        },
        "checks": checks,
    });
    write_new_json(&evidence_path, &evidence);
    append_new_journal(
        &journal_path,
        &json!({"event":"evidence_finalized","status":if valid { "valid" } else { "invalid" }}),
        false,
    );
    eprintln!(
        "SOP10f files={} directories={} analysis={:.3}s peak-private={} digest={} status={}",
        FILE_COUNT,
        directory_count,
        elapsed.as_secs_f64(),
        peak_private_bytes,
        result_digest,
        if valid { "valid" } else { "invalid" }
    );
    assert!(valid, "SOP10f retained result failed one or more checks");
}

fn create_run(db: &Database, root: &Path) -> i64 {
    let root = root.to_string_lossy().into_owned();
    let session = db
        .create_session("SOP10 scale", std::slice::from_ref(&root), &[])
        .unwrap();
    let run = db
        .create_scan_run(
            session,
            &RunParameters {
                roots: vec![root],
                ignore_patterns: vec![],
                directory_similarity_threshold_millis: 500,
                repeat_cache_policy: Default::default(),
                cloud_policy: Default::default(),
                manual_location_exclusions: vec![],
                registered_cloud_locations: vec![],
                cloud_detection_status: Default::default(),
            },
            "sop10-scale",
        )
        .unwrap();
    db.start_scan_run(run).unwrap();
    run
}

fn generate_bulk_rows(db: &Database, run_id: i64) {
    db.connection()
        .execute_batch(
            "PRAGMA synchronous = OFF; PRAGMA journal_mode = OFF; PRAGMA temp_store = MEMORY;",
        )
        .unwrap();
    db.connection()
        .execute(
            "WITH RECURSIVE numbers(n) AS (
                 VALUES(0) UNION ALL SELECT n + 1 FROM numbers WHERE n + 1 < ?2
             )
             INSERT INTO scanned_file
                (run_id, root_path, canonical_path, relative_path, file_name, extension_key,
                 parent_dir, drive_letter, file_size, last_modified, partial_hash, content_hash,
                 file_identity, warning_message, marked_deleted)
             SELECT ?1,
                    'S:\\synthetic',
                    printf('S:\\synthetic\\d%06d\\f%02d-%06d.bin', n % ?3, n / ?3, n % ?3),
                    printf('d%06d/f%02d-%06d.bin', n % ?3, n / ?3, n % ?3),
                    printf('f%02d-%06d.bin', n / ?3, n % ?3),
                    'bin',
                    printf('S:\\synthetic\\d%06d', n % ?3),
                    'S:',
                    (n / ?3) + 1,
                    0,
                    n,
                    n,
                    printf('bulk:%d', n),
                    NULL,
                    0
             FROM numbers",
            params![run_id, BULK_FILE_COUNT as i64, BULK_DIRECTORY_COUNT as i64],
        )
        .unwrap();
}

fn insert_correctness_rows(db: &Database, run_id: i64, root: &Path) {
    let definitions = [
        ("exact-a/a/b/c/d/e/f/g/h/item.bin", Some(11), None, true),
        ("exact-b/a/b/c/d/e/f/g/h/item.bin", Some(11), None, true),
        ("hard-a/item.bin", Some(22), Some("physical-hardlink"), true),
        ("hard-b/item.bin", Some(22), Some("physical-hardlink"), true),
        ("different-a/item.bin", Some(31), None, true),
        ("different-b/item.bin", Some(32), None, true),
        ("warning-a/item.bin", Some(41), None, true),
        ("warning-b/item.bin", Some(41), None, false),
    ];
    let files = definitions
        .into_iter()
        .map(|(relative, hash, identity, present)| {
            let path = relative
                .split('/')
                .fold(PathBuf::from(root), |path, part| path.join(part));
            if present {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, vec![0x5a; 16]).unwrap();
            }
            ScannedFile {
                id: 0,
                run_id,
                root_path: root.to_string_lossy().into_owned(),
                canonical_path: path.to_string_lossy().into_owned(),
                relative_path: relative.to_owned(),
                file_name: "item.bin".to_owned(),
                parent_dir: path.parent().unwrap().to_string_lossy().into_owned(),
                drive_letter: String::new(),
                file_size: 16,
                last_modified: 0,
                partial_hash: None,
                content_hash: hash,
                file_identity: identity.map(str::to_owned),
                warning_message: None,
                marked_deleted: false,
            }
        })
        .collect::<Vec<_>>();
    db.insert_scanned_files(&files).unwrap();
}

fn cancellation_preflight() -> bool {
    let db = Database::open_in_memory().unwrap();
    let temp = TempDir::new().unwrap();
    let run_id = create_run(&db, temp.path());
    let cancelled = AtomicBool::new(true);
    matches!(
        exact_folders::analyze_exact_folders_cancellable(&db, run_id, &cancelled, &SilentReporter,),
        Err(super_duper_core::Error::Cancelled)
    ) && directory_count(&db, run_id) == 0
}

fn exact_result_digest(db: &Database, run_id: i64, root: &Path) -> String {
    let page = db
        .page_duplicate_folder_groups(&DuplicateFolderGroupPageQuery {
            run_id,
            limit: 10,
            sort_field: DuplicateFolderGroupSortField::RepresentativePath,
            sort_direction: SortDirection::Ascending,
            filter: DuplicateFolderGroupFilter {
                search: None,
                minimum_size: 0,
            },
            cursor: None,
        })
        .unwrap();
    let mut hasher = XxHash64::with_seed(0x534f_5031_3046);
    hasher.write_i64(page.total);
    for group in page.groups {
        hasher.write_i64(group.total_size);
        hasher.write_i64(group.file_count);
        hasher.write_i64(group.folder_count);
        let relative = Path::new(&group.representative_path)
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        hasher.write(relative.as_bytes());
    }
    format!("{:016x}", hasher.finish())
}

fn scanned_file_count(db: &Database, run_id: i64) -> usize {
    db.connection()
        .query_row(
            "SELECT COUNT(*) FROM scanned_file WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )
        .unwrap()
}

fn directory_count(db: &Database, run_id: i64) -> usize {
    db.connection()
        .query_row(
            "SELECT COUNT(*) FROM directory_node WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )
        .unwrap()
}

fn similarity_pair_count(db: &Database, run_id: i64) -> usize {
    db.connection()
        .query_row(
            "SELECT COUNT(*) FROM directory_similarity WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )
        .unwrap()
}

fn required_new_path(variable: &str) -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os(variable).unwrap_or_else(|| panic!("{variable} is required")),
    );
    assert!(
        !path.exists(),
        "write-once path already exists: {}",
        path.display()
    );
    path
}

fn append_new_journal(path: &Path, value: &serde_json::Value, create: bool) {
    let mut options = OpenOptions::new();
    options.write(true);
    if create {
        options.create_new(true);
    } else {
        options.append(true);
    }
    let mut file = options.open(path).unwrap();
    serde_json::to_writer(&mut file, value).unwrap();
    writeln!(file).unwrap();
    file.sync_all().unwrap();
}

fn write_new_json(path: &Path, value: &serde_json::Value) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    serde_json::to_writer_pretty(&mut file, value).unwrap();
    writeln!(file).unwrap();
    file.sync_all().unwrap();
}

fn file_bytes(path: &Path) -> u64 {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

#[cfg(windows)]
fn current_process_private_bytes() -> u64 {
    let mut counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    let result = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters as *mut PROCESS_MEMORY_COUNTERS_EX as *mut PROCESS_MEMORY_COUNTERS,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        )
    };
    assert_ne!(result, 0, "GetProcessMemoryInfo failed");
    counters.PrivateUsage as u64
}

#[cfg(not(windows))]
fn current_process_private_bytes() -> u64 {
    0
}
