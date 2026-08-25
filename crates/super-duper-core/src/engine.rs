use crate::analysis::{dir_fingerprint, dir_similarity, exact_folders};
use crate::config::{self, AppConfig};
use crate::error::Error;
use crate::hasher;
use crate::platform;
use crate::progress::ProgressReporter;
use crate::scanner;
use crate::storage::models::{
    CloudPolicy, RunExclusionInsert, RunParameters, RunWarningAggregateInsert, ScannedFile,
};
use crate::storage::Database;
use crate::telemetry::{
    ActiveDeviceProgress, ActiveDeviceUnavailableReason, ProgressLogicalCounters,
    ProgressObservation, ProgressReducer, ScanCounters, StatusDatabase, StatusRunStart,
    StatusRunTerminal, TelemetryFlush, TelemetryPhase, TelemetryPhaseState, TelemetryRunState,
    METRICS_CONTRACT_VERSION, PROGRESS_CONTRACT_VERSION,
};
use chrono::Utc;
use dashmap::DashMap;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::Hasher;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, UNIX_EPOCH};
use tracing::{info, warn};
use twox_hash::XxHash64;

pub struct ScanEngine {
    config: AppConfig,
    db_path: String,
    status_db_path: Option<String>,
    status_worker_version: Option<String>,
    status_sample_interval: Duration,
    status_maximum_samples: u64,
    session_id: Option<i64>,
    cancel_token: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct ScanResult {
    pub session_id: i64,
    pub run_id: i64,
    pub scan_duration: Duration,
    pub hash_duration: Duration,
    pub db_write_duration: Duration,
    pub dir_analysis_duration: Duration,
    pub finalizing_duration: Duration,
    pub total_files_scanned: usize,
    pub total_bytes_discovered: u64,
    pub files_hashed: usize,
    pub duplicate_groups: usize,
    pub duplicate_files: usize,
    pub duplicate_folder_groups: usize,
    pub wasted_bytes: u64,
    pub warning_count: usize,
    pub dir_fingerprints: usize,
    pub dir_similarity_pairs: usize,
}

#[derive(Debug)]
pub struct ScanStats {
    pub distinct_sizes: u64,
    pub total_files: usize,
    pub total_size: u64,
    pub singleton_sizes: u64,
    pub singleton_files: usize,
    pub singleton_bytes: u64,
    pub duplicate_candidate_sizes: u64,
    pub duplicate_candidate_files: usize,
    pub duplicate_candidate_bytes: u64,
}

struct PersistedResults {
    groups: usize,
    duplicate_files: usize,
    duplicate_logical_bytes: u64,
    wasted_bytes: u64,
    warnings: usize,
    warning_examples: Vec<String>,
}

struct RunTelemetry {
    database: Option<StatusDatabase>,
    status_run_id: Option<i64>,
    started: Instant,
    sequence: u64,
    counters: ScanCounters,
    logical: ProgressLogicalCounters,
    candidate_totals_known: bool,
    final_results_complete: bool,
    progress_reducer: ProgressReducer,
    current_phase: Option<(TelemetryPhase, u64)>,
    heartbeat_interval: Option<Duration>,
    #[cfg(target_os = "windows")]
    sampler: Option<
        crate::telemetry::TelemetrySampler<
            crate::telemetry::WindowsSamplerPlatform,
            crate::telemetry::SystemSamplerClock,
        >,
    >,
}

impl RunTelemetry {
    fn begin(engine: &ScanEngine, product_run_id: i64, parameters: &RunParameters) -> Self {
        let started = Instant::now();
        let mut telemetry = Self {
            database: None,
            status_run_id: None,
            started,
            sequence: 0,
            counters: ScanCounters::default(),
            logical: ProgressLogicalCounters::default(),
            candidate_totals_known: false,
            final_results_complete: false,
            progress_reducer: ProgressReducer::new(),
            current_phase: None,
            heartbeat_interval: None,
            #[cfg(target_os = "windows")]
            sampler: None,
        };
        let Some(path) = engine.status_db_path.as_deref() else {
            return telemetry;
        };

        let mut signature_hasher = XxHash64::with_seed(0);
        match serde_json::to_vec(parameters) {
            Ok(payload) => signature_hasher.write(&payload),
            Err(error) => {
                warn!("Unable to serialize scan telemetry input signature: {error}");
                signature_hasher.write(parameters.roots_json().as_bytes());
            }
        }
        let start = StatusRunStart {
            operation_id: format!("scan-run-{product_run_id}"),
            product_run_id: Some(product_run_id),
            engine_version: crate::ENGINE_VERSION.to_owned(),
            worker_version: engine.status_worker_version.clone(),
            app_version: None,
            product_schema_version: Some(crate::storage::sqlite::CURRENT_SCHEMA_VERSION),
            input_signature: format!("xxh64:{:016x}", signature_hasher.finish()),
            started_unix_millis: Utc::now().timestamp_millis(),
        };
        let open_result = StatusDatabase::open(path).and_then(|mut database| {
            let (run, _) = database.begin_run(&start)?;
            Ok((database, run.id))
        });
        match open_result {
            Ok((database, run_id)) => {
                telemetry.database = Some(database);
                telemetry.status_run_id = Some(run_id);
                #[cfg(target_os = "windows")]
                {
                    match crate::telemetry::TelemetrySampler::new(
                        crate::telemetry::WindowsSamplerPlatform::default(),
                        crate::telemetry::SystemSamplerClock::default(),
                        &parameters.roots,
                        engine
                            .status_sample_interval
                            .as_nanos()
                            .min(u128::from(u64::MAX)) as u64,
                        engine.status_maximum_samples,
                    ) {
                        Ok(sampler) => {
                            telemetry.sampler = Some(sampler);
                            telemetry.heartbeat_interval = Some(engine.status_sample_interval);
                        }
                        Err(error) => {
                            telemetry.counters.unavailable_counters =
                                telemetry.counters.unavailable_counters.saturating_add(1);
                            warn!("Scan platform telemetry is unavailable: {error}");
                        }
                    }
                }
            }
            Err(error) => {
                warn!("Scan telemetry is unavailable for product run {product_run_id}: {error}");
            }
        }
        telemetry
    }

    fn elapsed_nanos(&self) -> u64 {
        self.started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }

    fn record_progress_observation(
        &mut self,
        phase: TelemetryPhase,
        phase_started_monotonic_nanos: u64,
    ) -> Option<ProgressObservation> {
        let observation = ProgressObservation {
            progress_contract_version: PROGRESS_CONTRACT_VERSION,
            metrics_contract_version: METRICS_CONTRACT_VERSION,
            monotonic_nanos: self.elapsed_nanos(),
            phase,
            phase_started_monotonic_nanos,
            candidate_totals_known: self.candidate_totals_known,
            final_results_complete: self.final_results_complete,
            counters: self.counters.clone(),
            logical: self.logical,
            active_devices: ActiveDeviceProgress::Unavailable {
                reason: ActiveDeviceUnavailableReason::MappingUnavailable,
            },
        };
        match self.progress_reducer.observe(observation.clone()) {
            Ok(_) => Some(observation),
            Err(error) => {
                warn!("Rejected internally produced scan progress observation: {error}");
                debug_assert!(false, "invalid internally produced scan progress: {error}");
                None
            }
        }
    }

