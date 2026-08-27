use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const METRICS_CONTRACT_VERSION: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryRunState {
    Pending,
    Running,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
    Interrupted,
}

impl TelemetryRunState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Cancelling => "cancelling",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Failed | Self::Interrupted
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryPhaseState {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed,
    Interrupted,
}

impl TelemetryPhaseState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryPhase {
    Overall,
    Discovering,
    CandidateScreening,
    FullHashing,
    Persisting,
    AnalyzingFolders,
    Finalizing,
}

impl TelemetryPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overall => "overall",
            Self::Discovering => "discovering",
            Self::CandidateScreening => "candidate_screening",
            Self::FullHashing => "full_hashing",
            Self::Persisting => "persisting",
            Self::AnalyzingFolders => "analyzing_folders",
            Self::Finalizing => "finalizing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterKind {
    DiscoveredFiles,
    DiscoveredBytes,
    ZeroByteFiles,
    HardLinkAliasFiles,
    HardLinkAliasBytes,
    SizeBuckets,
    SingletonSizeBuckets,
    SingletonSizeFiles,
    SingletonSizeBytes,
    CandidateSizeBuckets,
    CandidateFiles,
    CandidateBytes,
    DuplicateCandidateSizeBuckets,
    DuplicateCandidateFiles,
    DuplicateCandidateBytes,
    MetadataResolvedFiles,
    MetadataResolvedBytes,
    PartialHashesAttempted,
    PartialHashesSucceeded,
    PartialHashesFailed,
    PartialHashBytesRead,
    PartialHashCacheHits,
    PartialHashCacheMisses,
    PartialHashCacheErrors,
    PartialHashCacheStores,
    PartialCollisionBuckets,
    PartialCollisionFiles,
    PartialCollisionBytes,
    FullHashRequests,
    FullHashCacheHits,
    FullHashCacheMisses,
    FullHashCacheErrors,
    FullHashCacheStores,
    FullHashContentReadsStarted,
    FullHashContentReadsCompleted,
    FullHashContentReadsFailed,
    FullHashBytesRead,
    ConfirmedDuplicateGroups,
    ConfirmedLogicalCopies,
    ConfirmedPhysicalItems,
    RecoverableBytes,
    Warnings,
    CancelChecks,
    CancelledWorkItems,
    TelemetrySamplesLost,
    TelemetryFlushErrors,
    UnavailableCounters,
}

