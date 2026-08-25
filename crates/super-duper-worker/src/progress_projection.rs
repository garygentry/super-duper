use serde_json::{json, Map, Value};
use super_duper_core::telemetry::ScanProgressSnapshot;

pub(crate) const PROGRESS_EVENT_INTERVAL_NANOS: u64 = 100_000_000;

#[derive(Clone, Debug)]
pub(crate) struct LegacyProgressProjection {
    pub phase: &'static str,
    pub files_discovered: usize,
    pub bytes_discovered: u64,
    pub files_hashed: usize,
    pub warning_count: usize,
    pub current_path: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingProgress {
    pub snapshot: ScanProgressSnapshot,
    pub legacy: LegacyProgressProjection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Coalesced<T> {
    pub sequence: u64,
    pub emitted_at_nanos: u64,
    pub cancelling: bool,
    pub value: T,
}

#[derive(Debug)]
pub(crate) struct LatestValueCoalescer<T> {
    pending: Option<T>,
    last_emitted_nanos: Option<u64>,
    next_sequence: u64,
    cancelling: bool,
    terminal: bool,
}

impl<T> Default for LatestValueCoalescer<T> {
    fn default() -> Self {
        Self {
            pending: None,
            last_emitted_nanos: None,
            next_sequence: 1,
            cancelling: false,
            terminal: false,
        }
    }
}

impl<T> LatestValueCoalescer<T> {
    pub(crate) fn submit(&mut self, value: T, cancelling: bool) {
        if self.terminal {
            return;
        }
        self.cancelling |= cancelling;
        self.pending = Some(value);
    }

    #[cfg(test)]
    pub(crate) fn offer(
        &mut self,
        now_nanos: u64,
        value: T,
        cancelling: bool,
    ) -> Option<Coalesced<T>> {
        self.submit(value, cancelling);
        self.take_due(now_nanos)
    }

    pub(crate) fn latch_cancelling(&mut self, cancelling: bool) {
        self.cancelling |= cancelling;
    }

    pub(crate) fn take_due(&mut self, now_nanos: u64) -> Option<Coalesced<T>> {
        if self.terminal || self.pending.is_none() {
            return None;
        }
        if self
            .last_emitted_nanos
            .is_some_and(|last| now_nanos.saturating_sub(last) < PROGRESS_EVENT_INTERVAL_NANOS)
        {
            return None;
        }
        let sequence = self.next_sequence;
        let Some(next_sequence) = sequence.checked_add(1) else {
            self.terminal = true;
            self.pending = None;
            return None;
        };
        self.next_sequence = next_sequence;
        self.last_emitted_nanos = Some(now_nanos);
        Some(Coalesced {
            sequence,
            emitted_at_nanos: now_nanos,
            cancelling: self.cancelling,
            value: self.pending.take().expect("pending progress was checked"),
        })
    }

    pub(crate) fn next_due_nanos(&self) -> Option<u64> {
        if self.terminal || self.pending.is_none() {
            return None;
        }
        Some(
            self.last_emitted_nanos
                .map_or(0, |last| last.saturating_add(PROGRESS_EVENT_INTERVAL_NANOS)),
        )
    }

    pub(crate) fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub(crate) fn terminate(&mut self) {
        self.terminal = true;
        self.pending = None;
    }
}

pub(crate) fn progress_event_data(
    run_id: i64,
    progress: &Coalesced<PendingProgress>,
) -> Result<Value, &'static str> {
    let legacy = &progress.value.legacy;
    let mut data = json!({
        "runId": run_id,
        "sequence": progress.sequence,
        "status": if progress.cancelling { "cancelling" } else { "running" },
        "phase": legacy.phase,
        "filesDiscovered": legacy.files_discovered,
        "bytesDiscovered": legacy.bytes_discovered.to_string(),
        "filesHashed": legacy.files_hashed,
        "warningCount": legacy.warning_count,
        "progress": progress_snapshot_value(&progress.value.snapshot)?,
    });
    if let Some(path) = legacy.current_path.as_ref().filter(|path| !path.is_empty()) {
        data["currentPath"] = Value::String(path.clone());
    }
    if legacy.warning_count > 0 {
        data["message"] = Value::String(
            "The scan encountered recoverable warnings; see local diagnostics.".to_owned(),
        );
    }
    Ok(data)
}

fn progress_snapshot_value(snapshot: &ScanProgressSnapshot) -> Result<Value, &'static str> {
    let mut value = serde_json::to_value(snapshot).map_err(|_| "snapshot serialization failed")?;

    let counters = object_field_mut(&mut value, "counters")?;
    decimal_fields(
        counters,
        &[
            "discoveredBytes",
            "hardLinkAliasBytes",
            "singletonSizeBytes",
            "candidateBytes",
            "duplicateCandidateBytes",
            "metadataResolvedBytes",
            "partialHashBytesRead",
            "partialCollisionBytes",
            "fullHashBytesRead",
            "recoverableBytes",
        ],
    )?;