    fn apply_hash_progress(&mut self, delta: &hasher::HashProgressDelta) -> io::Result<()> {
        macro_rules! checked_add {
            ($target:expr, $value:expr, $name:literal) => {
                $target = $target.checked_add($value).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        concat!("scan progress counter overflow: ", $name),
                    )
                })?;
            };
        }
        checked_add!(
            self.counters.partial_hashes_attempted,
            delta.partial_hashes_attempted,
            "partial_hashes_attempted"
        );
        checked_add!(
            self.counters.partial_hashes_succeeded,
            delta.partial_hashes_succeeded,
            "partial_hashes_succeeded"
        );
        checked_add!(
            self.counters.partial_hashes_failed,
            delta.partial_hashes_failed,
            "partial_hashes_failed"
        );
        checked_add!(
            self.counters.partial_hash_bytes_read,
            delta.partial_hash_bytes_read,
            "partial_hash_bytes_read"
        );
        checked_add!(
            self.counters.partial_collision_buckets,
            delta.partial_collision_buckets,
            "partial_collision_buckets"
        );
        checked_add!(
            self.counters.partial_collision_files,
            delta.partial_collision_files,
            "partial_collision_files"
        );
        checked_add!(
            self.counters.partial_collision_bytes,
            delta.partial_collision_bytes,
            "partial_collision_bytes"
        );
        checked_add!(
            self.counters.full_hash_requests,
            delta.full_hash_requests,
            "full_hash_requests"
        );
        checked_add!(
            self.counters.full_hash_cache_hits,
            delta.full_hash_cache_hits,
            "full_hash_cache_hits"
        );
        checked_add!(
            self.counters.full_hash_cache_misses,
            delta.full_hash_cache_misses,
            "full_hash_cache_misses"
        );
        checked_add!(
            self.counters.full_hash_cache_errors,
            delta.full_hash_cache_errors,
            "full_hash_cache_errors"
        );
        checked_add!(
            self.counters.full_hash_cache_stores,
            delta.full_hash_cache_stores,
            "full_hash_cache_stores"
        );
        checked_add!(
            self.counters.full_hash_content_reads_started,
            delta.full_hash_content_reads_started,
            "full_hash_content_reads_started"
        );
        checked_add!(
            self.counters.full_hash_content_reads_completed,
            delta.full_hash_content_reads_completed,
            "full_hash_content_reads_completed"
        );
        checked_add!(
            self.counters.full_hash_content_reads_failed,
            delta.full_hash_content_reads_failed,
            "full_hash_content_reads_failed"
        );
        checked_add!(
            self.counters.full_hash_bytes_read,
            delta.full_hash_bytes_read,
            "full_hash_bytes_read"
        );
        checked_add!(
            self.counters.unavailable_counters,
            delta.unavailable_counters,
            "unavailable_counters"
        );
        checked_add!(
            self.counters.cancel_checks,
            delta.cancel_checks,
            "cancel_checks"
        );
        checked_add!(
            self.counters.cancelled_work_items,
            delta.cancelled_work_items,
            "cancelled_work_items"
        );
        checked_add!(self.counters.warnings, delta.warning_count, "warnings");
        checked_add!(
            self.logical.partial_screened_files,
            delta.partial_screened_files,
            "partial_screened_files"
        );
        checked_add!(
            self.logical.partial_screened_bytes,
            delta.partial_screened_bytes,
            "partial_screened_bytes"
        );
        checked_add!(
            self.logical.full_hash_request_bytes,
            delta.full_hash_request_bytes,
            "full_hash_request_bytes"
        );
        checked_add!(
            self.logical.full_hash_satisfied_files,
            delta.full_hash_satisfied_files,
            "full_hash_satisfied_files"
        );
        checked_add!(
            self.logical.full_hash_satisfied_bytes,
            delta.full_hash_satisfied_bytes,
            "full_hash_satisfied_bytes"
        );
        checked_add!(
            self.logical.full_hash_failed_files,
            delta.full_hash_failures,
            "full_hash_failed_files"
        );
        checked_add!(
            self.logical.full_hash_failed_bytes,
            delta.full_hash_failed_bytes,
            "full_hash_failed_bytes"
        );
        checked_add!(
            self.logical.hash_pipeline_resolved_files,
            delta.hash_pipeline_resolved_files,
            "hash_pipeline_resolved_files"
        );
        checked_add!(
            self.logical.hash_pipeline_resolved_bytes,
            delta.hash_pipeline_resolved_bytes,
            "hash_pipeline_resolved_bytes"
        );
        Ok(())
    }

    fn flush_phase(
        &mut self,
        phase: TelemetryPhase,
        state: TelemetryPhaseState,
        started_nanos: Option<u64>,
        completed_nanos: Option<u64>,
    ) {
        if state == TelemetryPhaseState::Running {
            self.current_phase =
                Some((phase, started_nanos.unwrap_or_else(|| self.elapsed_nanos())));
        } else if self
            .current_phase
            .is_some_and(|(current_phase, _)| current_phase == phase)
        {
            self.current_phase = None;
        }
        self.sequence = self.sequence.saturating_add(1);
        let now = self.elapsed_nanos();
        let sample = self.take_sample(self.sequence, Some(phase));
        self.persist_flush(phase, state, started_nanos, completed_nanos, now, sample);
    }

    fn sample_current_phase(&mut self) {
        let Some((phase, phase_start)) = self.current_phase else {
            return;
        };
        let next_sequence = self.sequence.saturating_add(1);
        let Some(sample) = self.take_sample(next_sequence, Some(phase)) else {
            return;
        };
        self.sequence = next_sequence;
        let now = self.elapsed_nanos();
        self.persist_flush(
            phase,
            TelemetryPhaseState::Running,
            Some(phase_start),
            None,
            now,
            Some(sample),
        );
    }

    fn flush_running_progress_snapshot(&mut self) -> Option<ProgressObservation> {
        let (phase, phase_start) = self.current_phase?;
        self.sequence = self.sequence.saturating_add(1);
        let now = self.elapsed_nanos();
        self.persist_flush(
            phase,
            TelemetryPhaseState::Running,
            Some(phase_start),
            None,
            now,
            None,
        );
        self.record_progress_observation(phase, phase_start)
    }

    #[cfg(target_os = "windows")]
    fn take_sample(
        &mut self,
        sequence: u64,
        phase: Option<TelemetryPhase>,
    ) -> Option<crate::telemetry::TelemetrySampleBatch> {
        let mut sample = self.sampler.as_mut()?.try_sample(sequence, phase)?;
        // The cadence clock is independent, but persisted sample identity belongs to the run's
        // monotonic/status envelope.
        sample.host.monotonic_nanos = self.elapsed_nanos();
        sample.host.observed_unix_millis = Utc::now().timestamp_millis();
        self.counters.telemetry_samples_lost = self
            .counters
            .telemetry_samples_lost
            .saturating_add(sample.samples_lost_since_previous);
        let unavailable = u64::from(sample.host.unavailable_counter_count).saturating_add(
            sample
                .devices
                .iter()
                .map(|device| u64::from(device.unavailable_counter_count))
                .sum(),
        );
        self.counters.unavailable_counters = self
            .counters
            .unavailable_counters
            .saturating_add(unavailable);
        Some(sample)
    }

    #[cfg(not(target_os = "windows"))]
    fn take_sample(
        &mut self,
        _sequence: u64,
        _phase: Option<TelemetryPhase>,
    ) -> Option<crate::telemetry::TelemetrySampleBatch> {
        None
    }

    fn persist_flush(
        &mut self,
        phase: TelemetryPhase,
        state: TelemetryPhaseState,
        started_nanos: Option<u64>,
        completed_nanos: Option<u64>,
        now: u64,
        sample: Option<crate::telemetry::TelemetrySampleBatch>,
    ) {
        let (Some(database), Some(run_id)) = (&mut self.database, self.status_run_id) else {
            return;
        };
        let now = sample
            .as_ref()
            .map(|sample| sample.host.monotonic_nanos)
            .unwrap_or(now);
        let observed_unix_millis = sample
            .as_ref()
            .map(|sample| sample.host.observed_unix_millis)
            .unwrap_or_else(|| Utc::now().timestamp_millis());
        let phase_start = started_nanos.unwrap_or(now);
        let phase_end = completed_nanos.unwrap_or(now);
        #[cfg(target_os = "windows")]
        let devices = sample
            .as_ref()
            .and(self.sampler.as_ref())
            .map(|sampler| sampler.devices().to_vec())
            .unwrap_or_default();
        #[cfg(not(target_os = "windows"))]
        let devices = Vec::new();
        let (host_sample, device_samples) = sample
            .map(|sample| (Some(sample.host), sample.devices))
            .unwrap_or_default();
        let flush = TelemetryFlush {
            sequence: self.sequence,
            observed_unix_millis,
            monotonic_nanos: now,
            phase,
            phase_state: state,
            phase_started_monotonic_nanos: Some(phase_start),
            phase_completed_monotonic_nanos: completed_nanos,
            phase_active_nanos: phase_end.saturating_sub(phase_start),
            counters: self.counters.clone(),
            host_sample,
            devices,
            device_samples,
        };
        if let Err(error) = database.flush(run_id, &flush) {
            self.counters.telemetry_flush_errors =
                self.counters.telemetry_flush_errors.saturating_add(1);
            warn!("Scan telemetry flush {} failed: {error}", self.sequence);
        }
    }

    fn finish(
        &mut self,
        state: TelemetryRunState,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) {
        let now = self.elapsed_nanos();
        let (Some(database), Some(run_id)) = (&mut self.database, self.status_run_id) else {
            return;
        };
        let terminal = StatusRunTerminal {
            state,
            completed_unix_millis: Utc::now().timestamp_millis(),
            monotonic_nanos: now,
            error_code: error_code.map(str::to_owned),
            error_message: error_message.map(str::to_owned),
        };
        match database.finish_run(run_id, &terminal) {
            Ok(_) => {
                if let Err(error) =
                    database.apply_retention(crate::telemetry::StatusRetentionPolicy::default())
                {
                    warn!("Scan telemetry retention failed after terminal write: {error}");
                }
            }
            Err(error) => {
                warn!("Scan telemetry terminal write failed for product run: {error}");
            }
        }
    }
}

