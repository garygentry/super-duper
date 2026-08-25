use super_duper_core::telemetry::{
    ActiveDeviceProgress, ActiveDeviceUnavailableReason, EtaUnavailableReason,
    ProgressContractError, ProgressEta, ProgressLogicalCounters, ProgressObservation,
    ProgressRateUnavailableReason, ProgressRateValue, ProgressReducer, RemainingWorkStage,
    ScanCounters, TelemetryPhase, ETA_MIN_INTERVAL_NANOS, ETA_MIN_OBSERVATION_SPAN_NANOS,
    ETA_RATE_STABILITY_MIN_BASIS_POINTS, MAX_ACTIVE_PROGRESS_DEVICES, MAX_PROGRESS_RATE_POINTS,
    METRICS_CONTRACT_VERSION, PROGRESS_CONTRACT_VERSION, PROGRESS_RATE_POINT_MIN_INTERVAL_NANOS,
    RECENT_PROGRESS_RATE_WINDOW_NANOS,
};

const SECOND: u64 = 1_000_000_000;

fn observation(
    monotonic_nanos: u64,
    candidate_files: u64,
    candidate_bytes: u64,
    resolved_files: u64,
    resolved_bytes: u64,
    physical_partial_bytes: u64,
) -> ProgressObservation {
    ProgressObservation {
        progress_contract_version: PROGRESS_CONTRACT_VERSION,
        metrics_contract_version: METRICS_CONTRACT_VERSION,
        monotonic_nanos,
        phase: TelemetryPhase::CandidateScreening,
        phase_started_monotonic_nanos: 0,
        candidate_totals_known: true,
        final_results_complete: false,
        counters: ScanCounters {
            discovered_files: candidate_files,
            discovered_bytes: candidate_bytes,
            size_buckets: u64::from(candidate_files > 0),
            candidate_size_buckets: u64::from(candidate_files > 0),
            candidate_files,
            candidate_bytes,
            partial_hashes_attempted: resolved_files,
            partial_hashes_succeeded: resolved_files,
            partial_hash_bytes_read: physical_partial_bytes,
            ..Default::default()
        },
        logical: ProgressLogicalCounters {
            partial_screened_files: resolved_files,
            partial_screened_bytes: resolved_bytes,
            hash_pipeline_resolved_files: resolved_files,
            hash_pipeline_resolved_bytes: resolved_bytes,
            ..Default::default()
        },
        active_devices: ActiveDeviceProgress::Unavailable {
            reason: ActiveDeviceUnavailableReason::NoActiveIo,
        },
    }
}

fn rate(value: ProgressRateValue) -> super_duper_core::telemetry::ProgressRate {
    match value {
        ProgressRateValue::Available { rate } => rate,
        ProgressRateValue::Unavailable { reason } => {
            panic!("expected an available rate, got {reason:?}")
        }
    }
}

fn assert_transition_rejected_atomically(
    previous: ProgressObservation,
    proposed: ProgressObservation,
    expected_message: &'static str,
) {
    let mut reducer = ProgressReducer::new();
    assert_eq!(reducer.observe(previous.clone()).unwrap().revision, 1);
    assert_eq!(
        reducer.observe(proposed.clone()).unwrap_err(),
        ProgressContractError::Invariant(expected_message)
    );
    let mut continuation = previous;
    continuation.monotonic_nanos = proposed
        .monotonic_nanos
        .max(continuation.monotonic_nanos + 1);
    assert_eq!(
        reducer.observe(continuation).unwrap().revision,
        2,
        "rejected transition must not consume revision or reducer state"
    );
}

