mod models;
mod progress;
mod sampler;
mod status_db;

pub use models::{
    CounterKind, DeviceDescriptor, DeviceSample, HostSample, MetricInvariantError, ScanCounters,
    StatusCounterSummary, StatusPhaseSummary, StatusRetentionPolicy, StatusRetentionResult,
    StatusRunRecord, StatusRunStart, StatusRunTerminal, TelemetryFlush, TelemetryPhase,
    TelemetryPhaseState, TelemetryRunState, WriteDisposition, METRICS_CONTRACT_VERSION,
};
pub use progress::{
    ActiveDeviceProgress, ActiveDeviceUnavailableReason, CandidateFunnelProgress,
    EtaUnavailableReason, ProgressContractError, ProgressEta, ProgressLogicalCounters,
    ProgressObservation, ProgressQuantity, ProgressRate, ProgressRateUnavailableReason,
    ProgressRateValue, ProgressRates, ProgressReducer, RemainingKnownWork, RemainingWorkStage,
    ScanProgressSnapshot, ETA_MIN_INTERVAL_NANOS, ETA_MIN_OBSERVATION_SPAN_NANOS,
    ETA_RATE_STABILITY_MIN_BASIS_POINTS, MAX_ACTIVE_PROGRESS_DEVICES, MAX_PROGRESS_RATE_POINTS,
    PROGRESS_CONTRACT_VERSION, PROGRESS_RATE_POINT_MIN_INTERVAL_NANOS,
    RECENT_PROGRESS_RATE_WINDOW_NANOS,
};
#[cfg(target_os = "windows")]
pub use sampler::WindowsSamplerPlatform;
pub use sampler::{
    DeviceGaugeSnapshot, HostGaugeSnapshot, SamplerClock, SamplerPlatform, SystemSamplerClock,
    TelemetrySampleBatch, TelemetrySampler,
};
pub use status_db::{
    StatusDatabase, StatusStoreError, CURRENT_STATUS_SCHEMA_VERSION, MAX_STATUS_DEVICES_PER_RUN,
    MAX_STATUS_RUN_PAGE, MAX_STATUS_SAMPLE_PAGE,
};