impl CounterKind {
    pub const ALL: [Self; 47] = [
        Self::DiscoveredFiles,
        Self::DiscoveredBytes,
        Self::ZeroByteFiles,
        Self::HardLinkAliasFiles,
        Self::HardLinkAliasBytes,
        Self::SizeBuckets,
        Self::SingletonSizeBuckets,
        Self::SingletonSizeFiles,
        Self::SingletonSizeBytes,
        Self::CandidateSizeBuckets,
        Self::CandidateFiles,
        Self::CandidateBytes,
        Self::DuplicateCandidateSizeBuckets,
        Self::DuplicateCandidateFiles,
        Self::DuplicateCandidateBytes,
        Self::MetadataResolvedFiles,
        Self::MetadataResolvedBytes,
        Self::PartialHashesAttempted,
        Self::PartialHashesSucceeded,
        Self::PartialHashesFailed,
        Self::PartialHashBytesRead,
        Self::PartialHashCacheHits,
        Self::PartialHashCacheMisses,
        Self::PartialHashCacheErrors,
        Self::PartialHashCacheStores,
        Self::PartialCollisionBuckets,
        Self::PartialCollisionFiles,
        Self::PartialCollisionBytes,
        Self::FullHashRequests,
        Self::FullHashCacheHits,
        Self::FullHashCacheMisses,
        Self::FullHashCacheErrors,
        Self::FullHashCacheStores,
        Self::FullHashContentReadsStarted,
        Self::FullHashContentReadsCompleted,
        Self::FullHashContentReadsFailed,
        Self::FullHashBytesRead,
        Self::ConfirmedDuplicateGroups,
        Self::ConfirmedLogicalCopies,
        Self::ConfirmedPhysicalItems,
        Self::RecoverableBytes,
        Self::Warnings,
        Self::CancelChecks,
        Self::CancelledWorkItems,
        Self::TelemetrySamplesLost,
        Self::TelemetryFlushErrors,
        Self::UnavailableCounters,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DiscoveredFiles => "discovered_files",
            Self::DiscoveredBytes => "discovered_bytes",
            Self::ZeroByteFiles => "zero_byte_files",
            Self::HardLinkAliasFiles => "hard_link_alias_files",
            Self::HardLinkAliasBytes => "hard_link_alias_bytes",
            Self::SizeBuckets => "size_buckets",
            Self::SingletonSizeBuckets => "singleton_size_buckets",
            Self::SingletonSizeFiles => "singleton_size_files",
            Self::SingletonSizeBytes => "singleton_size_bytes",
            Self::CandidateSizeBuckets => "candidate_size_buckets",
            Self::CandidateFiles => "candidate_files",
            Self::CandidateBytes => "candidate_bytes",
            Self::DuplicateCandidateSizeBuckets => "duplicate_candidate_size_buckets",
            Self::DuplicateCandidateFiles => "duplicate_candidate_files",
            Self::DuplicateCandidateBytes => "duplicate_candidate_bytes",
            Self::MetadataResolvedFiles => "metadata_resolved_files",
            Self::MetadataResolvedBytes => "metadata_resolved_bytes",
            Self::PartialHashesAttempted => "partial_hashes_attempted",
            Self::PartialHashesSucceeded => "partial_hashes_succeeded",
            Self::PartialHashesFailed => "partial_hashes_failed",
            Self::PartialHashBytesRead => "partial_hash_bytes_read",
            Self::PartialHashCacheHits => "partial_hash_cache_hits",
            Self::PartialHashCacheMisses => "partial_hash_cache_misses",
            Self::PartialHashCacheErrors => "partial_hash_cache_errors",
            Self::PartialHashCacheStores => "partial_hash_cache_stores",
            Self::PartialCollisionBuckets => "partial_collision_buckets",
            Self::PartialCollisionFiles => "partial_collision_files",
            Self::PartialCollisionBytes => "partial_collision_bytes",
            Self::FullHashRequests => "full_hash_requests",
            Self::FullHashCacheHits => "full_hash_cache_hits",
            Self::FullHashCacheMisses => "full_hash_cache_misses",
            Self::FullHashCacheErrors => "full_hash_cache_errors",
            Self::FullHashCacheStores => "full_hash_cache_stores",
            Self::FullHashContentReadsStarted => "full_hash_content_reads_started",
            Self::FullHashContentReadsCompleted => "full_hash_content_reads_completed",
            Self::FullHashContentReadsFailed => "full_hash_content_reads_failed",
            Self::FullHashBytesRead => "full_hash_bytes_read",
            Self::ConfirmedDuplicateGroups => "confirmed_duplicate_groups",
            Self::ConfirmedLogicalCopies => "confirmed_logical_copies",
            Self::ConfirmedPhysicalItems => "confirmed_physical_items",
            Self::RecoverableBytes => "recoverable_bytes",
            Self::Warnings => "warnings",
            Self::CancelChecks => "cancel_checks",
            Self::CancelledWorkItems => "cancelled_work_items",
            Self::TelemetrySamplesLost => "telemetry_samples_lost",
            Self::TelemetryFlushErrors => "telemetry_flush_errors",
            Self::UnavailableCounters => "unavailable_counters",
        }
    }
}