#[test]
fn progress_contract_round_trips_versions_and_rejects_unknown_semantics() {
    assert_eq!(PROGRESS_CONTRACT_VERSION, 1);
    assert_eq!(PROGRESS_RATE_POINT_MIN_INTERVAL_NANOS, 100_000_000);
    assert_eq!(RECENT_PROGRESS_RATE_WINDOW_NANOS, 30 * SECOND);
    assert_eq!(ETA_MIN_OBSERVATION_SPAN_NANOS, 10 * SECOND);
    assert_eq!(ETA_MIN_INTERVAL_NANOS, 5 * SECOND);
    assert_eq!(ETA_RATE_STABILITY_MIN_BASIS_POINTS, 7_500);
    assert_eq!(MAX_PROGRESS_RATE_POINTS, 304);

    let input = observation(0, 10, 10_000, 0, 0, 0);
    let encoded = serde_json::to_string(&input).unwrap();
    let decoded: ProgressObservation = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, input);

    let snapshot = ProgressReducer::new().observe(decoded).unwrap();
    let snapshot_json = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(
        snapshot_json["partialReadRates"]["recent"],
        serde_json::json!({"state": "unavailable", "reason": "no_elapsed_time"})
    );
    assert_eq!(
        serde_json::from_value::<super_duper_core::telemetry::ScanProgressSnapshot>(snapshot_json)
            .unwrap(),
        snapshot
    );

    let mut unknown: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    unknown["unreviewedSemantic"] = serde_json::json!(0);
    assert!(serde_json::from_value::<ProgressObservation>(unknown).is_err());

    let mut unsupported = input;
    unsupported.progress_contract_version += 1;
    assert_eq!(
        ProgressReducer::new().observe(unsupported).unwrap_err(),
        ProgressContractError::UnsupportedVersion {
            expected: PROGRESS_CONTRACT_VERSION,
            actual: PROGRESS_CONTRACT_VERSION + 1,
        }
    );

    let mut unsupported_metrics = observation(0, 0, 0, 0, 0, 0);
    unsupported_metrics.metrics_contract_version += 1;
    assert_eq!(
        ProgressReducer::new()
            .observe(unsupported_metrics)
            .unwrap_err(),
        ProgressContractError::UnsupportedMetricsVersion {
            expected: METRICS_CONTRACT_VERSION,
            actual: METRICS_CONTRACT_VERSION + 1,
        }
    );
}

#[test]
fn full_hash_failures_include_failures_before_content_reads_start() {
    let mut failed = observation(SECOND, 2, 200, 2, 200, 200);
    failed.counters.partial_collision_buckets = 1;
    failed.counters.partial_collision_files = 2;
    failed.counters.partial_collision_bytes = 200;
    failed.counters.full_hash_requests = 2;
    failed.counters.full_hash_cache_misses = 1;
    failed.counters.full_hash_content_reads_started = 1;
    failed.counters.full_hash_content_reads_failed = 1;
    failed.counters.warnings = 2;
    failed.logical.full_hash_request_bytes = 200;
    failed.logical.full_hash_failed_files = 2;
    failed.logical.full_hash_failed_bytes = 200;
    failed.logical.hash_pipeline_resolved_files = 2;
    failed.logical.hash_pipeline_resolved_bytes = 200;

    assert!(ProgressReducer::new().observe(failed.clone()).is_ok());

    failed.logical.full_hash_failed_files = 0;
    assert_eq!(
        ProgressReducer::new().observe(failed).unwrap_err(),
        ProgressContractError::Invariant(
            "failed full-content reads cannot exceed failed full-hash requests"
        )
    );
}