struct TelemetryHeartbeat {
    stop: mpsc::Sender<()>,
    handle: Option<JoinHandle<()>>,
}

impl TelemetryHeartbeat {
    fn start(telemetry: Arc<Mutex<RunTelemetry>>) -> Option<Self> {
        #[cfg(target_os = "windows")]
        let interval = telemetry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .heartbeat_interval?;
        #[cfg(not(target_os = "windows"))]
        return None;

        #[cfg(target_os = "windows")]
        {
            let (stop, receiver) = mpsc::channel();
            let handle = std::thread::spawn(move || loop {
                match receiver.recv_timeout(interval) {
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => telemetry
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .sample_current_phase(),
                }
            });
            Some(Self {
                stop,
                handle: Some(handle),
            })
        }
    }

    fn stop(mut self) {
        let _ = self.stop.send(());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn publish_progress_observation(
    telemetry: &Arc<Mutex<RunTelemetry>>,
    progress: &dyn ProgressReporter,
    phase: TelemetryPhase,
    phase_started_monotonic_nanos: u64,
) {
    let observation = telemetry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .record_progress_observation(phase, phase_started_monotonic_nanos);
    if let Some(observation) = observation {
        progress.on_progress_observation(&observation);
    }
}

fn publish_terminal_progress_snapshot(
    telemetry: &Arc<Mutex<RunTelemetry>>,
    progress: &dyn ProgressReporter,
) {
    let observation = telemetry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .flush_running_progress_snapshot();
    if let Some(observation) = observation {
        progress.on_progress_observation(&observation);
    }
}

struct EngineHashProgressSink<'a> {
    telemetry: Arc<Mutex<RunTelemetry>>,
    progress: &'a dyn ProgressReporter,
    phase_started_monotonic_nanos: u64,
    totals: Mutex<hasher::HashProgressDelta>,
    publication: Mutex<()>,
}

impl<'a> EngineHashProgressSink<'a> {
    fn new(
        telemetry: Arc<Mutex<RunTelemetry>>,
        progress: &'a dyn ProgressReporter,
        phase_started_monotonic_nanos: u64,
    ) -> Self {
        Self {
            telemetry,
            progress,
            phase_started_monotonic_nanos,
            totals: Mutex::new(hasher::HashProgressDelta::default()),
            publication: Mutex::new(()),
        }
    }
}

impl hasher::HashProgressSink for EngineHashProgressSink<'_> {
    fn publish(&self, delta: hasher::HashProgressDelta) -> io::Result<()> {
        let _publication = self
            .publication
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.totals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .checked_add_assign(&delta)?;
        let observation = {
            let mut telemetry = self
                .telemetry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            telemetry.apply_hash_progress(&delta)?;
            telemetry.record_progress_observation(
                TelemetryPhase::CandidateScreening,
                self.phase_started_monotonic_nanos,
            )
        }
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "scan engine rejected a hash progress observation",
            )
        })?;
        self.progress.on_progress_observation(&observation);
        Ok(())
    }

    fn snapshot(&self) -> hasher::HashProgressDelta {
        self.totals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

struct EngineDiscoveryProgressReporter<'a> {
    telemetry: Arc<Mutex<RunTelemetry>>,
    progress: &'a dyn ProgressReporter,
    publication: Mutex<()>,
}