    let logical = object_field_mut(&mut value, "logical")?;
    decimal_fields(
        logical,
        &[
            "partialScreenedBytes",
            "fullHashRequestBytes",
            "fullHashSatisfiedBytes",
            "fullHashFailedBytes",
            "hashPipelineResolvedBytes",
            "confirmedLogicalBytes",
        ],
    )?;

    let funnel = object_field_mut(&mut value, "funnel")?;
    for stage in [
        "discovered",
        "metadataResolved",
        "hashPipelineCandidates",
        "partialScreened",
        "selectedForFullHash",
        "fullHashSatisfied",
        "finalizedDuplicates",
    ] {
        decimal_field(object_field_mut_in(funnel, stage)?, "logicalBytes")?;
    }

    for rates in ["partialReadRates", "fullReadRates"] {
        let rates = object_field_mut(&mut value, rates)?;
        for window in ["cumulative", "recent"] {
            let window = object_field_mut_in(rates, window)?;
            if window.get("state").and_then(Value::as_str) == Some("available") {
                decimal_field(
                    object_field_mut_in(window, "rate")?,
                    "physicalBytesPerSecond",
                )?;
            }
        }
    }

    if !value["remainingKnownWork"].is_null() {
        decimal_field(
            object_field_mut(&mut value, "remainingKnownWork")?,
            "logicalBytes",
        )?;
    }
    let eta = object_field_mut(&mut value, "eta")?;
    if eta.get("state").and_then(Value::as_str) == Some("available") {
        decimal_fields(
            eta,
            &["remaining_logical_bytes", "logical_bytes_per_second_millis"],
        )?;
    }
    Ok(value)
}

fn object_field_mut<'a>(
    value: &'a mut Value,
    field: &'static str,
) -> Result<&'a mut Map<String, Value>, &'static str> {
    value
        .get_mut(field)
        .and_then(Value::as_object_mut)
        .ok_or("progress snapshot field shape changed")
}

fn object_field_mut_in<'a>(
    object: &'a mut Map<String, Value>,
    field: &'static str,
) -> Result<&'a mut Map<String, Value>, &'static str> {
    object
        .get_mut(field)
        .and_then(Value::as_object_mut)
        .ok_or("progress snapshot nested field shape changed")
}

fn decimal_fields(
    object: &mut Map<String, Value>,
    fields: &[&'static str],
) -> Result<(), &'static str> {
    for field in fields {
        decimal_field(object, field)?;
    }
    Ok(())
}