#[test]
fn progress_transitions_are_monotonic_checked_and_atomic() {
    let mut reducer = ProgressReducer::new();
    let first = reducer
        .observe(observation(SECOND, 100, 100_000, 10, 10_000, 1_000))
        .unwrap();
    assert_eq!(first.revision, 1);

    let mut counter_regression = observation(2 * SECOND, 100, 100_000, 20, 20_000, 999);
    counter_regression.counters.partial_hash_bytes_read = 999;
    assert_eq!(
        reducer.observe(counter_regression).unwrap_err(),
        ProgressContractError::CounterRegression {
            metric: "partial_hash_bytes_read"
        }
    );

    let mut logical_reducer = ProgressReducer::new();
    let mut complete_hash = observation(SECOND, 2, 200, 2, 200, 200);
    complete_hash.counters.partial_collision_buckets = 1;
    complete_hash.counters.partial_collision_files = 2;
    complete_hash.counters.partial_collision_bytes = 200;
    complete_hash.counters.full_hash_requests = 2;
    complete_hash.counters.full_hash_cache_hits = 2;
    complete_hash.counters.confirmed_logical_copies = 1;
    complete_hash.logical.full_hash_request_bytes = 200;
    complete_hash.logical.full_hash_satisfied_files = 2;
    complete_hash.logical.full_hash_satisfied_bytes = 200;
    complete_hash.logical.confirmed_logical_bytes = 100;
    logical_reducer.observe(complete_hash.clone()).unwrap();
    let mut logical_regression = complete_hash;
    logical_regression.monotonic_nanos = 2 * SECOND;
    logical_regression.logical.confirmed_logical_bytes = 50;
    assert_eq!(
        logical_reducer.observe(logical_regression).unwrap_err(),
        ProgressContractError::CounterRegression {
            metric: "confirmed_logical_bytes"
        }
    );

    let mut timestamp_regression = observation(SECOND - 1, 100, 100_000, 20, 20_000, 2_000);
    timestamp_regression.phase_started_monotonic_nanos = 0;
    assert!(matches!(
        reducer.observe(timestamp_regression),
        Err(ProgressContractError::Invariant(
            "observation time cannot regress"
        ))
    ));

    let next = reducer
        .observe(observation(2 * SECOND, 100, 100_000, 20, 20_000, 2_000))
        .unwrap();
    assert_eq!(next.revision, 2, "rejected updates must not advance state");

    let mut stale_phase_start = observation(3 * SECOND, 100, 100_000, 30, 30_000, 3_000);
    stale_phase_start.phase = TelemetryPhase::Persisting;
    stale_phase_start.phase_started_monotonic_nanos = SECOND;
    assert!(matches!(
        reducer.observe(stale_phase_start),
        Err(ProgressContractError::Invariant(
            "a new phase cannot start before the preceding observation"
        ))
    ));

    let mut synthetic_full_hash = observation(0, 1, 1, 0, 0, 0);
    synthetic_full_hash.phase = TelemetryPhase::FullHashing;
    assert!(matches!(
        ProgressReducer::new().observe(synthetic_full_hash),
        Err(ProgressContractError::Invariant(
            "full_hashing is reserved until the producer has a truthful global phase"
        ))
    ));

    let previous = observation(SECOND, 100, 100_000, 0, 0, 0);
    let mut changed_phase_start = previous.clone();
    changed_phase_start.monotonic_nanos = 2 * SECOND;
    changed_phase_start.phase_started_monotonic_nanos = SECOND;
    assert_transition_rejected_atomically(
        previous,
        changed_phase_start,
        "phase start cannot change within one phase",
    );

    let mut persisting = observation(SECOND, 100, 100_000, 0, 0, 0);
    persisting.phase = TelemetryPhase::Persisting;
    let mut phase_regression = persisting.clone();
    phase_regression.monotonic_nanos = 2 * SECOND;
    phase_regression.phase = TelemetryPhase::CandidateScreening;
    phase_regression.phase_started_monotonic_nanos = 2 * SECOND;
    assert_transition_rejected_atomically(
        persisting,
        phase_regression,
        "live progress phase cannot regress",
    );

    let mut known_discovery = observation(SECOND, 0, 0, 0, 0, 0);
    known_discovery.phase = TelemetryPhase::Discovering;
    let mut knowledge_regression = known_discovery.clone();
    knowledge_regression.monotonic_nanos = 2 * SECOND;
    knowledge_regression.candidate_totals_known = false;
    assert_transition_rejected_atomically(
        known_discovery,
        knowledge_regression,
        "candidate-total knowledge cannot regress",
    );

    let mut completed = observation(SECOND, 1, 1, 1, 1, 1);
    completed.phase = TelemetryPhase::Finalizing;
    completed.final_results_complete = true;
    let mut completion_regression = completed.clone();
    completion_regression.monotonic_nanos = 2 * SECOND;
    completion_regression.final_results_complete = false;
    assert_transition_rejected_atomically(
        completed,
        completion_regression,
        "final-result knowledge cannot regress",
    );

    let known_totals = observation(SECOND, 100, 100_000, 0, 0, 0);
    let mut changed_buckets = known_totals.clone();
    changed_buckets.monotonic_nanos = 2 * SECOND;
    changed_buckets.counters.size_buckets = 2;
    changed_buckets.counters.candidate_size_buckets = 2;
    let mut changed_files = known_totals.clone();
    changed_files.monotonic_nanos = 2 * SECOND;
    changed_files.counters.discovered_files = 101;
    changed_files.counters.candidate_files = 101;
    let mut changed_bytes = known_totals.clone();
    changed_bytes.monotonic_nanos = 2 * SECOND;
    changed_bytes.counters.discovered_bytes = 101_000;
    changed_bytes.counters.candidate_bytes = 101_000;
    for proposed in [changed_buckets, changed_files, changed_bytes] {
        assert_transition_rejected_atomically(
            known_totals.clone(),
            proposed,
            "known candidate totals cannot change",
        );
    }

    let overflow = ProgressObservation {
        counters: ScanCounters {
            discovered_files: u64::MAX,
            discovered_bytes: u64::MAX,
            candidate_files: u64::MAX,
            candidate_bytes: u64::MAX,
            partial_hashes_attempted: u64::MAX,
            partial_hashes_succeeded: u64::MAX,
            partial_hashes_failed: 1,
            ..Default::default()
        },
        logical: ProgressLogicalCounters {
            partial_screened_files: u64::MAX,
            partial_screened_bytes: u64::MAX,
            ..Default::default()
        },
        ..observation(3 * SECOND, 0, 0, 0, 0, 0)
    };
    assert!(ProgressReducer::new().observe(overflow).is_err());
}