impl<'a> EngineDiscoveryProgressReporter<'a> {
    fn new(telemetry: Arc<Mutex<RunTelemetry>>, progress: &'a dyn ProgressReporter) -> Self {
        Self {
            telemetry,
            progress,
            publication: Mutex::new(()),
        }
    }
}

impl ProgressReporter for EngineDiscoveryProgressReporter<'_> {
    fn on_discovery_progress(
        &self,
        files_found: usize,
        bytes_found: u64,
        warning_count: usize,
        current_path: &str,
    ) {
        let _publication = self
            .publication
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let observation = {
            let mut telemetry = self
                .telemetry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let discovered_files = files_found as u64;
            let warnings = warning_count as u64;
            let changed = discovered_files > telemetry.counters.discovered_files
                || bytes_found > telemetry.counters.discovered_bytes
                || warnings > telemetry.counters.warnings;
            telemetry.counters.discovered_files =
                telemetry.counters.discovered_files.max(discovered_files);
            telemetry.counters.discovered_bytes =
                telemetry.counters.discovered_bytes.max(bytes_found);
            telemetry.counters.warnings = telemetry.counters.warnings.max(warnings);
            changed
                .then(|| telemetry.record_progress_observation(TelemetryPhase::Discovering, 0))
                .flatten()
        };
        if let Some(observation) = observation {
            self.progress.on_progress_observation(&observation);
        }
        self.progress
            .on_discovery_progress(files_found, bytes_found, warning_count, current_path);
    }
}

impl ScanEngine {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            db_path: "super_duper.db".to_string(),
            status_db_path: None,
            status_worker_version: None,
            status_sample_interval: Duration::from_secs(5),
            status_maximum_samples: 100_000,
            session_id: None,
            cancel_token: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_db_path(mut self, path: &str) -> Self {
        self.db_path = path.to_string();
        self
    }

    /// Enable best-effort scan telemetry in a database separate from product state.
    pub fn with_status_db_path(mut self, path: &str) -> Self {
        self.status_db_path = Some(path.to_string());
        self
    }

    pub fn with_status_worker_version(mut self, version: &str) -> Self {
        self.status_worker_version = Some(version.to_string());
        self
    }

    /// Override the default five-second sampler cadence. Zero values leave the bounded defaults.
    pub fn with_status_sampling(mut self, interval: Duration, maximum_samples: u64) -> Self {
        if !interval.is_zero() && maximum_samples > 0 {
            self.status_sample_interval = interval;
            self.status_maximum_samples = maximum_samples;
        }
        self
    }

    /// Bind execution to an existing reusable session definition.
    pub fn with_session_id(mut self, session_id: i64) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn cancel(&self) {
        self.cancel_token.store(true, Ordering::Relaxed);
    }

    pub fn cancel_token(&self) -> Arc<AtomicBool> {
        self.cancel_token.clone()
    }

    /// Creates a durable run before traversal, then persists exactly one terminal outcome.
    pub fn scan(&self, progress: &dyn ProgressReporter) -> Result<ScanResult, Error> {
        self.cancel_token.store(false, Ordering::Relaxed);

        let roots = config::non_overlapping_directories(self.config.root_paths.clone());
        let db = Database::open(&self.db_path)?;
        let session_id = match self.session_id {
            Some(id) => {
                db.get_session(id)?;
                id
            }
            None => db.ensure_default_session(&roots, &self.config.ignore_patterns)?,
        };
        let parameters = RunParameters {
            roots: roots.clone(),
            ignore_patterns: self.config.ignore_patterns.clone(),
            directory_similarity_threshold_millis: 500,
            cloud_policy: CloudPolicy::default(),
            manual_location_exclusions: Vec::new(),
            registered_cloud_locations: Vec::new(),
            cloud_detection_status: Default::default(),
        };
        let run_id = db.create_scan_run(session_id, &parameters, crate::ENGINE_VERSION)?;
        if let Err(error) = db.start_scan_run(run_id) {
            let _ = db.fail_scan_run(run_id, &error.to_string());
            return Err(error.into());
        }

        self.finish_started_run(&db, session_id, run_id, &parameters, progress)
    }

    /// Executes a run that a coordinator has already transitioned to `running`.
    ///
    /// Unlike [`scan`](Self::scan), this does not reset the cancellation token. That lets a
    /// process coordinator publish the run ID and accept cancellation before the scan thread has
    /// entered traversal.
    pub fn execute_started_run(
        &self,
        run_id: i64,
        progress: &dyn ProgressReporter,
    ) -> Result<ScanResult, Error> {
        let db = Database::open_connection(&self.db_path)?;
        let run = db.get_scan_run(run_id)?;
        let parameters = RunParameters::from_json(&run.parameters_json)
            .ok_or_else(|| Error::Other(format!("run {run_id} has invalid parameters")))?;
        if run.status == "cancelling" && self.cancel_token.load(Ordering::Acquire) {
            let terminal_result = db.cancel_scan_run(run_id);
            let mut telemetry = RunTelemetry::begin(self, run_id, &parameters);
            telemetry.finish(
                TelemetryRunState::Cancelled,
                Some("cancelled"),
                Some("The scan was cancelled before traversal started."),
            );
            terminal_result?;
            return Err(Error::Cancelled);
        }
        if run.status != "running" {
            return Err(Error::Other(format!(
                "run {run_id} is not in the running state"
            )));
        }
        self.finish_started_run(&db, run.session_id, run_id, &parameters, progress)
    }

