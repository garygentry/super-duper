use rusqlite::Connection;
use super_duper_core::telemetry::{
    CounterKind, ScanCounters, StatusDatabase, TelemetryPhase, TelemetryRunState,
    CURRENT_STATUS_SCHEMA_VERSION, METRICS_CONTRACT_VERSION,
};
use tempfile::tempdir;

#[test]
fn metric_contract_has_stable_unique_identifiers() {
    assert_eq!(METRICS_CONTRACT_VERSION, 1);
    let mut identifiers = CounterKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>();
    identifiers.sort_unstable();
    identifiers.dedup();
    assert_eq!(identifiers.len(), CounterKind::ALL.len());
    assert_eq!(
        TelemetryPhase::CandidateScreening.as_str(),
        "candidate_screening"
    );
    assert_eq!(TelemetryRunState::Interrupted.as_str(), "interrupted");
}

#[test]
fn counter_invariants_accept_partial_progress_and_reject_semantic_mixing() {
    let valid = ScanCounters {
        discovered_files: 12,
        discovered_bytes: 1_200,
        zero_byte_files: 1,
        hard_link_alias_files: 1,
        hard_link_alias_bytes: 100,
        size_buckets: 4,
        singleton_size_buckets: 2,
        singleton_size_files: 2,
        singleton_size_bytes: 200,
        candidate_size_buckets: 2,
        candidate_files: 8,
        candidate_bytes: 800,
        partial_hashes_attempted: 6,
        partial_hashes_succeeded: 5,
        partial_hashes_failed: 1,
        partial_collision_buckets: 1,
        partial_collision_files: 3,
        partial_collision_bytes: 300,
        full_hash_requests: 3,
        full_hash_cache_hits: 1,
        full_hash_cache_misses: 1,
        full_hash_cache_errors: 1,
        full_hash_content_reads_started: 2,
        full_hash_content_reads_completed: 1,
        confirmed_duplicate_groups: 1,
        confirmed_logical_copies: 2,
        confirmed_physical_items: 2,
        ..Default::default()
    };
    valid.validate().unwrap();

    let mut invalid = valid.clone();
    invalid.singleton_size_files = 3;
    assert_eq!(
        invalid.validate().unwrap_err().0,
        "every singleton size bucket contains exactly one file"
    );

    let mut invalid = valid;
    invalid.full_hash_content_reads_started = 3;
    assert_eq!(
        invalid.validate().unwrap_err().0,
        "content reads require a cache miss or cache error"
    );

    let overflow = ScanCounters {
        discovered_files: u64::MAX,
        zero_byte_files: u64::MAX,
        hard_link_alias_files: 1,
        ..Default::default()
    };
    assert_eq!(
        overflow.validate().unwrap_err().0,
        "excluded logical files cannot exceed discovered files"
    );
}

#[test]
fn status_schema_creates_reopens_and_stays_separate_from_product_tables() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("scan-status.db");
    let database = StatusDatabase::open(path.to_str().unwrap()).unwrap();
    assert_eq!(
        database.schema_version().unwrap(),
        CURRENT_STATUS_SCHEMA_VERSION
    );
    for table in [
        "status_run",
        "status_phase",
        "status_counter",
        "status_device",
        "status_host_sample",
        "status_device_sample",
    ] {
        let exists: bool = database
            .connection()
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
                 )",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "missing status table {table}");
    }
    let has_product_run: bool = database
        .connection()
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'scan_run'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!has_product_run);
    drop(database);

    let reopened = StatusDatabase::open(path.to_str().unwrap()).unwrap();
    assert_eq!(
        reopened.schema_version().unwrap(),
        CURRENT_STATUS_SCHEMA_VERSION
    );
}

#[test]
fn status_schema_rejects_newer_or_unversioned_nonempty_databases_without_changes() {
    let temp = tempdir().unwrap();
    let newer_path = temp.path().join("newer-status.db");
    let newer = Connection::open(&newer_path).unwrap();
    newer
        .execute_batch("CREATE TABLE future_evidence(id INTEGER); PRAGMA user_version = 99;")
        .unwrap();
    drop(newer);

    let error = StatusDatabase::open(newer_path.to_str().unwrap())
        .err()
        .expect("newer schema must fail");
    assert!(error.to_string().contains("newer than supported"));
    let newer = Connection::open(&newer_path).unwrap();
    let version: i64 = newer
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 99);
    let row_count: i64 = newer
        .query_row("SELECT COUNT(*) FROM future_evidence", [], |row| row.get(0))
        .unwrap();
    assert_eq!(row_count, 0);

    let legacy_path = temp.path().join("unversioned-status.db");
    let legacy = Connection::open(&legacy_path).unwrap();
    legacy
        .execute_batch("CREATE TABLE unknown_local_state(id INTEGER PRIMARY KEY);")
        .unwrap();
    drop(legacy);
    let error = StatusDatabase::open(legacy_path.to_str().unwrap())
        .err()
        .expect("unversioned non-empty schema must fail");
    assert!(error.to_string().contains("was not modified"));
    let legacy = Connection::open(&legacy_path).unwrap();
    let exists: bool = legacy
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'unknown_local_state'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(exists);
}