#[test]
fn funnel_keeps_logical_work_and_physical_io_distinct() {
    let mut reducer = ProgressReducer::new();
    reducer
        .observe(observation(0, 10, 10_000, 0, 0, 0))
        .unwrap();
    let mut partially_classified = observation(10 * SECOND, 10, 10_000, 5, 5_000, 500);
    partially_classified.logical.hash_pipeline_resolved_files = 4;
    partially_classified.logical.hash_pipeline_resolved_bytes = 4_000;
    let snapshot = reducer.observe(partially_classified).unwrap();

    assert_eq!(snapshot.funnel.partial_screened.files, 5);
    assert_eq!(snapshot.funnel.partial_screened.logical_bytes, 5_000);
    let partial = rate(snapshot.partial_read_rates.cumulative);
    assert_eq!(partial.files_per_second_millis, 500);
    assert_eq!(partial.physical_bytes_per_second, 50);
    assert_ne!(
        snapshot.funnel.partial_screened.logical_bytes, partial.physical_bytes_per_second,
        "logical work bytes must never be relabelled as physical throughput"
    );
    assert_eq!(snapshot.remaining_known_work.unwrap().logical_bytes, 6_000);
}

#[test]
fn recent_and_cumulative_rates_use_exact_bounded_windows() {
    let mut reducer = ProgressReducer::new();
    let first = reducer
        .observe(observation(0, 100, 100_000, 0, 0, 0))
        .unwrap();
    assert_eq!(
        first.partial_read_rates.recent,
        ProgressRateValue::Unavailable {
            reason: ProgressRateUnavailableReason::NoElapsedTime
        }
    );

    reducer
        .observe(observation(10 * SECOND, 100, 100_000, 10, 10_000, 1_000))
        .unwrap();
    let snapshot = reducer
        .observe(observation(40 * SECOND, 100, 100_000, 40, 40_000, 4_000))
        .unwrap();
    let recent = rate(snapshot.partial_read_rates.recent);
    let cumulative = rate(snapshot.partial_read_rates.cumulative);
    assert_eq!(recent.window_nanos, RECENT_PROGRESS_RATE_WINDOW_NANOS);
    assert_eq!(recent.files_per_second_millis, 1_000);
    assert_eq!(recent.physical_bytes_per_second, 100);
    assert_eq!(cumulative.window_nanos, 40 * SECOND);
    assert_eq!(cumulative.files_per_second_millis, 1_000);
    assert_eq!(cumulative.physical_bytes_per_second, 100);

    let mut saturating = ProgressReducer::new();
    saturating
        .observe(observation(0, u64::MAX, u64::MAX, 0, 0, 0))
        .unwrap();
    let saturated = saturating
        .observe(observation(
            1,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
        ))
        .unwrap();
    assert_eq!(
        rate(saturated.partial_read_rates.cumulative).physical_bytes_per_second,
        u64::MAX
    );
}