    fn finish_started_run(
        &self,
        db: &Database,
        session_id: i64,
        run_id: i64,
        parameters: &RunParameters,
        progress: &dyn ProgressReporter,
    ) -> Result<ScanResult, Error> {
        let roots = &parameters.roots;

        let telemetry = Arc::new(Mutex::new(RunTelemetry::begin(self, run_id, parameters)));
        telemetry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .flush_phase(
                TelemetryPhase::Discovering,
                TelemetryPhaseState::Running,
                Some(0),
                None,
            );
        publish_progress_observation(&telemetry, progress, TelemetryPhase::Discovering, 0);
        let heartbeat = TelemetryHeartbeat::start(telemetry.clone());

        info!(
            "Processing run {} for session {}: {:?}",
            run_id, session_id, roots
        );
        let result = self.execute_run(db, session_id, run_id, parameters, progress, &telemetry);
        if let Some(heartbeat) = heartbeat {
            heartbeat.stop();
        }
        publish_terminal_progress_snapshot(&telemetry, progress);
        let mut telemetry = telemetry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match result {
            Ok(result) => {
                telemetry.finish(TelemetryRunState::Completed, None, None);
                Ok(result)
            }
            Err(Error::Cancelled) => {
                let _ = db.mark_run_cancelling(run_id);
                let terminal_result = db.cancel_scan_run(run_id);
                telemetry.finish(
                    TelemetryRunState::Cancelled,
                    Some("cancelled"),
                    Some("The scan was cancelled."),
                );
                terminal_result?;
                Err(Error::Cancelled)
            }
            Err(_) if self.cancel_token.load(Ordering::Acquire) => {
                let _ = db.mark_run_cancelling(run_id);
                let terminal_result = db.cancel_scan_run(run_id);
                telemetry.finish(
                    TelemetryRunState::Cancelled,
                    Some("cancelled"),
                    Some("The scan was cancelled."),
                );
                terminal_result?;
                Err(Error::Cancelled)
            }
            Err(error) => {
                let terminal_result = db.fail_scan_run(run_id, &error.to_string());
                telemetry.finish(
                    TelemetryRunState::Failed,
                    Some("scan_failed"),
                    Some(&error.to_string()),
                );
                terminal_result?;
                Err(error)
            }
        }
    }

    fn execute_run(
        &self,
        db: &Database,
        session_id: i64,
        run_id: i64,
        parameters: &RunParameters,
        progress: &dyn ProgressReporter,
        telemetry: &Arc<Mutex<RunTelemetry>>,
    ) -> Result<ScanResult, Error> {
        let roots = &parameters.roots;
        let ignore_patterns = &parameters.ignore_patterns;
        let root_slices: Vec<&str> = roots.iter().map(String::as_str).collect();
        let ignore_slices: Vec<&str> = ignore_patterns.iter().map(String::as_str).collect();
        let mut location_exclusions = parameters
            .manual_location_exclusions
            .iter()
            .map(|path| scanner::LocationExclusion {
                path: PathBuf::from(path),
                reason_code: "manual_location_exclusion".to_owned(),
                provider_id: None,
                provider_name: None,
            })
            .collect::<Vec<_>>();
        if parameters.cloud_policy == CloudPolicy::ExcludeRegisteredRoots {
            location_exclusions.extend(parameters.registered_cloud_locations.iter().map(
                |location| scanner::LocationExclusion {
                    path: PathBuf::from(&location.path),
                    reason_code: "registered_cloud_root_excluded".to_owned(),
                    provider_id: Some(location.provider_id.clone()),
                    provider_name: Some(location.display_name.clone()),
                },
            ));
        }

        progress.on_scan_start();
        let scan_start = Instant::now();
        let discovery_progress = EngineDiscoveryProgressReporter::new(telemetry.clone(), progress);
        let traversal = scanner::discover_files_with_exclusions(
            &root_slices,
            &ignore_slices,
            &location_exclusions,
            &self.cancel_token,
            &discovery_progress,
        )?;
        db.replace_run_exclusions(
            run_id,
            &traversal
                .excluded_subtrees
                .iter()
                .map(|exclusion| RunExclusionInsert {
                    path: exclusion.path.clone(),
                    reason_code: exclusion.reason_code.clone(),
                    provider_id: exclusion.provider_id.clone(),
                    provider_name: exclusion.provider_name.clone(),
                })
                .collect::<Vec<_>>(),
        )?;
        let mut warning_aggregates = Vec::new();
        if let Some(warning) = warning_aggregate(
            "discovering",
            "discovery_recoverable_warning",
            "Some roots, directories, entries, or file metadata could not be inspected safely.",
            traversal.warning_count,
            traversal
                .files
                .iter()
                .filter_map(|file| {
                    file.warning_message
                        .as_ref()
                        .map(|message| format!("{}: {message}", file.canonical_path))
                })
                .collect(),
            "A discovery item could not be inspected; original details are in local diagnostics.",
        ) {
            warning_aggregates.push(warning);
        }
        db.replace_run_warning_aggregates(run_id, &warning_aggregates)?;
        let scan_duration = scan_start.elapsed();
        if self.cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let stats = compute_scan_stats(&traversal.size_to_files);
        debug_assert!(stats.total_files <= traversal.files_discovered);
        debug_assert!(stats.total_size <= traversal.bytes_discovered);
        {
            let mut telemetry = telemetry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            telemetry.counters.discovered_files = traversal
                .files_discovered
                .saturating_add(traversal.zero_byte_files)
                as u64;
            telemetry.counters.discovered_bytes = traversal.bytes_discovered;
            telemetry.counters.zero_byte_files = traversal.zero_byte_files as u64;
            telemetry.counters.hard_link_alias_files =
                traversal.files_discovered.saturating_sub(stats.total_files) as u64;
            telemetry.counters.hard_link_alias_bytes =
                traversal.bytes_discovered.saturating_sub(stats.total_size);
            telemetry.counters.size_buckets = stats.distinct_sizes;
            telemetry.counters.singleton_size_buckets = stats.singleton_sizes;
            telemetry.counters.singleton_size_files = stats.singleton_files as u64;
            telemetry.counters.singleton_size_bytes = stats.singleton_bytes;
            // Baseline behavior admits every non-empty first-physical file to partial hashing,
            // including singleton buckets. SOP5 will make metadata_resolved_* non-zero.
            telemetry.counters.candidate_size_buckets = stats.distinct_sizes;
            telemetry.counters.candidate_files = stats.total_files as u64;
            telemetry.counters.candidate_bytes = stats.total_size;
            telemetry.counters.duplicate_candidate_size_buckets = stats.duplicate_candidate_sizes;
            telemetry.counters.duplicate_candidate_files = stats.duplicate_candidate_files as u64;
            telemetry.counters.duplicate_candidate_bytes = stats.duplicate_candidate_bytes;
            telemetry.counters.warnings = traversal.warning_count as u64;
            telemetry.candidate_totals_known = true;
            let discovery_done = telemetry.elapsed_nanos();
            telemetry.flush_phase(
                TelemetryPhase::Discovering,
                TelemetryPhaseState::Completed,
                Some(0),
                Some(discovery_done),
            );
        }
        publish_progress_observation(telemetry, progress, TelemetryPhase::Discovering, 0);
        progress.on_scan_complete(traversal.files_discovered, scan_duration.as_secs_f64());
        db.update_run_progress(
            run_id,
            "hashing",
            traversal.files_discovered as i64,
            traversal.bytes_discovered as i64,
            0,
            traversal.warning_count as i64,
        )?;
        if self.cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }

