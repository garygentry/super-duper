use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const METRICS_CONTRACT_VERSION: u32 = 1;

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
    PartialHashesAttempted,
    PartialHashesSucceeded,
    PartialHashesFailed,
    PartialHashBytesRead,
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
    pub const ALL: [Self; 38] = [
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
        Self::PartialHashesAttempted,
        Self::PartialHashesSucceeded,
        Self::PartialHashesFailed,
        Self::PartialHashBytesRead,
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
            Self::PartialHashesAttempted => "partial_hashes_attempted",
            Self::PartialHashesSucceeded => "partial_hashes_succeeded",
            Self::PartialHashesFailed => "partial_hashes_failed",
            Self::PartialHashBytesRead => "partial_hash_bytes_read",
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

/// Monotonic cumulative scan counters for metrics contract v1.
///
/// `discovered_files` includes logical zero-byte files and hard-link aliases. Size-bucket and hash
/// counters describe first-physical, non-empty files only. Cache lookup outcomes are exclusive:
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
    pub partial_hashes_attempted: u64,
    pub partial_hashes_succeeded: u64,
    pub partial_hashes_failed: u64,
    pub partial_hash_bytes_read: u64,
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
                &[self.singleton_size_buckets, self.candidate_size_buckets],
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
                        &[self.singleton_size_files, self.candidate_files],
                        physical_files,
                    )
                }),
            "classified physical files cannot exceed discovered non-empty physical files",
        )?;
        invariant(
            sum_leq(
                &[self.singleton_size_bytes, self.candidate_bytes],
                self.discovered_bytes,
            ),
            "classified physical bytes cannot exceed discovered logical bytes",
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
            self.full_hash_cache_misses
                .checked_add(self.full_hash_cache_errors)
                .is_some_and(|content_reads| self.full_hash_content_reads_started <= content_reads),
            "content reads require a cache miss or cache error",
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