#[test]
fn phase_change_resets_elapsed_without_resetting_run_cumulative_io() {
    let mut reducer = ProgressReducer::new();
    let mut discovering = observation(0, 0, 0, 0, 0, 0);
    discovering.phase = TelemetryPhase::Discovering;
    discovering.candidate_totals_known = false;
    reducer.observe(discovering).unwrap();

    let mut hashing = observation(10 * SECOND, 10, 10_000, 5, 5_000, 1_000);
    hashing.phase_started_monotonic_nanos = 10 * SECOND;
    let snapshot = reducer.observe(hashing).unwrap();
    assert_eq!(snapshot.phase_elapsed_nanos, 0);
    assert_eq!(
        rate(snapshot.partial_read_rates.cumulative).physical_bytes_per_second,
        100
    );
    assert_eq!(
        snapshot.eta,
        ProgressEta::Unavailable {
            reason: EtaUnavailableReason::WindowWarming
        }
    );

    let mut persisting = observation(20 * SECOND, 10, 10_000, 5, 5_000, 1_000);
    persisting.phase = TelemetryPhase::Persisting;
    persisting.phase_started_monotonic_nanos = 20 * SECOND;
    let persisted = reducer.observe(persisting).unwrap();
    assert_eq!(
        persisted.partial_read_rates.recent,
        ProgressRateValue::Unavailable {
            reason: ProgressRateUnavailableReason::NoElapsedTime
        }
    );
    assert_eq!(
        persisted.eta,
        ProgressEta::Unavailable {
            reason: EtaUnavailableReason::NotApplicable
        }
    );
}

#[test]
fn cache_effectiveness_uses_completed_lookup_outcomes() {
    let mut one_hit_many_errors = observation(0, 100, 100, 100, 100, 100);
    one_hit_many_errors.counters.partial_collision_buckets = 1;
    one_hit_many_errors.counters.partial_collision_files = 100;
    one_hit_many_errors.counters.partial_collision_bytes = 100;
    one_hit_many_errors.counters.full_hash_requests = 100;
    one_hit_many_errors.counters.full_hash_cache_hits = 1;
    one_hit_many_errors.counters.full_hash_cache_errors = 99;
    one_hit_many_errors.logical.full_hash_request_bytes = 100;
    one_hit_many_errors.logical.full_hash_satisfied_files = 1;
    one_hit_many_errors.logical.full_hash_satisfied_bytes = 1;
    one_hit_many_errors.logical.hash_pipeline_resolved_files = 1;
    one_hit_many_errors.logical.hash_pipeline_resolved_bytes = 1;

    one_hit_many_errors.monotonic_nanos = 10 * SECOND;
    let mut cache_reducer = ProgressReducer::new();
    cache_reducer
        .observe(observation(0, 100, 100, 0, 0, 0))
        .unwrap();
    let snapshot = cache_reducer.observe(one_hit_many_errors).unwrap();
    assert_eq!(snapshot.cache_hit_rate_basis_points, Some(100));
    let full_read = rate(snapshot.full_read_rates.cumulative);
    assert_eq!(full_read.files_per_second_millis, 0);
    assert_eq!(full_read.physical_bytes_per_second, 0);

    let no_outcome = ProgressReducer::new()
        .observe(observation(0, 1, 1, 0, 0, 0))
        .unwrap();
    assert_eq!(no_outcome.cache_hit_rate_basis_points, None);
}