        progress.on_hash_start();
        let hashing_telemetry_start = {
            let mut telemetry = telemetry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let started = telemetry.elapsed_nanos();
            telemetry.flush_phase(
                TelemetryPhase::CandidateScreening,
                TelemetryPhaseState::Running,
                Some(started),
                None,
            );
            started
        };
        publish_progress_observation(
            telemetry,
            progress,
            TelemetryPhase::CandidateScreening,
            hashing_telemetry_start,
        );
        let hash_start = Instant::now();
        let hash_progress_sink =
            EngineHashProgressSink::new(telemetry.clone(), progress, hashing_telemetry_start);
        let hash_outcome = match hasher::build_content_hash_map_with_progress(
            traversal.size_to_files,
            &self.cancel_token,
            progress,
            &hash_progress_sink,
            &hasher::SystemHashPipelineIo,
        ) {
            Ok(outcome) => outcome,
            Err(_) if self.cancel_token.load(Ordering::Relaxed) => return Err(Error::Cancelled),
            Err(error) => return Err(error.into()),
        };
        let hash_duration = hash_start.elapsed();
        let warning_count = traversal.warning_count + hash_outcome.warning_count;
        {
            let mut telemetry = telemetry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            telemetry.counters.partial_hashes_attempted = hash_outcome.partial_hashes_attempted;
            telemetry.counters.partial_hashes_succeeded = hash_outcome.partial_hashes_succeeded;
            telemetry.counters.partial_hashes_failed = hash_outcome.partial_hashes_failed;
            telemetry.counters.partial_hash_bytes_read = hash_outcome.partial_hash_bytes_read;
            telemetry.counters.partial_collision_buckets = hash_outcome.partial_collision_buckets;
            telemetry.counters.partial_collision_files = hash_outcome.partial_collision_files;
            telemetry.counters.partial_collision_bytes = hash_outcome.partial_collision_bytes;
            telemetry.counters.full_hash_requests = hash_outcome.full_hash_requests;
            telemetry.counters.full_hash_cache_hits = hash_outcome.full_hash_cache_hits;
            telemetry.counters.full_hash_cache_misses = hash_outcome.full_hash_cache_misses;
            telemetry.counters.full_hash_cache_errors = hash_outcome.full_hash_cache_errors;
            telemetry.counters.full_hash_cache_stores = hash_outcome.full_hash_cache_stores;
            telemetry.counters.full_hash_content_reads_started =
                hash_outcome.full_hash_content_reads_started;
            telemetry.counters.full_hash_content_reads_completed =
                hash_outcome.full_hash_content_reads_completed;
            telemetry.counters.full_hash_content_reads_failed =
                hash_outcome.full_hash_content_reads_failed;
            telemetry.counters.full_hash_bytes_read = hash_outcome.full_hash_bytes_read;
            telemetry.counters.cancel_checks = hash_outcome.cancel_checks;
            telemetry.counters.cancelled_work_items = hash_outcome.cancelled_work_items;
            telemetry.counters.warnings = warning_count as u64;
            telemetry.logical.partial_screened_files = hash_outcome.partial_screened_files;
            telemetry.logical.partial_screened_bytes = hash_outcome.partial_screened_bytes;
            telemetry.logical.full_hash_request_bytes = hash_outcome.full_hash_request_bytes;
            telemetry.logical.full_hash_satisfied_files = hash_outcome.full_hash_satisfied_files;
            telemetry.logical.full_hash_satisfied_bytes = hash_outcome.full_hash_satisfied_bytes;
            telemetry.logical.full_hash_failed_files = hash_outcome.full_hash_failures;
            telemetry.logical.full_hash_failed_bytes = hash_outcome.full_hash_failed_bytes;
            telemetry.logical.hash_pipeline_resolved_files =
                hash_outcome.hash_pipeline_resolved_files;
            telemetry.logical.hash_pipeline_resolved_bytes =
                hash_outcome.hash_pipeline_resolved_bytes;
            let hashing_telemetry_done = telemetry.elapsed_nanos();
            telemetry.flush_phase(
                TelemetryPhase::CandidateScreening,
                TelemetryPhaseState::Completed,
                Some(hashing_telemetry_start),
                Some(hashing_telemetry_done),
            );
        }
        publish_progress_observation(
            telemetry,
            progress,
            TelemetryPhase::CandidateScreening,
            hashing_telemetry_start,
        );
        if let Some(warning) = warning_aggregate(
            "hashing",
            "hash_recoverable_warning",
            "Some candidate files could not be read or their hash cache operation degraded safely.",
            hash_outcome.warning_count,
            Vec::new(),
            "A candidate file read or cache operation failed; original details are in local diagnostics.",
        ) {
            warning_aggregates.push(warning);
        }
        db.replace_run_warning_aggregates(run_id, &warning_aggregates)?;
        progress.on_hash_complete(
            hash_outcome.confirmed_duplicates.len(),
            hash_duration.as_secs_f64(),
        );
        db.update_run_progress(
            run_id,
            "persisting",
            traversal.files_discovered as i64,
            traversal.bytes_discovered as i64,
            hash_outcome.files_hashed as i64,
            warning_count as i64,
        )?;
        if self.cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }

        progress.on_db_write_start();
        let persistence_telemetry_start = {
            let mut telemetry = telemetry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let started = telemetry.elapsed_nanos();
            telemetry.flush_phase(
                TelemetryPhase::Persisting,
                TelemetryPhaseState::Running,
                Some(started),
                None,
            );
            started
        };
        publish_progress_observation(
            telemetry,
            progress,
            TelemetryPhase::Persisting,
            persistence_telemetry_start,
        );
        let db_start = Instant::now();
        let persisted = persist_run_results(
            db,
            run_id,
            traversal.files,
            &hash_outcome.confirmed_duplicates,
            &self.cancel_token,
            progress,
        )?;
        let db_duration = db_start.elapsed();
        let mut warning_count = warning_count + persisted.warnings;
        {
            let mut telemetry = telemetry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            telemetry.counters.confirmed_duplicate_groups = persisted.groups as u64;
            telemetry.counters.confirmed_logical_copies = persisted.duplicate_files as u64;
            telemetry.counters.confirmed_physical_items = persisted.duplicate_files as u64;
            telemetry.counters.recoverable_bytes = persisted.wasted_bytes;
            telemetry.logical.confirmed_logical_bytes = persisted.duplicate_logical_bytes;
            telemetry.counters.warnings = warning_count as u64;
            let persistence_telemetry_done = telemetry.elapsed_nanos();
            telemetry.flush_phase(
                TelemetryPhase::Persisting,
                TelemetryPhaseState::Completed,
                Some(persistence_telemetry_start),
                Some(persistence_telemetry_done),
            );
        }
        publish_progress_observation(
            telemetry,
            progress,
            TelemetryPhase::Persisting,
            persistence_telemetry_start,
        );
        if let Some(warning) = warning_aggregate(
            "persisting",
            "snapshot_changed_after_discovery",
            "Some files changed or vanished after discovery and were excluded from duplicate results.",
            persisted.warnings,
            persisted.warning_examples,
            "A discovered file changed or vanished before its immutable result was committed.",
        ) {
            warning_aggregates.push(warning);
        }
        db.replace_run_warning_aggregates(run_id, &warning_aggregates)?;
        progress.on_db_write_complete(traversal.files_discovered, db_duration.as_secs_f64());
        db.update_run_progress(
            run_id,
            "analyzing_folders",
            traversal.files_discovered as i64,
            traversal.bytes_discovered as i64,
            hash_outcome.files_hashed as i64,
            warning_count as i64,
        )?;
        if self.cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }

        progress.on_dir_analysis_start();
        let analysis_telemetry_start = {
            let mut telemetry = telemetry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let started = telemetry.elapsed_nanos();
            telemetry.flush_phase(
                TelemetryPhase::AnalyzingFolders,
                TelemetryPhaseState::Running,
                Some(started),
                None,
            );
            started
        };
        publish_progress_observation(
            telemetry,
            progress,
            TelemetryPhase::AnalyzingFolders,
            analysis_telemetry_start,
        );
        let dir_start = Instant::now();
        let dir_fingerprints = dir_fingerprint::build_directory_fingerprints_cancellable(
            db,
            run_id,
            &self.cancel_token,
            progress,
        )?;
        let exact_folder_analysis = exact_folders::analyze_exact_folders_cancellable(
            db,
            run_id,
            &self.cancel_token,
            progress,
        )?;
        warning_count += exact_folder_analysis.warning_count;
        telemetry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .counters
            .warnings = warning_count as u64;
        if let Some(warning) = warning_aggregate(
            "analyzing_folders",
            "exact_folder_verification_warning",
            "Some exact-folder candidates could not be verified and were omitted.",
            exact_folder_analysis.warning_count,
            Vec::new(),
            "An exact-folder candidate changed, became unavailable, or could not be hashed safely.",
        ) {
            warning_aggregates.push(warning);
        }
        db.replace_run_warning_aggregates(run_id, &warning_aggregates)?;
        let dir_similarity_pairs = dir_similarity::compute_directory_similarity_cancellable(
            db,
            run_id,
            0.5,
            &self.cancel_token,
            progress,
        )?;
        let dir_duration = dir_start.elapsed();
        {
            let mut telemetry = telemetry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let done = telemetry.elapsed_nanos();
            telemetry.flush_phase(
                TelemetryPhase::AnalyzingFolders,
                TelemetryPhaseState::Completed,
                Some(analysis_telemetry_start),
                Some(done),
            );
        }
        publish_progress_observation(
            telemetry,
            progress,
            TelemetryPhase::AnalyzingFolders,
            analysis_telemetry_start,
        );
        progress.on_dir_analysis_complete(
            dir_fingerprints,
            dir_similarity_pairs,
            dir_duration.as_secs_f64(),
        );
        db.update_run_progress(
            run_id,
            "finalizing",
            traversal.files_discovered as i64,
            traversal.bytes_discovered as i64,
            hash_outcome.files_hashed as i64,
            warning_count as i64,
        )?;
        progress.on_finalizing();
        let finalizing_telemetry_start = {
            let mut telemetry = telemetry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let started = telemetry.elapsed_nanos();
            telemetry.flush_phase(
                TelemetryPhase::Finalizing,
                TelemetryPhaseState::Running,
                Some(started),
                None,
            );
            started
        };
        publish_progress_observation(
            telemetry,
            progress,
            TelemetryPhase::Finalizing,
            finalizing_telemetry_start,
        );
        let finalizing_start = Instant::now();
        if self.cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }

        db.complete_scan_run(
            run_id,
            traversal.files_discovered as i64,
            traversal.bytes_discovered as i64,
            hash_outcome.files_hashed as i64,
            persisted.groups as i64,
            exact_folder_analysis.visible_groups as i64,
            persisted.wasted_bytes as i64,
            warning_count as i64,
        )?;
        let finalizing_duration = finalizing_start.elapsed();
        {
            let mut telemetry = telemetry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            telemetry.final_results_complete = true;
            let done = telemetry.elapsed_nanos();
            telemetry.flush_phase(
                TelemetryPhase::Finalizing,
                TelemetryPhaseState::Completed,
                Some(finalizing_telemetry_start),
                Some(done),
            );
        }
        publish_progress_observation(
            telemetry,
            progress,
            TelemetryPhase::Finalizing,
            finalizing_telemetry_start,
        );
        progress.on_finalizing_complete(finalizing_duration.as_secs_f64());

        Ok(ScanResult {
            session_id,
            run_id,
            scan_duration,
            hash_duration,
            db_write_duration: db_duration,
            dir_analysis_duration: dir_duration,
            finalizing_duration,
            total_files_scanned: traversal.files_discovered,
            total_bytes_discovered: traversal.bytes_discovered,
            files_hashed: hash_outcome.files_hashed,
            duplicate_groups: persisted.groups,
            duplicate_files: persisted.duplicate_files,
            duplicate_folder_groups: exact_folder_analysis.visible_groups,
            wasted_bytes: persisted.wasted_bytes,
            warning_count,
            dir_fingerprints,
            dir_similarity_pairs,
        })
    }
}