/// Monotonic cumulative scan counters for metrics contract v3.
///
/// `discovered_files` includes logical zero-byte files and hard-link aliases. Size-bucket and hash
/// counters describe first-physical, non-empty files only. `candidate_*` records files admitted to
/// the content-hash pipeline by the producing engine. Pre-SOP5 metrics-v2 history can overlap the
/// singleton classification; SOP5 and later producers admit only `duplicate_candidate_*` and put
/// singleton work that avoided content I/O in `metadata_resolved_*`.
/// Cache lookup outcomes are exclusive:
/// an error means lookup degraded to a content read and is not also a miss. Full-content read
/// counters exclude cache hits.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCounters {
    pub discovered_files: u64,
    pub discovered_bytes: u64,
    pub zero_byte_files: u64,
    pub hard_link_alias_files: u64,
    pub hard_link_alias_bytes: u64,
    pub size_buckets: u64,
    pub singleton_size_buckets: u64,
    pub singleton_size_files: u64,
    pub singleton_size_bytes: u64,
    pub candidate_size_buckets: u64,
    pub candidate_files: u64,
    pub candidate_bytes: u64,
    pub duplicate_candidate_size_buckets: u64,
    pub duplicate_candidate_files: u64,
    pub duplicate_candidate_bytes: u64,
    pub metadata_resolved_files: u64,
    pub metadata_resolved_bytes: u64,
    pub partial_hashes_attempted: u64,
    pub partial_hashes_succeeded: u64,
    pub partial_hashes_failed: u64,
    pub partial_hash_bytes_read: u64,
    pub partial_hash_cache_hits: u64,
    pub partial_hash_cache_misses: u64,
    pub partial_hash_cache_errors: u64,
    pub partial_hash_cache_stores: u64,
    pub partial_collision_buckets: u64,
    pub partial_collision_files: u64,
    pub partial_collision_bytes: u64,
    pub full_hash_requests: u64,
    pub full_hash_cache_hits: u64,
    pub full_hash_cache_misses: u64,
    pub full_hash_cache_errors: u64,
    pub full_hash_cache_stores: u64,
    pub full_hash_content_reads_started: u64,
    pub full_hash_content_reads_completed: u64,
    pub full_hash_content_reads_failed: u64,
    pub full_hash_bytes_read: u64,
    pub confirmed_duplicate_groups: u64,
    pub confirmed_logical_copies: u64,
    pub confirmed_physical_items: u64,
    pub recoverable_bytes: u64,
    pub warnings: u64,
    pub cancel_checks: u64,
    pub cancelled_work_items: u64,
    pub telemetry_samples_lost: u64,
    pub telemetry_flush_errors: u64,
    pub unavailable_counters: u64,
}