#[test]
fn eta_and_active_device_states_are_closed_and_deterministic() {
    let mut stable = ProgressReducer::new();
    stable
        .observe(observation(0, 100, 100_000, 0, 0, 0))
        .unwrap();
    let warming = stable
        .observe(observation(5 * SECOND, 100, 100_000, 10, 10_000, 1_000))
        .unwrap();
    assert_eq!(
        warming.eta,
        ProgressEta::Unavailable {
            reason: EtaUnavailableReason::WindowWarming
        }
    );
    let available = stable
        .observe(observation(10 * SECOND, 100, 100_000, 20, 20_000, 2_000))
        .unwrap();
    assert_eq!(
        available.eta,
        ProgressEta::Available {
            stage: RemainingWorkStage::HashPipeline,
            remaining_logical_bytes: 80_000,
            logical_bytes_per_second_millis: 2_000_000,
            estimated_seconds: 40,
            window_nanos: 10 * SECOND,
        }
    );

    let mut irregular = ProgressReducer::new();
    irregular
        .observe(observation(0, 100, 100_000, 0, 0, 0))
        .unwrap();
    irregular
        .observe(observation(
            5 * SECOND + 100_000_000,
            100,
            100_000,
            10,
            10_200,
            1_000,
        ))
        .unwrap();
    assert_eq!(
        irregular
            .observe(observation(
                10 * SECOND + 200_000_000,
                100,
                100_000,
                20,
                20_400,
                2_000,
            ))
            .unwrap()
            .eta,
        ProgressEta::Available {
            stage: RemainingWorkStage::HashPipeline,
            remaining_logical_bytes: 79_600,
            logical_bytes_per_second_millis: 2_000_000,
            estimated_seconds: 40,
            window_nanos: 10 * SECOND + 200_000_000,
        }
    );

    let mut unequal_intervals = ProgressReducer::new();
    unequal_intervals
        .observe(observation(0, 100, 100_000, 0, 0, 0))
        .unwrap();
    unequal_intervals
        .observe(observation(6 * SECOND, 100, 100_000, 6, 6_000, 600))
        .unwrap();
    assert_eq!(
        unequal_intervals
            .observe(observation(11 * SECOND, 100, 100_000, 11, 11_000, 1_100))
            .unwrap()
            .eta,
        ProgressEta::Available {
            stage: RemainingWorkStage::HashPipeline,
            remaining_logical_bytes: 89_000,
            logical_bytes_per_second_millis: 1_000_000,
            estimated_seconds: 89,
            window_nanos: 11 * SECOND,
        }
    );

    let mut slow = ProgressReducer::new();
    slow.observe(observation(0, 100, 100, 0, 0, 0)).unwrap();
    slow.observe(observation(5 * SECOND, 100, 100, 1, 1, 1))
        .unwrap();
    assert_eq!(
        slow.observe(observation(10 * SECOND, 100, 100, 2, 2, 2))
            .unwrap()
            .eta,
        ProgressEta::Available {
            stage: RemainingWorkStage::HashPipeline,
            remaining_logical_bytes: 98,
            logical_bytes_per_second_millis: 200,
            estimated_seconds: 490,
            window_nanos: 10 * SECOND,
        }
    );

    let mut zero = ProgressReducer::new();
    zero.observe(observation(0, 1, 1, 0, 0, 0)).unwrap();
    zero.observe(observation(5 * SECOND, 1, 1, 0, 0, 0))
        .unwrap();
    assert_eq!(
        zero.observe(observation(10 * SECOND, 1, 1, 0, 0, 0))
            .unwrap()
            .eta,
        ProgressEta::Unavailable {
            reason: EtaUnavailableReason::NoRecentProgress
        }
    );

    let mut unstable = ProgressReducer::new();
    unstable
        .observe(observation(0, 100, 100_000, 0, 0, 0))
        .unwrap();
    unstable
        .observe(observation(5 * SECOND, 100, 100_000, 5, 5_000, 500))
        .unwrap();
    assert_eq!(
        unstable
            .observe(observation(10 * SECOND, 100, 100_000, 25, 25_000, 2_500))
            .unwrap()
            .eta,
        ProgressEta::Unavailable {
            reason: EtaUnavailableReason::UnstableRate
        }
    );

    let mut complete_observation = observation(0, 1, 1, 1, 1, 1);
    complete_observation.final_results_complete = true;
    complete_observation.phase = TelemetryPhase::Finalizing;
    assert_eq!(
        ProgressReducer::new()
            .observe(complete_observation)
            .unwrap()
            .eta,
        ProgressEta::Complete
    );

    let mut unknown = observation(0, 0, 0, 0, 0, 0);
    unknown.candidate_totals_known = false;
    unknown.phase = TelemetryPhase::Discovering;
    assert_eq!(
        ProgressReducer::new().observe(unknown).unwrap().eta,
        ProgressEta::Unavailable {
            reason: EtaUnavailableReason::WorkNotYetKnown
        }
    );

    let devices = (0..MAX_ACTIVE_PROGRESS_DEVICES)
        .map(|index| format!("physical:{index}"))
        .collect::<Vec<_>>();
    let mut multiple = observation(0, 1, 1, 0, 0, 0);
    multiple.active_devices = ActiveDeviceProgress::Multiple {
        device_keys: devices,
    };
    ProgressReducer::new().observe(multiple).unwrap();

    for invalid in [
        ActiveDeviceProgress::One {
            device_key: " ".to_owned(),
        },
        ActiveDeviceProgress::Multiple {
            device_keys: vec!["physical:0".to_owned(), "physical:0".to_owned()],
        },
        ActiveDeviceProgress::Multiple {
            device_keys: (0..=MAX_ACTIVE_PROGRESS_DEVICES)
                .map(|index| format!("physical:{index}"))
                .collect(),
        },
    ] {
        let mut invalid_observation = observation(0, 1, 1, 0, 0, 0);
        invalid_observation.active_devices = invalid;
        assert!(ProgressReducer::new().observe(invalid_observation).is_err());
    }
}