fn decimal_field(object: &mut Map<String, Value>, field: &'static str) -> Result<(), &'static str> {
    let value = object
        .get_mut(field)
        .ok_or("progress snapshot byte field is missing")?;
    let number = value
        .as_u64()
        .ok_or("progress snapshot byte field is not an unsigned integer")?;
    *value = Value::String(number.to_string());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super_duper_core::telemetry::{
        ActiveDeviceProgress, ActiveDeviceUnavailableReason, CandidateFunnelProgress, ProgressEta,
        ProgressLogicalCounters, ProgressQuantity, ProgressRate, ProgressRateValue, ProgressRates,
        RemainingKnownWork, RemainingWorkStage, ScanCounters, TelemetryPhase,
        METRICS_CONTRACT_VERSION, PROGRESS_CONTRACT_VERSION,
    };

    #[test]
    fn thousand_updates_are_latest_wins_and_bounded_in_every_half_open_second() {
        let mut coalescer = LatestValueCoalescer::default();
        let mut emitted = Vec::new();
        for update in 0..1_000_u64 {
            if let Some(value) = coalescer.offer(update * 1_000_000, update, false) {
                emitted.push(value);
            }
        }
        emitted.push(coalescer.take_due(1_000_000_000).unwrap());

        assert_eq!(
            emitted.iter().map(|value| value.value).collect::<Vec<_>>(),
            vec![0, 100, 200, 300, 400, 500, 600, 700, 800, 900, 999]
        );
        assert!(emitted
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence
                && pair[1].emitted_at_nanos - pair[0].emitted_at_nanos
                    >= PROGRESS_EVENT_INTERVAL_NANOS));
        for window_start_millis in 0..=1_000_u64 {
            let start = window_start_millis * 1_000_000;
            let end = start + 1_000_000_000;
            assert!(
                emitted
                    .iter()
                    .filter(|value| {
                        value.emitted_at_nanos >= start && value.emitted_at_nanos < end
                    })
                    .count()
                    <= 10
            );
        }
    }

    #[test]
    fn phase_churn_waits_for_next_legal_slot_and_cancelling_is_sticky() {
        let mut coalescer = LatestValueCoalescer::default();
        assert_eq!(
            coalescer.offer(0, "discovering", false).unwrap().sequence,
            1
        );
        assert!(coalescer
            .offer(10_000_000, "candidate_screening", false)
            .is_none());
        assert!(coalescer.offer(20_000_000, "persisting", true).is_none());
        assert!(coalescer.offer(99_999_999, "finalizing", false).is_none());
        let emission = coalescer.take_due(100_000_000).unwrap();
        assert_eq!(emission.sequence, 2);
        assert_eq!(emission.value, "finalizing");
        assert!(emission.cancelling);
    }

    #[test]
    fn terminal_discards_pending_and_suppresses_late_progress() {
        let mut coalescer = LatestValueCoalescer::default();
        assert!(coalescer.offer(0, 1_u64, false).is_some());
        assert!(coalescer.offer(1, 2, false).is_none());
        coalescer.terminate();
        assert!(coalescer.take_due(PROGRESS_EVENT_INTERVAL_NANOS).is_none());
        assert!(coalescer
            .offer(PROGRESS_EVENT_INTERVAL_NANOS, 3, false)
            .is_none());
    }

    #[test]
    fn typed_snapshot_projects_every_byte_quantity_as_a_decimal_string() {
        let mut counters = ScanCounters::default();
        counters.discovered_bytes = u64::MAX;
        counters.hard_link_alias_bytes = u64::MAX;
        counters.singleton_size_bytes = u64::MAX;
        counters.candidate_bytes = u64::MAX;
        counters.duplicate_candidate_bytes = u64::MAX;
        counters.metadata_resolved_bytes = u64::MAX;
        counters.partial_hash_bytes_read = u64::MAX;
        counters.partial_collision_bytes = u64::MAX;
        counters.full_hash_bytes_read = u64::MAX;
        counters.recoverable_bytes = u64::MAX;
        let quantity = ProgressQuantity {
            files: 7,
            logical_bytes: u64::MAX,
        };
        let rate = ProgressRateValue::Available {
            rate: ProgressRate {
                files_per_second_millis: 1_000,
                physical_bytes_per_second: u64::MAX,
                window_nanos: 1_000_000_000,
            },
        };
        let snapshot = ScanProgressSnapshot {
            progress_contract_version: PROGRESS_CONTRACT_VERSION,
            metrics_contract_version: METRICS_CONTRACT_VERSION,
            revision: 9,
            monotonic_nanos: 10_000_000_000,
            phase: TelemetryPhase::CandidateScreening,
            phase_elapsed_nanos: 9_000_000_000,
            counters,
            logical: ProgressLogicalCounters {
                partial_screened_bytes: u64::MAX,
                full_hash_request_bytes: u64::MAX,
                full_hash_satisfied_bytes: u64::MAX,
                full_hash_failed_bytes: u64::MAX,
                hash_pipeline_resolved_bytes: u64::MAX,
                confirmed_logical_bytes: u64::MAX,
                ..ProgressLogicalCounters::default()
            },
            funnel: CandidateFunnelProgress {
                discovered: quantity,
                metadata_resolved: quantity,
                hash_pipeline_candidates: quantity,
                partial_screened: quantity,
                selected_for_full_hash: quantity,
                full_hash_satisfied: quantity,
                finalized_duplicates: quantity,
            },
            partial_read_rates: ProgressRates {
                cumulative: rate,
                recent: rate,
            },
            full_read_rates: ProgressRates {
                cumulative: rate,
                recent: rate,
            },
            cache_hit_rate_basis_points: Some(5_000),
            warning_count: 3,
            active_devices: ActiveDeviceProgress::Unavailable {
                reason: ActiveDeviceUnavailableReason::MappingUnavailable,
            },
            remaining_known_work: Some(RemainingKnownWork {
                stage: RemainingWorkStage::HashPipeline,
                files: 1,
                logical_bytes: u64::MAX,
            }),
            eta: ProgressEta::Available {
                stage: RemainingWorkStage::HashPipeline,
                remaining_logical_bytes: u64::MAX,
                logical_bytes_per_second_millis: u64::MAX,
                estimated_seconds: 42,
                window_nanos: 10_000_000_000,
            },
        };
        let data = progress_event_data(
            19,
            &Coalesced {
                sequence: 8,
                emitted_at_nanos: 0,
                cancelling: false,
                value: PendingProgress {
                    snapshot,
                    legacy: LegacyProgressProjection {
                        phase: "hashing",
                        files_discovered: 7,
                        bytes_discovered: u64::MAX,
                        files_hashed: 4,
                        warning_count: 3,
                        current_path: None,
                    },
                },
            },
        )
        .unwrap();

        let decimal = u64::MAX.to_string();
        assert_eq!(data["bytesDiscovered"], decimal);
        assert_eq!(data["progress"]["counters"]["candidateBytes"], decimal);
        assert_eq!(
            data["progress"]["logical"]["hashPipelineResolvedBytes"],
            decimal
        );
        assert_eq!(
            data["progress"]["funnel"]["selectedForFullHash"]["logicalBytes"],
            decimal
        );
        assert_eq!(
            data["progress"]["partialReadRates"]["recent"]["rate"]["physicalBytesPerSecond"],
            decimal
        );
        assert_eq!(
            data["progress"]["remainingKnownWork"]["logicalBytes"],
            decimal
        );
        assert_eq!(
            data["progress"]["eta"]["logical_bytes_per_second_millis"],
            decimal
        );
        assert_eq!(
            data["progress"]["activeDevices"]["reason"],
            "mapping_unavailable"
        );
    }
}