impl ScanCounters {
    pub const fn value(&self, kind: CounterKind) -> u64 {
        match kind {
            CounterKind::DiscoveredFiles => self.discovered_files,
            CounterKind::DiscoveredBytes => self.discovered_bytes,
            CounterKind::ZeroByteFiles => self.zero_byte_files,
            CounterKind::HardLinkAliasFiles => self.hard_link_alias_files,
            CounterKind::HardLinkAliasBytes => self.hard_link_alias_bytes,
            CounterKind::SizeBuckets => self.size_buckets,
            CounterKind::SingletonSizeBuckets => self.singleton_size_buckets,
            CounterKind::SingletonSizeFiles => self.singleton_size_files,
            CounterKind::SingletonSizeBytes => self.singleton_size_bytes,
            CounterKind::CandidateSizeBuckets => self.candidate_size_buckets,
            CounterKind::CandidateFiles => self.candidate_files,
            CounterKind::CandidateBytes => self.candidate_bytes,
            CounterKind::DuplicateCandidateSizeBuckets => self.duplicate_candidate_size_buckets,
            CounterKind::DuplicateCandidateFiles => self.duplicate_candidate_files,
            CounterKind::DuplicateCandidateBytes => self.duplicate_candidate_bytes,
            CounterKind::MetadataResolvedFiles => self.metadata_resolved_files,
            CounterKind::MetadataResolvedBytes => self.metadata_resolved_bytes,
            CounterKind::PartialHashesAttempted => self.partial_hashes_attempted,
            CounterKind::PartialHashesSucceeded => self.partial_hashes_succeeded,
            CounterKind::PartialHashesFailed => self.partial_hashes_failed,
            CounterKind::PartialHashBytesRead => self.partial_hash_bytes_read,
            CounterKind::PartialHashCacheHits => self.partial_hash_cache_hits,
            CounterKind::PartialHashCacheMisses => self.partial_hash_cache_misses,
            CounterKind::PartialHashCacheErrors => self.partial_hash_cache_errors,
            CounterKind::PartialHashCacheStores => self.partial_hash_cache_stores,
            CounterKind::PartialCollisionBuckets => self.partial_collision_buckets,
            CounterKind::PartialCollisionFiles => self.partial_collision_files,
            CounterKind::PartialCollisionBytes => self.partial_collision_bytes,
            CounterKind::FullHashRequests => self.full_hash_requests,
            CounterKind::FullHashCacheHits => self.full_hash_cache_hits,
            CounterKind::FullHashCacheMisses => self.full_hash_cache_misses,
            CounterKind::FullHashCacheErrors => self.full_hash_cache_errors,
            CounterKind::FullHashCacheStores => self.full_hash_cache_stores,
            CounterKind::FullHashContentReadsStarted => self.full_hash_content_reads_started,
            CounterKind::FullHashContentReadsCompleted => self.full_hash_content_reads_completed,
            CounterKind::FullHashContentReadsFailed => self.full_hash_content_reads_failed,
            CounterKind::FullHashBytesRead => self.full_hash_bytes_read,
            CounterKind::ConfirmedDuplicateGroups => self.confirmed_duplicate_groups,
            CounterKind::ConfirmedLogicalCopies => self.confirmed_logical_copies,
            CounterKind::ConfirmedPhysicalItems => self.confirmed_physical_items,
            CounterKind::RecoverableBytes => self.recoverable_bytes,
            CounterKind::Warnings => self.warnings,
            CounterKind::CancelChecks => self.cancel_checks,
            CounterKind::CancelledWorkItems => self.cancelled_work_items,
            CounterKind::TelemetrySamplesLost => self.telemetry_samples_lost,
            CounterKind::TelemetryFlushErrors => self.telemetry_flush_errors,
            CounterKind::UnavailableCounters => self.unavailable_counters,
        }
    }