fn compute_scan_stats(map: &DashMap<u64, Vec<PathBuf>>) -> ScanStats {
    let mut stats = ScanStats {
        distinct_sizes: 0,
        total_files: 0,
        total_size: 0,
        singleton_sizes: 0,
        singleton_files: 0,
        singleton_bytes: 0,
        duplicate_candidate_sizes: 0,
        duplicate_candidate_files: 0,
        duplicate_candidate_bytes: 0,
    };
    for entry in map {
        let file_count = entry.value().len();
        let bytes = entry.key().saturating_mul(file_count as u64);
        stats.distinct_sizes += 1;
        stats.total_files += file_count;
        stats.total_size = stats.total_size.saturating_add(bytes);
        if file_count == 1 {
            stats.singleton_sizes += 1;
            stats.singleton_files += 1;
            stats.singleton_bytes = stats.singleton_bytes.saturating_add(bytes);
        } else {
            stats.duplicate_candidate_sizes += 1;
            stats.duplicate_candidate_files += file_count;
            stats.duplicate_candidate_bytes = stats.duplicate_candidate_bytes.saturating_add(bytes);
        }
    }
    stats
}

fn persist_run_results(
    db: &Database,
    run_id: i64,
    mut discovered: Vec<scanner::DiscoveredFile>,
    content_hash_map: &DashMap<u64, Vec<PathBuf>>,
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> Result<PersistedResults, Error> {
    let mut hashes_by_path = HashMap::new();
    let mut groups = Vec::new();
    let mut warnings = 0;
    let mut warning_examples = Vec::new();
    let mut wasted_bytes = 0u64;
    let mut duplicate_logical_bytes = 0u64;
    let mut duplicate_files = 0usize;
    let mut invalid_snapshots = HashSet::new();
    let mut discovered_by_path = HashMap::new();
    for (index, file) in discovered.iter().enumerate() {
        discovered_by_path.insert(
            path_key(PathBuf::from(&file.canonical_path).as_path()),
            index,
        );
        discovered_by_path.insert(
            path_key(
                PathBuf::from(&file.root_path)
                    .join(&file.relative_path)
                    .as_path(),
            ),
            index,
        );
    }

    // Reconcile the discovery snapshot before constructing any duplicate groups. A warning must
    // never coexist with a group that was built from metadata we already know is stale.
    for (index, file) in discovered.iter_mut().enumerate() {
        if let Err(error) = validate_discovered_snapshot(file) {
            warnings += 1;
            push_warning_example(
                &mut warning_examples,
                format!("{}: {error}", file.canonical_path),
            );
            invalid_snapshots.insert(index);
            append_warning(
                &mut file.warning_message,
                format!("File changed or disappeared after discovery: {error}"),
            );
            tracing::warn!(
                "Discovered file {} changed or disappeared before persistence: {}",
                file.canonical_path,
                error
            );
        }
    }

    for entry in content_hash_map {
        if cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let hash = *entry.key() as i64;
        let mut paths = Vec::new();
        let mut file_size = 0i64;
        for path in entry.value() {
            if cancel_token.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let discovered_index = discovered_by_path.get(&path_key(path)).copied();
            let validated = fs::canonicalize(path).and_then(|canonical| {
                let metadata = fs::metadata(&canonical)?;
                let index = discovered_index
                    .or_else(|| discovered_by_path.get(&path_key(&canonical)).copied())
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "candidate was not present in the discovery snapshot",
                        )
                    })?;
                if invalid_snapshots.contains(&index) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "candidate no longer matches the discovery snapshot",
                    ));
                }
                let snapshot = &discovered[index];
                let modified = metadata_modified_nanos(&metadata)?;
                if metadata.len() != snapshot.file_size
                    || (snapshot.last_modified != 0 && modified != snapshot.last_modified)
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "file changed after discovery",
                    ));
                }
                Ok((index, canonical.to_string_lossy().into_owned()))
            });
            match validated {
                Ok((index, path)) => {
                    file_size = discovered[index].file_size as i64;
                    hashes_by_path.insert(path.clone(), hash);
                    paths.push(path);
                }
                Err(error) => {
                    let already_invalid =
                        discovered_index.is_some_and(|index| invalid_snapshots.contains(&index));
                    if !already_invalid {
                        warnings += 1;
                        push_warning_example(
                            &mut warning_examples,
                            format!("{}: {error}", path.display()),
                        );
                    }
                    if let Some(index) = discovered_index.filter(|_| !already_invalid) {
                        invalid_snapshots.insert(index);
                        append_warning(
                            &mut discovered[index].warning_message,
                            format!("Excluded from duplicate results: {error}"),
                        );
                    }
                    tracing::warn!(
                        "Duplicate candidate {} changed or disappeared before persistence: {}",
                        path.display(),
                        error
                    );
                }
            }
        }
        if paths.len() > 1 {
            duplicate_files += paths.len();
            duplicate_logical_bytes = duplicate_logical_bytes
                .saturating_add((file_size as u64).saturating_mul(paths.len() as u64));
            wasted_bytes += file_size as u64 * (paths.len() as u64 - 1);
            groups.push((hash, file_size, paths));
        }
    }

    let total_files = discovered.len();
    let mut files = Vec::with_capacity(total_files);
    for file in discovered {
        if cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let canonical = PathBuf::from(&file.canonical_path);
        files.push(ScannedFile {
            id: 0,
            run_id,
            root_path: file.root_path,
            relative_path: file.relative_path,
            file_name: file.file_name,
            parent_dir: file.parent_dir,
            drive_letter: platform::get_drive_letter(&canonical)
                .map(|drive| drive.to_string_lossy().into_owned())
                .unwrap_or_default(),
            file_size: file.file_size as i64,
            last_modified: file.last_modified,
            partial_hash: None,
            content_hash: hashes_by_path.get(&file.canonical_path).copied(),
            file_identity: file.file_identity,
            warning_message: file.warning_message,
            marked_deleted: false,
            canonical_path: file.canonical_path,
        });
    }

    let mut persisted_rows = 0;
    for batch in files.chunks(256) {
        if cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        persisted_rows += db.insert_scanned_files(batch)?;
        progress.on_db_write_progress(persisted_rows, total_files);
    }

    let mut group_count = 0;
    for batch in groups.chunks(64) {
        if cancel_token.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        group_count += db.insert_duplicate_groups_cancellable(run_id, batch, cancel_token)?;
    }
    Ok(PersistedResults {
        groups: group_count,
        duplicate_files,
        duplicate_logical_bytes,
        wasted_bytes,
        warnings,
        warning_examples,
    })
}

