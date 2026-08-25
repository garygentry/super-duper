mod models;
mod sampler;
mod status_db;

pub use models::{
    CounterKind, DeviceDescriptor, DeviceSample, HostSample, MetricInvariantError, ScanCounters,
    StatusCounterSummary, StatusPhaseSummary, StatusRetentionPolicy, StatusRetentionResult,
    StatusRunRecord, StatusRunStart, StatusRunTerminal, TelemetryFlush, TelemetryPhase,
    TelemetryPhaseState, TelemetryRunState, WriteDisposition, METRICS_CONTRACT_VERSION,
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