    pub fn validate(&self) -> Result<(), MetricInvariantError> {
        invariant(
            sum_leq(
                &[self.zero_byte_files, self.hard_link_alias_files],
                self.discovered_files,
            ),
            "excluded logical files cannot exceed discovered files",
        )?;
        invariant(
            self.singleton_size_files == self.singleton_size_buckets,
            "every singleton size bucket contains exactly one file",
        )?;
        invariant(
            sum_leq(
                &[
                    self.singleton_size_buckets,
                    self.duplicate_candidate_size_buckets,
                ],
                self.size_buckets,
            ),
            "classified size buckets cannot exceed total size buckets",
        )?;
        invariant(
            self.zero_byte_files
                .checked_add(self.hard_link_alias_files)
                .and_then(|excluded| self.discovered_files.checked_sub(excluded))
                .is_some_and(|physical_files| {
                    sum_leq(
                        &[self.singleton_size_files, self.duplicate_candidate_files],
                        physical_files,
                    )
                }),
            "classified physical files cannot exceed discovered non-empty physical files",
        )?;
        invariant(
            sum_leq(
                &[self.singleton_size_bytes, self.duplicate_candidate_bytes],
                self.discovered_bytes,
            ),
            "classified physical bytes cannot exceed discovered logical bytes",
        )?;
        invariant(
            self.candidate_size_buckets <= self.size_buckets,
            "hash-pipeline size buckets cannot exceed total size buckets",
        )?;
        invariant(
            self.duplicate_candidate_files <= self.candidate_files,
            "duplicate candidates cannot exceed hash-pipeline candidates",
        )?;
        invariant(
            self.duplicate_candidate_bytes <= self.candidate_bytes,
            "duplicate candidate bytes cannot exceed hash-pipeline candidate bytes",
        )?;
        invariant(
            self.metadata_resolved_files <= self.singleton_size_files,
            "metadata-resolved files cannot exceed singleton files",
        )?;
        invariant(
            self.metadata_resolved_bytes <= self.singleton_size_bytes,
            "metadata-resolved bytes cannot exceed singleton bytes",
        )?;
        invariant(
            self.partial_hashes_attempted <= self.candidate_files,
            "partial hash attempts cannot exceed candidate files",
        )?;
        invariant(
            sum_leq(
                &[self.partial_hashes_succeeded, self.partial_hashes_failed],
                self.partial_hashes_attempted,
            ),
            "partial hash outcomes cannot exceed attempts",
        )?;
        invariant(
            sum_leq(
                &[
                    self.partial_hash_cache_hits,
                    self.partial_hash_cache_misses,
                    self.partial_hash_cache_errors,
                ],
                self.partial_hashes_attempted,
            ),
            "partial cache lookup outcomes cannot exceed partial hash attempts",
        )?;
        invariant(
            self.partial_hash_cache_stores <= self.partial_hashes_succeeded,
            "partial cache stores require successful partial hashes",
        )?;
        invariant(
            self.partial_collision_files <= self.partial_hashes_succeeded,
            "partial collision files must have a successful partial hash",
        )?;
        invariant(
            self.full_hash_requests <= self.partial_collision_files,
            "full hash requests cannot exceed partial collision files",
        )?;
        invariant(
            sum_leq(
                &[
                    self.full_hash_cache_hits,
                    self.full_hash_cache_misses,
                    self.full_hash_cache_errors,
                ],
                self.full_hash_requests,
            ),
            "cache lookup outcomes cannot exceed full hash requests",
        )?;
        invariant(
            self.full_hash_requests
                .checked_sub(self.full_hash_cache_hits)
                .is_some_and(|read_eligible| self.full_hash_content_reads_started <= read_eligible),
            "content reads cannot include cache-hit requests",
        )?;
        invariant(
            sum_leq(
                &[
                    self.full_hash_content_reads_completed,
                    self.full_hash_content_reads_failed,
                ],
                self.full_hash_content_reads_started,
            ),
            "full hash read outcomes cannot exceed started reads",
        )?;
        invariant(
            self.full_hash_cache_stores <= self.full_hash_content_reads_completed,
            "cache stores require completed content reads",
        )?;
        invariant(
            self.confirmed_logical_copies <= self.full_hash_requests,
            "confirmed logical copies cannot exceed full hash requests",
        )?;
        invariant(
            self.confirmed_physical_items <= self.confirmed_logical_copies,
            "confirmed physical items cannot exceed logical copies",
        )
    }
}

fn invariant(condition: bool, message: &'static str) -> Result<(), MetricInvariantError> {
    condition.then_some(()).ok_or(MetricInvariantError(message))
}

fn sum_leq(values: &[u64], limit: u64) -> bool {
    values
        .iter()
        .try_fold(0_u64, |sum, value| sum.checked_add(*value))
        .is_some_and(|sum| sum <= limit)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("metric invariant failed: {0}")]
pub struct MetricInvariantError(pub &'static str);

/// One bounded host/process gauge sample. `None` means unavailable, never zero.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSample {
    pub sequence: u64,
    pub observed_unix_millis: i64,
    pub monotonic_nanos: u64,
    pub phase: Option<TelemetryPhase>,
    pub process_cpu_nanos: Option<u64>,
    pub process_private_bytes: Option<u64>,
    pub process_working_set_bytes: Option<u64>,
    pub process_peak_working_set_bytes: Option<u64>,
    pub process_read_operations: Option<u64>,
    pub process_read_bytes: Option<u64>,
    pub process_write_operations: Option<u64>,
    pub process_write_bytes: Option<u64>,
    pub system_cpu_basis_points: Option<u32>,
    pub system_available_memory_bytes: Option<u64>,
    pub system_committed_memory_bytes: Option<u64>,
    pub unavailable_counter_count: u32,
}

/// Run-scoped device information. Hardware serial numbers are intentionally absent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDescriptor {
    pub device_key: String,
    pub volume_key: String,
    pub filesystem: Option<String>,
    pub capacity_bytes: Option<u64>,
    pub free_bytes_at_start: Option<u64>,
    pub bus_type: Option<String>,
    pub media_type: Option<String>,
    pub model: Option<String>,
}