fn warning_aggregate(
    phase: &str,
    code: &str,
    message: &str,
    count: usize,
    mut examples: Vec<String>,
    fallback_example: &str,
) -> Option<RunWarningAggregateInsert> {
    if count == 0 {
        return None;
    }
    examples.sort();
    examples.dedup();
    examples.truncate(3);
    if examples.is_empty() {
        examples.push(fallback_example.to_owned());
    }
    Some(RunWarningAggregateInsert {
        phase: phase.to_owned(),
        category: "scan".to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
        occurrence_count: count.min(i64::MAX as usize) as i64,
        examples,
    })
}

fn push_warning_example(examples: &mut Vec<String>, example: String) {
    if examples.len() < 3 && !examples.contains(&example) {
        examples.push(example);
    }
}

fn validate_discovered_snapshot(file: &scanner::DiscoveredFile) -> std::io::Result<()> {
    let metadata = fs::metadata(&file.canonical_path)?;
    let modified = metadata_modified_nanos(&metadata)?;
    if metadata.len() != file.file_size
        || (file.last_modified != 0 && modified != file.last_modified)
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file metadata no longer matches the discovery snapshot",
        ))
    } else {
        Ok(())
    }
}

fn metadata_modified_nanos(metadata: &fs::Metadata) -> std::io::Result<i64> {
    metadata.modified().and_then(|time| {
        time.duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    })
}

fn append_warning(target: &mut Option<String>, warning: String) {
    *target = Some(match target.take() {
        Some(previous) => format!("{previous}; {warning}"),
        None => warning,
    });
}

fn path_key(path: &std::path::Path) -> String {
    let value = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    {
        value.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        value
    }
}
