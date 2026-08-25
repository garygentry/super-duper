use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    CounterKind, MetricInvariantError, ScanCounters, TelemetryPhase, METRICS_CONTRACT_VERSION,
};

pub const PROGRESS_CONTRACT_VERSION: u32 = 1;
pub const PROGRESS_RATE_POINT_MIN_INTERVAL_NANOS: u64 = 100_000_000;
pub const RECENT_PROGRESS_RATE_WINDOW_NANOS: u64 = 30_000_000_000;
pub const ETA_MIN_OBSERVATION_SPAN_NANOS: u64 = 10_000_000_000;
pub const ETA_MIN_INTERVAL_NANOS: u64 = 5_000_000_000;
pub const ETA_RATE_STABILITY_MIN_BASIS_POINTS: u32 = 7_500;
pub const MAX_PROGRESS_RATE_POINTS: usize = 304;
pub const MAX_ACTIVE_PROGRESS_DEVICES: usize = 64;
const NANOS_PER_SECOND: u128 = 1_000_000_000;
const BASIS_POINTS: u128 = 10_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgressQuantity {
    pub files: u64,
    pub logical_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgressLogicalCounters {
    pub partial_screened_files: u64,
    pub partial_screened_bytes: u64,
    pub full_hash_request_bytes: u64,
    pub full_hash_satisfied_files: u64,
    pub full_hash_satisfied_bytes: u64,
    pub full_hash_failed_files: u64,
    pub full_hash_failed_bytes: u64,
    pub hash_pipeline_resolved_files: u64,
    pub hash_pipeline_resolved_bytes: u64,
    pub confirmed_logical_bytes: u64,
}

type LogicalCounterAccessor = (&'static str, fn(&ProgressLogicalCounters) -> u64);

impl ProgressLogicalCounters {
    const ALL: [LogicalCounterAccessor; 10] = [
        ("partial_screened_files", |value| {
            value.partial_screened_files
        }),
        ("partial_screened_bytes", |value| {
            value.partial_screened_bytes
        }),
        ("full_hash_request_bytes", |value| {
            value.full_hash_request_bytes
        }),
        ("full_hash_satisfied_files", |value| {
            value.full_hash_satisfied_files
        }),
        ("full_hash_satisfied_bytes", |value| {
            value.full_hash_satisfied_bytes
        }),
        ("full_hash_failed_files", |value| {
            value.full_hash_failed_files
        }),
        ("full_hash_failed_bytes", |value| {
            value.full_hash_failed_bytes
        }),
        ("hash_pipeline_resolved_files", |value| {
            value.hash_pipeline_resolved_files
        }),
        ("hash_pipeline_resolved_bytes", |value| {
            value.hash_pipeline_resolved_bytes
        }),
        ("confirmed_logical_bytes", |value| {
            value.confirmed_logical_bytes
        }),
    ];

    fn validate(&self, counters: &ScanCounters) -> Result<(), ProgressContractError> {
        exact_sum(
            self.partial_screened_files,
            counters.partial_hashes_succeeded,
            counters.partial_hashes_failed,
            "partial-screened files must equal successful plus failed partial hashes",
        )?;
        bound(
            self.partial_screened_bytes,
            counters.candidate_bytes,
            "partial-screened logical bytes cannot exceed candidate bytes",
        )?;
        bound(
            self.full_hash_request_bytes,
            counters.partial_collision_bytes,
            "full-hash request bytes cannot exceed partial-collision bytes",
        )?;
        let full_hash_outcome_files = checked_sum(
            self.full_hash_satisfied_files,
            self.full_hash_failed_files,
            "full-hash outcome file count overflow",
        )?;
        bound(
            full_hash_outcome_files,
            counters.full_hash_requests,
            "full-hash outcomes cannot exceed requests",
        )?;
        let full_hash_outcome_bytes = checked_sum(
            self.full_hash_satisfied_bytes,
            self.full_hash_failed_bytes,
            "full-hash outcome byte count overflow",
        )?;
        bound(
            full_hash_outcome_bytes,
            self.full_hash_request_bytes,
            "full-hash outcome bytes cannot exceed request bytes",
        )?;
        let satisfied_from_cache_or_read = checked_sum(
            counters.full_hash_cache_hits,
            counters.full_hash_content_reads_completed,
            "full-hash satisfied file count overflow",
        )?;
        exact(
            self.full_hash_satisfied_files,
            satisfied_from_cache_or_read,
            "full-hash satisfied files must equal cache hits plus completed content reads",
        )?;
        bound(
            counters.full_hash_content_reads_failed,
            self.full_hash_failed_files,
            "failed full-content reads cannot exceed failed full-hash requests",
        )?;
        bound(
            self.hash_pipeline_resolved_files,
            counters.candidate_files,
            "resolved hash-pipeline files cannot exceed candidates",
        )?;
        bound(
            self.hash_pipeline_resolved_files,
            self.partial_screened_files,
            "resolved hash-pipeline files cannot exceed partial-screened files",
        )?;
        bound(
            self.hash_pipeline_resolved_bytes,
            counters.candidate_bytes,
            "resolved hash-pipeline bytes cannot exceed candidates",
        )?;
        bound(
            self.hash_pipeline_resolved_bytes,
            self.partial_screened_bytes,
            "resolved hash-pipeline bytes cannot exceed partial-screened bytes",
        )?;
        let expected_resolved_files = checked_sum(
            self.partial_screened_files
                .checked_sub(counters.full_hash_requests)
                .ok_or(ProgressContractError::Invariant(
                    "full-hash requests cannot exceed partial-screened files",
                ))?,
            full_hash_outcome_files,
            "resolved hash-pipeline file count overflow",
        )?;
        bound(
            self.hash_pipeline_resolved_files,
            expected_resolved_files,
            "resolved hash-pipeline files cannot exceed classified screened and full-hash outcomes",
        )?;
        let expected_resolved_bytes = checked_sum(
            self.partial_screened_bytes
                .checked_sub(self.full_hash_request_bytes)
                .ok_or(ProgressContractError::Invariant(
                    "full-hash request bytes cannot exceed partial-screened bytes",
                ))?,
            full_hash_outcome_bytes,
            "resolved hash-pipeline byte count overflow",
        )?;
        bound(
            self.hash_pipeline_resolved_bytes,
            expected_resolved_bytes,
            "resolved hash-pipeline bytes cannot exceed classified screened and full-hash outcomes",
        )?;
        bound(
            counters.confirmed_logical_copies,
            self.full_hash_satisfied_files,
            "confirmed logical copies cannot exceed satisfied full hashes",
        )?;
        bound(
            self.confirmed_logical_bytes,
            self.full_hash_satisfied_bytes,
            "confirmed logical bytes cannot exceed satisfied full-hash bytes",
        )?;
        bound(
            counters.recoverable_bytes,
            self.confirmed_logical_bytes,
            "recoverable bytes cannot exceed confirmed logical bytes",
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActiveDeviceProgress {
    Unavailable {
        reason: ActiveDeviceUnavailableReason,
    },
    One {
        device_key: String,
    },
    Multiple {
        device_keys: Vec<String>,
    },
}

impl ActiveDeviceProgress {
    fn validate(&self) -> Result<(), ProgressContractError> {
        let keys = match self {
            Self::Unavailable { .. } => return Ok(()),
            Self::One { device_key } => std::slice::from_ref(device_key),
            Self::Multiple { device_keys } => {
                if !(2..=MAX_ACTIVE_PROGRESS_DEVICES).contains(&device_keys.len()) {
                    return Err(ProgressContractError::Invariant(
                        "multiple active devices must contain between two and 64 keys",
                    ));
                }
                device_keys.as_slice()
            }
        };
        let mut unique = HashSet::with_capacity(keys.len());
        for key in keys {
            if key.trim().is_empty() || key.len() > 256 {
                return Err(ProgressContractError::Invariant(
                    "active device keys must contain 1 through 256 non-whitespace bytes",
                ));
            }
            if !unique.insert(key) {
                return Err(ProgressContractError::Invariant(
                    "active device keys must be unique",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveDeviceUnavailableReason {
    NoActiveIo,
    MappingUnavailable,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgressObservation {
    pub progress_contract_version: u32,
    pub metrics_contract_version: u32,
    pub monotonic_nanos: u64,
    pub phase: TelemetryPhase,
    pub phase_started_monotonic_nanos: u64,
    pub candidate_totals_known: bool,
    pub final_results_complete: bool,
    pub counters: ScanCounters,
    pub logical: ProgressLogicalCounters,
    pub active_devices: ActiveDeviceProgress,
}

impl ProgressObservation {
    fn validate(&self) -> Result<(), ProgressContractError> {
        if self.progress_contract_version != PROGRESS_CONTRACT_VERSION {
            return Err(ProgressContractError::UnsupportedVersion {
                expected: PROGRESS_CONTRACT_VERSION,
                actual: self.progress_contract_version,
            });
        }
        if self.metrics_contract_version != METRICS_CONTRACT_VERSION {
            return Err(ProgressContractError::UnsupportedMetricsVersion {
                expected: METRICS_CONTRACT_VERSION,
                actual: self.metrics_contract_version,
            });
        }
        if self.phase == TelemetryPhase::Overall {
            return Err(ProgressContractError::Invariant(
                "overall is not a live progress phase",
            ));
        }
        if self.phase == TelemetryPhase::FullHashing {
            return Err(ProgressContractError::Invariant(
                "full_hashing is reserved until the producer has a truthful global phase",
            ));
        }
        if self.phase_started_monotonic_nanos > self.monotonic_nanos {
            return Err(ProgressContractError::Invariant(
                "phase start cannot follow the observation",
            ));
        }
        self.counters.validate()?;
        self.logical.validate(&self.counters)?;
        self.active_devices.validate()?;
        if self.final_results_complete && !self.candidate_totals_known {
            return Err(ProgressContractError::Invariant(
                "final results require known candidate totals",
            ));
        }
        if self.final_results_complete && self.phase != TelemetryPhase::Finalizing {
            return Err(ProgressContractError::Invariant(
                "final results are complete only in the finalizing phase",
            ));
        }
        if self.phase != TelemetryPhase::Discovering && !self.candidate_totals_known {
            return Err(ProgressContractError::Invariant(
                "post-discovery phases require known candidate totals",
            ));
        }
        if self.final_results_complete
            && (self.logical.hash_pipeline_resolved_files != self.counters.candidate_files
                || self.logical.hash_pipeline_resolved_bytes != self.counters.candidate_bytes)
        {
            return Err(ProgressContractError::Invariant(
                "final results require the hash pipeline to be fully resolved",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgressRate {
    /// Thousandths of a logical file per second; 1,000 means 1.000 files/s.
    pub files_per_second_millis: u64,
    /// Actual physical bytes read per second, never logical candidate bytes.
    pub physical_bytes_per_second: u64,
    pub window_nanos: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressRateUnavailableReason {
    NoElapsedTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgressRateValue {
    Available {
        rate: ProgressRate,
    },
    Unavailable {
        reason: ProgressRateUnavailableReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgressRates {
    pub cumulative: ProgressRateValue,
    pub recent: ProgressRateValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemainingWorkStage {
    HashPipeline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemainingKnownWork {
    pub stage: RemainingWorkStage,
    pub files: u64,
    pub logical_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EtaUnavailableReason {
    WorkNotYetKnown,
    WindowWarming,
    NoRecentProgress,
    UnstableRate,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgressEta {
    Available {
        stage: RemainingWorkStage,
        remaining_logical_bytes: u64,
        logical_bytes_per_second_millis: u64,
        estimated_seconds: u64,
        window_nanos: u64,
    },
    Unavailable {
        reason: EtaUnavailableReason,
    },
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateFunnelProgress {
    pub discovered: ProgressQuantity,
    pub metadata_resolved: ProgressQuantity,
    pub hash_pipeline_candidates: ProgressQuantity,
    pub partial_screened: ProgressQuantity,
    pub selected_for_full_hash: ProgressQuantity,
    pub full_hash_satisfied: ProgressQuantity,
    pub finalized_duplicates: ProgressQuantity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScanProgressSnapshot {
    pub progress_contract_version: u32,
    pub metrics_contract_version: u32,
    pub revision: u64,
    pub monotonic_nanos: u64,
    pub phase: TelemetryPhase,
    pub phase_elapsed_nanos: u64,
    pub counters: ScanCounters,
    pub logical: ProgressLogicalCounters,
    pub funnel: CandidateFunnelProgress,
    pub partial_read_rates: ProgressRates,
    pub full_read_rates: ProgressRates,
    pub cache_hit_rate_basis_points: Option<u32>,
    pub warning_count: u64,
    pub active_devices: ActiveDeviceProgress,
    pub remaining_known_work: Option<RemainingKnownWork>,
    pub eta: ProgressEta,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProgressContractError {
    #[error("unsupported progress contract version {actual}; expected {expected}")]
    UnsupportedVersion { expected: u32, actual: u32 },
    #[error("unsupported metrics contract version {actual}; expected {expected}")]
    UnsupportedMetricsVersion { expected: u32, actual: u32 },
    #[error("progress invariant failed: {0}")]
    Invariant(&'static str),
    #[error("progress counter regressed: {metric}")]
    CounterRegression { metric: &'static str },
    #[error(transparent)]
    Metric(#[from] MetricInvariantError),
}

#[derive(Debug, Clone)]
struct RatePoint {
    monotonic_nanos: u64,
    phase: TelemetryPhase,
    partial_files: u64,
    partial_bytes: u64,
    full_files: u64,
    full_bytes: u64,
    resolved_logical_bytes: u64,
}

impl RatePoint {
    fn from_observation(observation: &ProgressObservation) -> Result<Self, ProgressContractError> {
        Ok(Self {
            monotonic_nanos: observation.monotonic_nanos,
            phase: observation.phase,
            partial_files: observation.logical.partial_screened_files,
            partial_bytes: observation.counters.partial_hash_bytes_read,
            full_files: checked_sum(
                observation.counters.full_hash_content_reads_completed,
                observation.counters.full_hash_content_reads_failed,
                "full-content read outcome file count overflow",
            )?,
            full_bytes: observation.counters.full_hash_bytes_read,
            resolved_logical_bytes: observation.logical.hash_pipeline_resolved_bytes,
        })
    }
}

#[derive(Debug, Default)]
pub struct ProgressReducer {
    revision: u64,
    first_rate_point: Option<RatePoint>,
    previous: Option<ProgressObservation>,
    rate_points: VecDeque<RatePoint>,
}

impl ProgressReducer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(
        &mut self,
        observation: ProgressObservation,
    ) -> Result<ScanProgressSnapshot, ProgressContractError> {
        observation.validate()?;
        if let Some(previous) = &self.previous {
            validate_transition(previous, &observation)?;
        }
        let point = RatePoint::from_observation(&observation)?;
        let next_revision =
            self.revision
                .checked_add(1)
                .ok_or(ProgressContractError::Invariant(
                    "progress revision overflow",
                ))?;

        if self.first_rate_point.is_none() {
            self.first_rate_point = Some(point.clone());
        }
        push_rate_point(&mut self.rate_points, point);
        self.previous = Some(observation.clone());
        self.revision = next_revision;
        Ok(self.project(&observation))
    }

    fn project(&self, observation: &ProgressObservation) -> ScanProgressSnapshot {
        let phase_elapsed_nanos = observation
            .monotonic_nanos
            .saturating_sub(observation.phase_started_monotonic_nanos);
        let first_point = self
            .first_rate_point
            .as_ref()
            .expect("observation adds a cumulative rate point");
        let current_point = self
            .rate_points
            .back()
            .expect("observation adds a rate point");
        let recent_point = recent_anchor(
            &self.rate_points,
            observation.monotonic_nanos,
            observation.phase,
        )
        .unwrap_or(current_point);
        let partial_read_rates = ProgressRates {
            cumulative: rate_value(
                first_point,
                current_point,
                |point| point.partial_files,
                |point| point.partial_bytes,
            ),
            recent: rate_value(
                recent_point,
                current_point,
                |point| point.partial_files,
                |point| point.partial_bytes,
            ),
        };
        let full_read_rates = ProgressRates {
            cumulative: rate_value(
                first_point,
                current_point,
                |point| point.full_files,
                |point| point.full_bytes,
            ),
            recent: rate_value(
                recent_point,
                current_point,
                |point| point.full_files,
                |point| point.full_bytes,
            ),
        };
        let remaining_known_work = remaining_work(observation);
        ScanProgressSnapshot {
            progress_contract_version: PROGRESS_CONTRACT_VERSION,
            metrics_contract_version: METRICS_CONTRACT_VERSION,
            revision: self.revision,
            monotonic_nanos: observation.monotonic_nanos,
            phase: observation.phase,
            phase_elapsed_nanos,
            counters: observation.counters.clone(),
            logical: observation.logical,
            funnel: funnel(observation),
            partial_read_rates,
            full_read_rates,
            cache_hit_rate_basis_points: cache_hit_rate(&observation.counters),
            warning_count: observation.counters.warnings,
            active_devices: observation.active_devices.clone(),
            remaining_known_work,
            eta: eta(&self.rate_points, observation, remaining_known_work),
        }
    }
}

fn validate_transition(
    previous: &ProgressObservation,
    proposed: &ProgressObservation,
) -> Result<(), ProgressContractError> {
    if proposed.monotonic_nanos < previous.monotonic_nanos {
        return Err(ProgressContractError::Invariant(
            "observation time cannot regress",
        ));
    }
    if proposed.phase == previous.phase
        && proposed.phase_started_monotonic_nanos != previous.phase_started_monotonic_nanos
    {
        return Err(ProgressContractError::Invariant(
            "phase start cannot change within one phase",
        ));
    }
    if proposed.phase != previous.phase
        && proposed.phase_started_monotonic_nanos < previous.monotonic_nanos
    {
        return Err(ProgressContractError::Invariant(
            "a new phase cannot start before the preceding observation",
        ));
    }
    if phase_order(proposed.phase) < phase_order(previous.phase) {
        return Err(ProgressContractError::Invariant(
            "live progress phase cannot regress",
        ));
    }
    if previous.candidate_totals_known && !proposed.candidate_totals_known {
        return Err(ProgressContractError::Invariant(
            "candidate-total knowledge cannot regress",
        ));
    }
    if previous.final_results_complete && !proposed.final_results_complete {
        return Err(ProgressContractError::Invariant(
            "final-result knowledge cannot regress",
        ));
    }
    if previous.candidate_totals_known
        && (previous.counters.candidate_size_buckets != proposed.counters.candidate_size_buckets
            || previous.counters.candidate_files != proposed.counters.candidate_files
            || previous.counters.candidate_bytes != proposed.counters.candidate_bytes)
    {
        return Err(ProgressContractError::Invariant(
            "known candidate totals cannot change",
        ));
    }
    for kind in CounterKind::ALL {
        if proposed.counters.value(kind) < previous.counters.value(kind) {
            return Err(ProgressContractError::CounterRegression {
                metric: kind.as_str(),
            });
        }
    }
    for (metric, value) in ProgressLogicalCounters::ALL {
        if value(&proposed.logical) < value(&previous.logical) {
            return Err(ProgressContractError::CounterRegression { metric });
        }
    }
    Ok(())
}

fn phase_order(phase: TelemetryPhase) -> u8 {
    match phase {
        TelemetryPhase::Overall => 0,
        TelemetryPhase::Discovering => 1,
        TelemetryPhase::CandidateScreening => 2,
        TelemetryPhase::FullHashing => 3,
        TelemetryPhase::Persisting => 4,
        TelemetryPhase::AnalyzingFolders => 5,
        TelemetryPhase::Finalizing => 6,
    }
}

fn push_rate_point(points: &mut VecDeque<RatePoint>, point: RatePoint) {
    if points
        .back()
        .is_some_and(|last| point.monotonic_nanos == last.monotonic_nanos)
    {
        points.pop_back();
        points.push_back(point);
        return;
    }
    if points.back().is_some_and(|last| {
        point.monotonic_nanos / PROGRESS_RATE_POINT_MIN_INTERVAL_NANOS
            == last.monotonic_nanos / PROGRESS_RATE_POINT_MIN_INTERVAL_NANOS
    }) {
        points.pop_back();
    }
    points.push_back(point);
    while points.len() > MAX_PROGRESS_RATE_POINTS {
        points.pop_front();
    }
    if let Some(last) = points.back() {
        let cutoff = last
            .monotonic_nanos
            .saturating_sub(RECENT_PROGRESS_RATE_WINDOW_NANOS);
        while points.len() > 1
            && points
                .front()
                .is_some_and(|point| point.monotonic_nanos < cutoff)
        {
            points.pop_front();
        }
    }
}

fn recent_anchor(
    points: &VecDeque<RatePoint>,
    now: u64,
    phase: TelemetryPhase,
) -> Option<&RatePoint> {
    let cutoff = now.saturating_sub(RECENT_PROGRESS_RATE_WINDOW_NANOS);
    points
        .iter()
        .find(|point| point.phase == phase && point.monotonic_nanos >= cutoff)
}

fn rate_value(
    start: &RatePoint,
    end: &RatePoint,
    files: impl Fn(&RatePoint) -> u64,
    bytes: impl Fn(&RatePoint) -> u64,
) -> ProgressRateValue {
    let elapsed = end.monotonic_nanos.saturating_sub(start.monotonic_nanos);
    if elapsed == 0 {
        return ProgressRateValue::Unavailable {
            reason: ProgressRateUnavailableReason::NoElapsedTime,
        };
    }
    ProgressRateValue::Available {
        rate: ProgressRate {
            files_per_second_millis: scaled_rate(
                files(end).saturating_sub(files(start)),
                elapsed,
                1_000,
            ),
            physical_bytes_per_second: scaled_rate(
                bytes(end).saturating_sub(bytes(start)),
                elapsed,
                1,
            ),
            window_nanos: elapsed,
        },
    }
}

fn scaled_rate(value: u64, elapsed_nanos: u64, scale: u64) -> u64 {
    let projected = u128::from(value)
        .saturating_mul(u128::from(scale))
        .saturating_mul(NANOS_PER_SECOND)
        / u128::from(elapsed_nanos);
    u64::try_from(projected).unwrap_or(u64::MAX)
}

fn funnel(observation: &ProgressObservation) -> CandidateFunnelProgress {
    CandidateFunnelProgress {
        discovered: ProgressQuantity {
            files: observation.counters.discovered_files,
            logical_bytes: observation.counters.discovered_bytes,
        },
        metadata_resolved: ProgressQuantity {
            files: observation.counters.metadata_resolved_files,
            logical_bytes: observation.counters.metadata_resolved_bytes,
        },
        hash_pipeline_candidates: ProgressQuantity {
            files: observation.counters.candidate_files,
            logical_bytes: observation.counters.candidate_bytes,
        },
        partial_screened: ProgressQuantity {
            files: observation.logical.partial_screened_files,
            logical_bytes: observation.logical.partial_screened_bytes,
        },
        selected_for_full_hash: ProgressQuantity {
            files: observation.counters.full_hash_requests,
            logical_bytes: observation.logical.full_hash_request_bytes,
        },
        full_hash_satisfied: ProgressQuantity {
            files: observation.logical.full_hash_satisfied_files,
            logical_bytes: observation.logical.full_hash_satisfied_bytes,
        },
        finalized_duplicates: ProgressQuantity {
            files: observation.counters.confirmed_logical_copies,
            logical_bytes: observation.logical.confirmed_logical_bytes,
        },
    }
}

fn cache_hit_rate(counters: &ScanCounters) -> Option<u32> {
    let outcomes = u128::from(counters.full_hash_cache_hits)
        + u128::from(counters.full_hash_cache_misses)
        + u128::from(counters.full_hash_cache_errors);
    (outcomes > 0).then(|| {
        u32::try_from(u128::from(counters.full_hash_cache_hits) * BASIS_POINTS / outcomes)
            .expect("basis points fit in u32")
    })
}

fn remaining_work(observation: &ProgressObservation) -> Option<RemainingKnownWork> {
    observation
        .candidate_totals_known
        .then(|| RemainingKnownWork {
            stage: RemainingWorkStage::HashPipeline,
            files: observation
                .counters
                .candidate_files
                .checked_sub(observation.logical.hash_pipeline_resolved_files)
                .expect("validated resolved files"),
            logical_bytes: observation
                .counters
                .candidate_bytes
                .checked_sub(observation.logical.hash_pipeline_resolved_bytes)
                .expect("validated resolved bytes"),
        })
}

fn eta(
    points: &VecDeque<RatePoint>,
    observation: &ProgressObservation,
    remaining: Option<RemainingKnownWork>,
) -> ProgressEta {
    let Some(remaining) = remaining else {
        return ProgressEta::Unavailable {
            reason: EtaUnavailableReason::WorkNotYetKnown,
        };
    };
    if remaining.files == 0 && remaining.logical_bytes == 0 {
        return if observation.final_results_complete {
            ProgressEta::Complete
        } else {
            ProgressEta::Unavailable {
                reason: EtaUnavailableReason::NotApplicable,
            }
        };
    }
    if observation.phase != TelemetryPhase::CandidateScreening {
        return ProgressEta::Unavailable {
            reason: EtaUnavailableReason::NotApplicable,
        };
    }
    let Some(current) = points.back() else {
        return ProgressEta::Unavailable {
            reason: EtaUnavailableReason::WindowWarming,
        };
    };
    let interval_start_time = current
        .monotonic_nanos
        .saturating_sub(ETA_MIN_OBSERVATION_SPAN_NANOS);
    let midpoint_time = current
        .monotonic_nanos
        .saturating_sub(ETA_MIN_INTERVAL_NANOS);
    let Some(first) = points.iter().rev().find(|point| {
        point.phase == observation.phase && point.monotonic_nanos <= interval_start_time
    }) else {
        return ProgressEta::Unavailable {
            reason: EtaUnavailableReason::WindowWarming,
        };
    };
    let Some(midpoint) = points
        .iter()
        .rev()
        .find(|point| point.phase == observation.phase && point.monotonic_nanos <= midpoint_time)
    else {
        return ProgressEta::Unavailable {
            reason: EtaUnavailableReason::WindowWarming,
        };
    };
    let first_span = midpoint
        .monotonic_nanos
        .saturating_sub(first.monotonic_nanos);
    let second_span = current
        .monotonic_nanos
        .saturating_sub(midpoint.monotonic_nanos);
    if first_span < ETA_MIN_INTERVAL_NANOS || second_span < ETA_MIN_INTERVAL_NANOS {
        return ProgressEta::Unavailable {
            reason: EtaUnavailableReason::WindowWarming,
        };
    }
    let first_delta = midpoint
        .resolved_logical_bytes
        .saturating_sub(first.resolved_logical_bytes);
    let second_delta = current
        .resolved_logical_bytes
        .saturating_sub(midpoint.resolved_logical_bytes);
    if first_delta == 0 || second_delta == 0 {
        return ProgressEta::Unavailable {
            reason: EtaUnavailableReason::NoRecentProgress,
        };
    }
    let first_cross_rate = u128::from(first_delta) * u128::from(second_span);
    let second_cross_rate = u128::from(second_delta) * u128::from(first_span);
    let slower = first_cross_rate.min(second_cross_rate);
    let faster = first_cross_rate.max(second_cross_rate);
    let stability_basis_points = u128::from(ETA_RATE_STABILITY_MIN_BASIS_POINTS);
    let required_slower = (faster / BASIS_POINTS) * stability_basis_points
        + ((faster % BASIS_POINTS) * stability_basis_points).div_ceil(BASIS_POINTS);
    if slower < required_slower {
        return ProgressEta::Unavailable {
            reason: EtaUnavailableReason::UnstableRate,
        };
    }
    let total_span = first_span + second_span;
    let total_delta = first_delta.saturating_add(second_delta);
    let logical_bytes_per_second_millis = scaled_rate(total_delta, total_span, 1_000);
    let estimated_nanos = u128::from(remaining.logical_bytes)
        .saturating_mul(u128::from(total_span))
        .div_ceil(u128::from(total_delta));
    let estimated_seconds =
        u64::try_from(estimated_nanos.div_ceil(NANOS_PER_SECOND)).unwrap_or(u64::MAX);
    ProgressEta::Available {
        stage: remaining.stage,
        remaining_logical_bytes: remaining.logical_bytes,
        logical_bytes_per_second_millis,
        estimated_seconds,
        window_nanos: total_span,
    }
}

fn checked_sum(left: u64, right: u64, message: &'static str) -> Result<u64, ProgressContractError> {
    left.checked_add(right)
        .ok_or(ProgressContractError::Invariant(message))
}

fn exact_sum(
    expected: u64,
    left: u64,
    right: u64,
    message: &'static str,
) -> Result<(), ProgressContractError> {
    exact(expected, checked_sum(left, right, message)?, message)
}

fn exact(value: u64, expected: u64, message: &'static str) -> Result<(), ProgressContractError> {
    if value == expected {
        Ok(())
    } else {
        Err(ProgressContractError::Invariant(message))
    }
}

fn bound(value: u64, limit: u64, message: &'static str) -> Result<(), ProgressContractError> {
    if value <= limit {
        Ok(())
    } else {
        Err(ProgressContractError::Invariant(message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_history_is_bounded_under_dense_observations() {
        let mut points = VecDeque::new();
        for (monotonic_nanos, value) in [(0, 0), (10_000_000, 1), (99_000_000, 2)] {
            push_rate_point(
                &mut points,
                RatePoint {
                    monotonic_nanos,
                    phase: TelemetryPhase::CandidateScreening,
                    partial_files: value,
                    partial_bytes: value,
                    full_files: value,
                    full_bytes: value,
                    resolved_logical_bytes: value,
                },
            );
        }
        assert_eq!(points.len(), 1);
        assert_eq!(points.back().unwrap().monotonic_nanos, 99_000_000);
        assert_eq!(points.back().unwrap().partial_files, 2);
        points.clear();

        for index in 0..10_000_u64 {
            push_rate_point(
                &mut points,
                RatePoint {
                    monotonic_nanos: index * 10_000_000,
                    phase: TelemetryPhase::CandidateScreening,
                    partial_files: index,
                    partial_bytes: index,
                    full_files: index,
                    full_bytes: index,
                    resolved_logical_bytes: index,
                },
            );
        }
        assert!(points.len() <= MAX_PROGRESS_RATE_POINTS);
        assert!(
            points.back().unwrap().monotonic_nanos - points.front().unwrap().monotonic_nanos
                <= RECENT_PROGRESS_RATE_WINDOW_NANOS
        );
    }
}