/// One bounded physical-device gauge sample. Scaled integers keep units stable across providers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSample {
    pub sequence: u64,
    pub device_key: String,
    pub read_bytes_per_second: Option<u64>,
    pub read_iops_millis: Option<u64>,
    pub average_read_latency_micros: Option<u64>,
    pub active_millis_per_second: Option<u32>,
    pub queue_depth_millis: Option<u64>,
    pub unavailable_counter_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusRunStart {
    pub operation_id: String,
    pub product_run_id: Option<i64>,
    pub engine_version: String,
    pub worker_version: Option<String>,
    pub app_version: Option<String>,
    pub product_schema_version: Option<i64>,
    pub input_signature: String,
    pub started_unix_millis: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusRunRecord {
    pub id: i64,
    pub operation_id: String,
    pub product_run_id: Option<i64>,
    pub metrics_contract_version: u32,
    pub engine_version: String,
    pub worker_version: Option<String>,
    pub app_version: Option<String>,
    pub product_schema_version: Option<i64>,
    pub input_signature: String,
    pub state: TelemetryRunState,
    pub started_unix_millis: Option<i64>,
    pub completed_unix_millis: Option<i64>,
    pub last_monotonic_nanos: u64,
    pub last_sequence: u64,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryFlush {
    pub sequence: u64,
    pub observed_unix_millis: i64,
    pub monotonic_nanos: u64,
    pub phase: TelemetryPhase,
    pub phase_state: TelemetryPhaseState,
    pub phase_started_monotonic_nanos: Option<u64>,
    pub phase_completed_monotonic_nanos: Option<u64>,
    pub phase_active_nanos: u64,
    pub counters: ScanCounters,
    pub host_sample: Option<HostSample>,
    pub devices: Vec<DeviceDescriptor>,
    pub device_samples: Vec<DeviceSample>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusRunTerminal {
    pub state: TelemetryRunState,
    pub completed_unix_millis: i64,
    pub monotonic_nanos: u64,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteDisposition {
    Applied,
    Replayed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusPhaseSummary {
    pub phase: TelemetryPhase,
    pub state: TelemetryPhaseState,
    pub started_monotonic_nanos: Option<u64>,
    pub completed_monotonic_nanos: Option<u64>,
    pub active_nanos: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusCounterSummary {
    pub metric: String,
    pub value: u64,
    pub updated_sequence: u64,
}

/// Fixed-size host gauges for one run. Raw samples remain worker-owned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostPerformanceSummary {
    pub latest: Option<HostSample>,
    pub peak_process_private_bytes: Option<u64>,
    pub peak_process_working_set_bytes: Option<u64>,
    pub peak_system_cpu_basis_points: Option<u32>,
    pub minimum_system_available_memory_bytes: Option<u64>,
}

/// Fixed-size current and peak gauges for one run-scoped device descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePerformanceSummary {
    pub descriptor: DeviceDescriptor,
    pub latest: Option<DeviceSample>,
    pub peak_read_bytes_per_second: Option<u64>,
    pub peak_read_iops_millis: Option<u64>,
    pub peak_average_read_latency_micros: Option<u64>,
    pub peak_active_millis_per_second: Option<u32>,
    pub peak_queue_depth_millis: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusRetentionPolicy {
    pub max_terminal_runs: u32,
    pub max_samples_per_run: u32,
}

impl Default for StatusRetentionPolicy {
    fn default() -> Self {
        Self {
            max_terminal_runs: 50,
            max_samples_per_run: 100_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusRetentionResult {
    pub terminal_runs_deleted: u64,
    pub host_samples_deleted: u64,
    pub device_samples_deleted: u64,
    pub replay_flushes_deleted: u64,
    pub wal_busy: bool,
    pub wal_frames: Option<u64>,
    pub wal_frames_checkpointed: Option<u64>,
}
