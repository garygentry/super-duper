use super::cache;
use super::repeat_cache::{
    compare_content_signatures, observe_content_signature, ContentSignatureObservation,
    ContentSignatureWindow, RepeatCacheLookup, RepeatHashCache, SystemContentSignatureProbe,
};
use super::scheduler::{execute_device_reads, DeviceReadPolicy, ScheduledRead};
use crate::progress::ProgressReporter;
use crate::storage::models::RepeatCachePolicy;
use dashmap::DashMap;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::hash::Hasher as _;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use twox_hash::XxHash64;

const PARTIAL_HASH_LENGTH: usize = 1024;
const ROTATIONAL_STREAM_BUFFER_LENGTH: usize = 64 * 1024;
const SOLID_STATE_STREAM_BUFFER_LENGTH: usize = 1024 * 1024;
pub(crate) const HASH_PROGRESS_FILE_QUANTUM: u64 = 256;
pub(crate) const FULL_READ_PROGRESS_BYTE_QUANTUM: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HashProgressDelta {
    pub files_hashed: u64,
    pub warning_count: u64,
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
    pub full_hash_request_bytes: u64,
    pub full_hash_cache_hits: u64,
    pub full_hash_cache_misses: u64,
    pub full_hash_cache_errors: u64,
    pub full_hash_cache_stores: u64,
    pub full_hash_content_reads_started: u64,
    pub full_hash_content_reads_completed: u64,
    pub full_hash_content_reads_failed: u64,
    pub full_hash_bytes_read: u64,
    pub full_hash_failures: u64,
    pub full_hash_failed_bytes: u64,
    pub unavailable_counters: u64,
    pub cancel_checks: u64,
    pub cancelled_work_items: u64,
    pub partial_screened_files: u64,
    pub partial_screened_bytes: u64,
    pub full_hash_satisfied_files: u64,
    pub full_hash_satisfied_bytes: u64,
    pub hash_pipeline_resolved_files: u64,
    pub hash_pipeline_resolved_bytes: u64,
}

impl HashProgressDelta {
    pub(crate) fn checked_add_assign(&mut self, other: &Self) -> io::Result<()> {
        macro_rules! add_fields {
            ($($field:ident),+ $(,)?) => { $(
                self.$field = self.$field.checked_add(other.$field).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData,
                        concat!("hash progress counter overflow: ", stringify!($field)))
                })?;
            )+ };
        }
        add_fields!(
            files_hashed,
            warning_count,
            partial_hashes_attempted,
            partial_hashes_succeeded,
            partial_hashes_failed,
            partial_hash_bytes_read,
            partial_hash_cache_hits,
            partial_hash_cache_misses,
            partial_hash_cache_errors,
            partial_hash_cache_stores,
            partial_collision_buckets,
            partial_collision_files,
            partial_collision_bytes,
            full_hash_requests,
            full_hash_request_bytes,
            full_hash_cache_hits,
            full_hash_cache_misses,
            full_hash_cache_errors,
            full_hash_cache_stores,
            full_hash_content_reads_started,
            full_hash_content_reads_completed,
            full_hash_content_reads_failed,
            full_hash_bytes_read,
            full_hash_failures,
            full_hash_failed_bytes,
            unavailable_counters,
            cancel_checks,
            cancelled_work_items,
            partial_screened_files,
            partial_screened_bytes,
            full_hash_satisfied_files,
            full_hash_satisfied_bytes,
            hash_pipeline_resolved_files,
            hash_pipeline_resolved_bytes,
        );
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

pub(crate) trait HashProgressSink: Send + Sync {
    fn publish(&self, delta: HashProgressDelta) -> io::Result<()>;
    fn snapshot(&self) -> HashProgressDelta;
}

#[derive(Default)]
struct LocalHashProgressSink {
    totals: Mutex<HashProgressDelta>,
}

impl HashProgressSink for LocalHashProgressSink {
    fn publish(&self, delta: HashProgressDelta) -> io::Result<()> {
        self.totals
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .checked_add_assign(&delta)
    }

    fn snapshot(&self) -> HashProgressDelta {
        self.totals
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PartialHashRead {
    pub hash: u64,
    pub physical_bytes_read: u64,
    pub cache_outcome: Option<cache::CacheLookupOutcome>,
    pub cache_stored: bool,
    pub warning: Option<String>,
    pub verified_signature: Option<super::repeat_cache::CacheSignatureKey>,
}

#[derive(Debug)]
pub(crate) struct FullHashRead {
    pub hash: u64,
    pub warning: Option<String>,
    pub cache_outcome: Option<cache::CacheLookupOutcome>,
    pub cache_stored: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FullHashIoEvent {
    CacheLookup(cache::CacheLookupOutcome),
    ContentReadStarted,
    ContentBytesRead(u64),
    CancellationCheck { cancelled: bool },
}

pub(crate) trait HashPipelineIo: Send + Sync {
    fn partial_hash(&self, path: &Path, cancel: &AtomicBool) -> io::Result<PartialHashRead>;
    fn full_hash(
        &self,
        path: &Path,
        partial_hash: u64,
        partial_signature: Option<&super::repeat_cache::CacheSignatureKey>,
        media: crate::platform::StorageMediaClass,
        cancel: &AtomicBool,
        observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
    ) -> io::Result<FullHashRead>;
}

pub(crate) trait HashDeviceMapper: Send + Sync {
    fn device_for_path(&self, path: &Path) -> crate::platform::StorageDevice;
}

#[derive(Default)]
struct SystemHashDeviceMapper {
    #[cfg(target_os = "windows")]
    devices_by_drive: Mutex<HashMap<String, crate::platform::StorageDevice>>,
}

impl HashDeviceMapper for SystemHashDeviceMapper {
    fn device_for_path(&self, path: &Path) -> crate::platform::StorageDevice {
        #[cfg(target_os = "windows")]
        {
            let key = crate::platform::get_drive_letter(path)
                .map(|drive| drive.to_string_lossy().to_ascii_uppercase())
                .unwrap_or_else(|| crate::platform::UNKNOWN_STORAGE_DEVICE_KEY.to_owned());
            let mut cache = self
                .devices_by_drive
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            return cache
                .entry(key)
                .or_insert_with(|| crate::platform::storage_device_for_path(path))
                .clone();
        }
        #[cfg(not(target_os = "windows"))]
        crate::platform::storage_device_for_path(path)
    }
}

pub(crate) struct SystemHashPipelineIo {
    repeat_cache: Option<Arc<RepeatHashCache>>,
    repeat_cache_policy: RepeatCachePolicy,
    startup_warning: Mutex<Option<String>>,
}

impl Default for SystemHashPipelineIo {
    fn default() -> Self {
        Self {
            repeat_cache: None,
            repeat_cache_policy: RepeatCachePolicy::default(),
            startup_warning: Mutex::new(None),
        }
    }
}

impl SystemHashPipelineIo {
    pub(crate) fn with_repeat_cache(
        repeat_cache: Arc<RepeatHashCache>,
        repeat_cache_policy: RepeatCachePolicy,
    ) -> Self {
        Self {
            repeat_cache: Some(repeat_cache),
            repeat_cache_policy,
            startup_warning: Mutex::new(None),
        }
    }

    pub(crate) fn with_unavailable_repeat_cache(
        repeat_cache_policy: RepeatCachePolicy,
        warning: String,
    ) -> Self {
        Self {
            repeat_cache: None,
            repeat_cache_policy,
            startup_warning: Mutex::new(Some(warning)),
        }
    }

    fn take_startup_warning(&self) -> Option<String> {
        self.startup_warning
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }

    fn lookup(
        &self,
        before: &ContentSignatureObservation,
        require_full: bool,
    ) -> (
        Option<u64>,
        Option<cache::CacheLookupOutcome>,
        Option<String>,
    ) {
        if self.repeat_cache_policy != RepeatCachePolicy::ReuseVerified {
            return (None, None, None);
        }
        let Some(cache_store) = self.repeat_cache.as_ref() else {
            return (None, Some(cache::CacheLookupOutcome::Error), None);
        };
        let ContentSignatureObservation::Qualified(signature) = before else {
            return (None, Some(cache::CacheLookupOutcome::Error), None);
        };
        match cache_store.lookup(signature) {
            Ok(RepeatCacheLookup::Hit(hashes)) => {
                let hash = if require_full {
                    hashes.full_hash
                } else {
                    Some(hashes.partial_hash)
                };
                match hash {
                    Some(hash) => (Some(hash), Some(cache::CacheLookupOutcome::Hit), None),
                    None => (None, Some(cache::CacheLookupOutcome::Miss), None),
                }
            }
            Ok(RepeatCacheLookup::Miss) => (None, Some(cache::CacheLookupOutcome::Miss), None),
            Ok(RepeatCacheLookup::Ineligible(reason)) => (
                None,
                Some(cache::CacheLookupOutcome::Error),
                Some(format!("Repeat cache entry is ineligible: {reason}")),
            ),
            Err(error) => (
                None,
                Some(cache::CacheLookupOutcome::Error),
                Some(format!("Repeat cache lookup failed: {error}")),
            ),
        }
    }
}

impl HashPipelineIo for SystemHashPipelineIo {
    fn partial_hash(&self, path: &Path, cancel: &AtomicBool) -> io::Result<PartialHashRead> {
        let probe = SystemContentSignatureProbe;
        let mut before = observe_content_signature(path, &probe);
        let (cached_hash, mut cache_outcome, mut warning) = self.lookup(&before, false);
        if let Some(startup_warning) = self.take_startup_warning() {
            append_warning(&mut warning, startup_warning);
        }
        if let Some(hash) = cached_hash {
            let after_lookup = observe_content_signature(path, &probe);
            match compare_content_signatures(before.clone(), after_lookup.clone()) {
                ContentSignatureWindow::Unchanged(signature) => {
                    return Ok(PartialHashRead {
                        hash,
                        physical_bytes_read: 0,
                        cache_outcome,
                        cache_stored: false,
                        warning,
                        verified_signature: Some(signature),
                    });
                }
                ContentSignatureWindow::Changed | ContentSignatureWindow::Ineligible(_) => {
                    cache_outcome = Some(cache::CacheLookupOutcome::Error);
                    before = after_lookup;
                }
            }
        }
        let data = read_portion(path, cancel)?;
        let hash = hash_data(&data);
        let mut cache_stored = false;
        let verified_signature =
            match compare_content_signatures(before, observe_content_signature(path, &probe)) {
                ContentSignatureWindow::Unchanged(signature) => Some(signature),
                ContentSignatureWindow::Changed => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "file changed while its partial hash was being read",
                    ));
                }
                ContentSignatureWindow::Ineligible(_) => None,
            };
        if let (Some(cache_store), Some(signature)) =
            (self.repeat_cache.as_ref(), verified_signature.as_ref())
        {
            match cache_store.store_partial(signature, hash) {
                Ok(super::repeat_cache::RepeatCacheStoreOutcome::Stored) => cache_stored = true,
                Ok(super::repeat_cache::RepeatCacheStoreOutcome::Replayed) => {}
                Err(error) => append_warning(
                    &mut warning,
                    format!("Repeat cache partial store failed: {error}"),
                ),
            }
        }
        Ok(PartialHashRead {
            hash,
            physical_bytes_read: data.len() as u64,
            cache_outcome,
            cache_stored,
            warning,
            verified_signature,
        })
    }

    fn full_hash(
        &self,
        path: &Path,
        partial_hash: u64,
        partial_signature: Option<&super::repeat_cache::CacheSignatureKey>,
        media: crate::platform::StorageMediaClass,
        cancel: &AtomicBool,
        observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
    ) -> io::Result<FullHashRead> {
        let probe = SystemContentSignatureProbe;
        let mut before = observe_content_signature(path, &probe);
        let signature_matches_partial = matches!(
            (&before, partial_signature),
            (ContentSignatureObservation::Qualified(current), Some(expected)) if current == expected
        );
        let (cached_hash, mut cache_outcome, mut warning) = if signature_matches_partial {
            self.lookup(&before, true)
        } else if self.repeat_cache_policy == RepeatCachePolicy::ReuseVerified {
            (None, Some(cache::CacheLookupOutcome::Error), None)
        } else {
            (None, None, None)
        };
        if let Some(hash) = cached_hash {
            let after_lookup = observe_content_signature(path, &probe);
            match compare_content_signatures(before.clone(), after_lookup.clone()) {
                ContentSignatureWindow::Unchanged(_) => {
                    observe(FullHashIoEvent::CacheLookup(cache::CacheLookupOutcome::Hit))?;
                    return Ok(FullHashRead {
                        hash,
                        warning,
                        cache_outcome,
                        cache_stored: false,
                    });
                }
                ContentSignatureWindow::Changed | ContentSignatureWindow::Ineligible(_) => {
                    cache_outcome = Some(cache::CacheLookupOutcome::Error);
                    before = after_lookup;
                }
            }
        }
        if let Some(outcome) = cache_outcome {
            observe(FullHashIoEvent::CacheLookup(outcome))?;
        }
        let hash = hash_file_streaming_observed_with_options(
            path,
            cancel,
            stream_buffer_length(media),
            stream_sequential_hint(media),
            observe,
        )?;
        let mut cache_stored = false;
        match compare_content_signatures(before, observe_content_signature(path, &probe)) {
            ContentSignatureWindow::Unchanged(signature) => {
                if let Some(cache_store) = self.repeat_cache.as_ref() {
                    if partial_signature.is_some_and(|expected| expected == &signature) {
                        match cache_store.store_full(&signature, partial_hash, hash) {
                            Ok(super::repeat_cache::RepeatCacheStoreOutcome::Stored) => {
                                cache_stored = true
                            }
                            Ok(super::repeat_cache::RepeatCacheStoreOutcome::Replayed) => {}
                            Err(error) => append_warning(
                                &mut warning,
                                format!("Repeat cache full store failed: {error}"),
                            ),
                        }
                    }
                }
            }
            ContentSignatureWindow::Changed => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "file changed while its full hash was being read",
                ));
            }
            ContentSignatureWindow::Ineligible(_) => {}
        }
        Ok(FullHashRead {
            hash,
            warning,
            cache_outcome,
            cache_stored,
        })
    }
}

fn append_warning(warning: &mut Option<String>, message: String) {
    *warning = Some(match warning.take() {
        Some(previous) => format!("{previous}; {message}"),
        None => message,
    });
}

struct HashProgressBatcher<'a> {
    sink: &'a dyn HashProgressSink,
    pending: Mutex<PendingHashProgress>,
    publication: Mutex<()>,
    cancellation_published: AtomicBool,
    closed: AtomicBool,
}

#[derive(Default)]
struct PendingHashProgress {
    delta: HashProgressDelta,
    file_outcomes: u64,
    full_bytes: u64,
}

impl<'a> HashProgressBatcher<'a> {
    fn new(sink: &'a dyn HashProgressSink) -> Self {
        Self {
            sink,
            pending: Mutex::new(PendingHashProgress::default()),
            publication: Mutex::new(()),
            cancellation_published: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        }
    }

    fn record(&self, delta: HashProgressDelta, outcomes: u64, force: bool) -> io::Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        if self.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        pending.delta.checked_add_assign(&delta)?;
        pending.file_outcomes = pending.file_outcomes.checked_add(outcomes).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "hash progress outcome overflow")
        })?;
        pending.full_bytes = pending
            .full_bytes
            .checked_add(delta.full_hash_bytes_read)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "hash progress byte overflow")
            })?;
        if !force
            && pending.file_outcomes < HASH_PROGRESS_FILE_QUANTUM
            && pending.full_bytes < FULL_READ_PROGRESS_BYTE_QUANTUM
        {
            return Ok(());
        }
        self.publish_pending(pending)
    }

    fn flush(&self) -> io::Result<()> {
        let pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        self.publish_pending(pending)
    }

    fn publish_pending(
        &self,
        mut pending: std::sync::MutexGuard<'_, PendingHashProgress>,
    ) -> io::Result<()> {
        let _publication = self.publication.lock().unwrap_or_else(|p| p.into_inner());
        let delta = std::mem::take(&mut pending.delta);
        pending.file_outcomes = 0;
        pending.full_bytes = 0;
        drop(pending);
        if !delta.is_empty() {
            self.sink.publish(delta)?;
        }
        Ok(())
    }

    fn cancellation_check(&self, cancelled: bool) -> io::Result<()> {
        if !cancelled {
            return self.record(
                HashProgressDelta {
                    cancel_checks: 1,
                    ..Default::default()
                },
                0,
                false,
            );
        }
        if self
            .cancellation_published
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
            self.closed.store(true, Ordering::Release);
            pending.delta.checked_add_assign(&HashProgressDelta {
                cancel_checks: 1,
                cancelled_work_items: 1,
                ..Default::default()
            })?;
            self.publish_pending(pending)?;
        }
        Ok(())
    }

    fn check_cancelled(&self, cancel: &AtomicBool) -> io::Result<()> {
        let cancelled = cancel.load(Ordering::Relaxed);
        self.cancellation_check(cancelled)?;
        if cancelled {
            Err(cancelled_error())
        } else {
            Ok(())
        }
    }
}

pub struct HashOutcome {
    pub confirmed_duplicates: DashMap<u64, Vec<PathBuf>>,
    pub files_hashed: usize,
    pub warning_count: usize,
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
    pub full_hash_request_bytes: u64,
    pub full_hash_cache_hits: u64,
    pub full_hash_cache_misses: u64,
    pub full_hash_cache_errors: u64,
    pub full_hash_cache_stores: u64,
    pub full_hash_content_reads_started: u64,
    pub full_hash_content_reads_completed: u64,
    pub full_hash_content_reads_failed: u64,
    pub full_hash_bytes_read: u64,
    pub full_hash_failures: u64,
    pub full_hash_failed_bytes: u64,
    pub unavailable_counters: u64,
    pub cancel_checks: u64,
    pub cancelled_work_items: u64,
    pub partial_screened_files: u64,
    pub partial_screened_bytes: u64,
    pub full_hash_satisfied_files: u64,
    pub full_hash_satisfied_bytes: u64,
    pub hash_pipeline_resolved_files: u64,
    pub hash_pipeline_resolved_bytes: u64,
}

pub fn build_content_hash_map(
    map: DashMap<u64, Vec<PathBuf>>,
    cancel: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> io::Result<DashMap<u64, Vec<PathBuf>>> {
    Ok(build_content_hash_map_with_stats(map, cancel, progress)?.confirmed_duplicates)
}

pub fn build_content_hash_map_with_stats(
    map: DashMap<u64, Vec<PathBuf>>,
    cancel: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> io::Result<HashOutcome> {
    let sink = LocalHashProgressSink::default();
    build_content_hash_map_with_progress(
        map,
        cancel,
        progress,
        &sink,
        &SystemHashPipelineIo::default(),
    )
}

pub(crate) fn build_content_hash_map_with_progress(
    map: DashMap<u64, Vec<PathBuf>>,
    cancel: &AtomicBool,
    progress: &dyn ProgressReporter,
    sink: &dyn HashProgressSink,
    io: &dyn HashPipelineIo,
) -> io::Result<HashOutcome> {
    build_content_hash_map_with_scheduler(
        map,
        cancel,
        progress,
        sink,
        io,
        &SystemHashDeviceMapper::default(),
        DeviceReadPolicy::default(),
    )
}

fn build_content_hash_map_with_scheduler(
    map: DashMap<u64, Vec<PathBuf>>,
    cancel: &AtomicBool,
    progress: &dyn ProgressReporter,
    sink: &dyn HashProgressSink,
    io: &dyn HashPipelineIo,
    device_mapper: &dyn HashDeviceMapper,
    policy: DeviceReadPolicy,
) -> io::Result<HashOutcome> {
    let confirmed_duplicates: DashMap<u64, Vec<PathBuf>> = DashMap::new();
    let mut buckets = map
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .collect::<Vec<_>>();
    buckets.sort_by(|left, right| right.0.cmp(&left.0));
    let total_files = buckets.iter().map(|(_, files)| files.len()).sum();
    let batcher = HashProgressBatcher::new(sink);

    struct PartialTask {
        file_size: u64,
        file: PathBuf,
    }
    struct PartialResult {
        file_size: u64,
        file: PathBuf,
        hash: Option<u64>,
        signature: Option<super::repeat_cache::CacheSignatureKey>,
    }
    let partial_tasks = buckets
        .into_iter()
        .flat_map(|(file_size, files)| {
            files.into_iter().map(move |file| ScheduledRead {
                device: device_mapper.device_for_path(&file),
                value: PartialTask { file_size, file },
            })
        })
        .collect();
    let scheduled_partials = execute_device_reads(partial_tasks, cancel, policy, |task| {
        batcher.check_cancelled(cancel)?;
        let PartialTask { file_size, file } = task;
        let mut delta = HashProgressDelta {
            partial_hashes_attempted: 1,
            partial_screened_files: 1,
            partial_screened_bytes: file_size,
            ..Default::default()
        };
        let mut signature = None;
        let hash = match io.partial_hash(&file, cancel) {
            Ok(read) => {
                delta.files_hashed = 1;
                delta.partial_hashes_succeeded = 1;
                delta.partial_hash_bytes_read = read.physical_bytes_read;
                match read.cache_outcome {
                    Some(cache::CacheLookupOutcome::Hit) => delta.partial_hash_cache_hits = 1,
                    Some(cache::CacheLookupOutcome::Miss) => delta.partial_hash_cache_misses = 1,
                    Some(cache::CacheLookupOutcome::Error) => delta.partial_hash_cache_errors = 1,
                    None => {}
                }
                if read.cache_stored {
                    delta.partial_hash_cache_stores = 1;
                }
                if let Some(warning) = read.warning {
                    tracing::warn!("{}: {}", file.display(), warning);
                    delta.warning_count = 1;
                }
                signature = read.verified_signature;
                Some(read.hash)
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                batcher.cancellation_check(true)?;
                return Err(error);
            }
            Err(error) => {
                tracing::error!("Error processing file '{}': {}", file.display(), error);
                delta.partial_hashes_failed = 1;
                delta.warning_count = 1;
                None
            }
        };
        batcher.record(delta, 1, false)?;
        Ok(PartialResult {
            file_size,
            file,
            hash,
            signature,
        })
    });
    let partial_results = match scheduled_partials {
        Ok(results) => results,
        Err(error) if error.kind() == io::ErrorKind::Interrupted => {
            batcher.cancellation_check(true)?;
            return Err(error);
        }
        Err(error) => {
            batcher.flush()?;
            return Err(error);
        }
    };
    emit_legacy_progress(progress, &sink.snapshot(), total_files);

    let mut partial_groups =
        HashMap::<(u64, u64), Vec<(PathBuf, Option<super::repeat_cache::CacheSignatureKey>)>>::new(
        );
    let mut screened_by_size = HashMap::<u64, u64>::new();
    for result in partial_results {
        *screened_by_size.entry(result.file_size).or_default() += 1;
        if let Some(hash) = result.hash {
            partial_groups
                .entry((result.file_size, hash))
                .or_default()
                .push((result.file, result.signature));
        }
    }

    let mut collision_by_size = HashMap::<u64, (u64, u64)>::new();
    let mut full_tasks = Vec::new();
    for ((file_size, partial_hash), files) in partial_groups {
        if files.len() <= 1 {
            continue;
        }
        let collision = collision_by_size.entry(file_size).or_default();
        collision.0 += 1;
        collision.1 += files.len() as u64;
        full_tasks.extend(files.into_iter().map(|(file, signature)| {
            let device = device_mapper.device_for_path(&file);
            let media = device.media;
            ScheduledRead {
                device,
                value: (file_size, partial_hash, signature, file, media),
            }
        }));
    }
    for (file_size, screened_files) in screened_by_size {
        let (collision_bucket_count, collision_files) = collision_by_size
            .get(&file_size)
            .copied()
            .unwrap_or_default();
        let collision_bytes = collision_files.saturating_mul(file_size);
        let resolved_files = screened_files.saturating_sub(collision_files);
        batcher.record(
            HashProgressDelta {
                partial_collision_buckets: collision_bucket_count,
                partial_collision_files: collision_files,
                partial_collision_bytes: collision_bytes,
                full_hash_requests: collision_files,
                full_hash_request_bytes: collision_bytes,
                hash_pipeline_resolved_files: resolved_files,
                hash_pipeline_resolved_bytes: resolved_files.saturating_mul(file_size),
                ..Default::default()
            },
            0,
            false,
        )?;
    }

    let scheduled_full_hashes = execute_device_reads(
        full_tasks,
        cancel,
        policy,
        |(file_size, partial_hash, signature, file, media)| {
            batcher.check_cancelled(cancel)?;
            populate_full_hash(
                &file,
                file_size,
                partial_hash,
                signature.as_ref(),
                media,
                cancel,
                &batcher,
                io,
            )
            .map(|hash| (file_size, file, hash))
        },
    );
    let full_results = match scheduled_full_hashes {
        Ok(results) => results,
        Err(error) if error.kind() == io::ErrorKind::Interrupted => {
            batcher.cancellation_check(true)?;
            return Err(error);
        }
        Err(error) => {
            batcher.flush()?;
            return Err(error);
        }
    };
    let mut full_groups = HashMap::<(u64, u64), Vec<PathBuf>>::new();
    for (file_size, file, hash) in full_results {
        if let Some(hash) = hash {
            full_groups.entry((file_size, hash)).or_default().push(file);
        }
    }
    for ((_, hash), files) in full_groups {
        if files.len() > 1 {
            confirmed_duplicates.entry(hash).or_default().extend(files);
        }
    }

    batcher.flush()?;
    let totals = sink.snapshot();
    emit_legacy_progress(progress, &totals, total_files);
    Ok(HashOutcome::from_progress(confirmed_duplicates, totals))
}

fn emit_legacy_progress(progress: &dyn ProgressReporter, totals: &HashProgressDelta, total: usize) {
    progress.on_hash_progress_detailed(
        totals.files_hashed.min(usize::MAX as u64) as usize,
        total,
        totals.warning_count.min(usize::MAX as u64) as usize,
        None,
    );
}

impl HashOutcome {
    fn from_progress(
        confirmed_duplicates: DashMap<u64, Vec<PathBuf>>,
        value: HashProgressDelta,
    ) -> Self {
        Self {
            confirmed_duplicates,
            files_hashed: value.files_hashed.min(usize::MAX as u64) as usize,
            warning_count: value.warning_count.min(usize::MAX as u64) as usize,
            partial_hashes_attempted: value.partial_hashes_attempted,
            partial_hashes_succeeded: value.partial_hashes_succeeded,
            partial_hashes_failed: value.partial_hashes_failed,
            partial_hash_bytes_read: value.partial_hash_bytes_read,
            partial_hash_cache_hits: value.partial_hash_cache_hits,
            partial_hash_cache_misses: value.partial_hash_cache_misses,
            partial_hash_cache_errors: value.partial_hash_cache_errors,
            partial_hash_cache_stores: value.partial_hash_cache_stores,
            partial_collision_buckets: value.partial_collision_buckets,
            partial_collision_files: value.partial_collision_files,
            partial_collision_bytes: value.partial_collision_bytes,
            full_hash_requests: value.full_hash_requests,
            full_hash_request_bytes: value.full_hash_request_bytes,
            full_hash_cache_hits: value.full_hash_cache_hits,
            full_hash_cache_misses: value.full_hash_cache_misses,
            full_hash_cache_errors: value.full_hash_cache_errors,
            full_hash_cache_stores: value.full_hash_cache_stores,
            full_hash_content_reads_started: value.full_hash_content_reads_started,
            full_hash_content_reads_completed: value.full_hash_content_reads_completed,
            full_hash_content_reads_failed: value.full_hash_content_reads_failed,
            full_hash_bytes_read: value.full_hash_bytes_read,
            full_hash_failures: value.full_hash_failures,
            full_hash_failed_bytes: value.full_hash_failed_bytes,
            unavailable_counters: value.unavailable_counters,
            cancel_checks: value.cancel_checks,
            cancelled_work_items: value.cancelled_work_items,
            partial_screened_files: value.partial_screened_files,
            partial_screened_bytes: value.partial_screened_bytes,
            full_hash_satisfied_files: value.full_hash_satisfied_files,
            full_hash_satisfied_bytes: value.full_hash_satisfied_bytes,
            hash_pipeline_resolved_files: value.hash_pipeline_resolved_files,
            hash_pipeline_resolved_bytes: value.hash_pipeline_resolved_bytes,
        }
    }
}

fn populate_full_hash(
    file: &Path,
    file_size: u64,
    partial_hash: u64,
    partial_signature: Option<&super::repeat_cache::CacheSignatureKey>,
    media: crate::platform::StorageMediaClass,
    cancel: &AtomicBool,
    batcher: &HashProgressBatcher<'_>,
    io: &dyn HashPipelineIo,
) -> io::Result<Option<u64>> {
    let mut lookup = None;
    let mut content_started = false;
    let result = io.full_hash(
        file,
        partial_hash,
        partial_signature,
        media,
        cancel,
        &mut |event| match event {
            FullHashIoEvent::CacheLookup(outcome) => {
                lookup = Some(outcome);
                let mut delta = HashProgressDelta::default();
                match outcome {
                    cache::CacheLookupOutcome::Hit => delta.full_hash_cache_hits = 1,
                    cache::CacheLookupOutcome::Miss => delta.full_hash_cache_misses = 1,
                    cache::CacheLookupOutcome::Error => delta.full_hash_cache_errors = 1,
                }
                batcher.record(delta, 0, false)
            }
            FullHashIoEvent::ContentReadStarted => {
                content_started = true;
                batcher.record(
                    HashProgressDelta {
                        full_hash_content_reads_started: 1,
                        ..Default::default()
                    },
                    0,
                    false,
                )
            }
            FullHashIoEvent::ContentBytesRead(bytes) => batcher.record(
                HashProgressDelta {
                    full_hash_bytes_read: bytes,
                    ..Default::default()
                },
                0,
                false,
            ),
            FullHashIoEvent::CancellationCheck { cancelled } => {
                batcher.cancellation_check(cancelled)
            }
        },
    );

    match result {
        Ok(outcome) => {
            if lookup != outcome.cache_outcome {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "full hash I/O returned conflicting cache lookup outcome",
                ));
            }
            if outcome.cache_outcome != Some(cache::CacheLookupOutcome::Hit) && !content_started {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "cache fallback completed without content-read start",
                ));
            }
            let mut delta = HashProgressDelta {
                full_hash_satisfied_files: 1,
                full_hash_satisfied_bytes: file_size,
                hash_pipeline_resolved_files: 1,
                hash_pipeline_resolved_bytes: file_size,
                ..Default::default()
            };
            if content_started {
                delta.full_hash_content_reads_completed = 1;
            }
            if outcome.cache_stored {
                delta.full_hash_cache_stores = 1;
            }
            if let Some(warning) = outcome.warning {
                delta.warning_count = 1;
                tracing::warn!("{}: {}", file.display(), warning);
            }
            batcher.record(delta, 1, false)?;
            Ok(Some(outcome.hash))
        }
        Err(error) if error.kind() == io::ErrorKind::Interrupted => {
            batcher.cancellation_check(true)?;
            Err(error)
        }
        Err(error) => {
            tracing::error!("Error processing file '{}': {}", file.display(), error);
            let mut delta = HashProgressDelta {
                warning_count: 1,
                full_hash_failures: 1,
                full_hash_failed_bytes: file_size,
                hash_pipeline_resolved_files: 1,
                hash_pipeline_resolved_bytes: file_size,
                ..Default::default()
            };
            if content_started {
                delta.full_hash_content_reads_failed = 1;
            }
            if lookup.is_none() {
                delta.unavailable_counters = 1;
            }
            batcher.record(delta, 1, false)?;
            Ok(None)
        }
    }
}

fn read_portion(path: &Path, cancel: &AtomicBool) -> io::Result<Vec<u8>> {
    check_cancelled(cancel)?;
    let mut file = File::open(path)?;
    let mut buffer = vec![0; PARTIAL_HASH_LENGTH];
    let bytes = file.read(&mut buffer)?;
    buffer.truncate(bytes);
    Ok(buffer)
}

pub fn hash_file_streaming(path: &Path, cancel: &AtomicBool) -> io::Result<u64> {
    hash_file_streaming_observed(path, cancel, &mut |_| Ok(()))
}

pub(crate) fn hash_file_streaming_observed(
    path: &Path,
    cancel: &AtomicBool,
    observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
) -> io::Result<u64> {
    hash_file_streaming_observed_with_options(
        path,
        cancel,
        ROTATIONAL_STREAM_BUFFER_LENGTH,
        true,
        observe,
    )
}

pub(crate) fn hash_file_streaming_observed_with_options(
    path: &Path,
    cancel: &AtomicBool,
    buffer_length: usize,
    sequential_hint: bool,
    observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
) -> io::Result<u64> {
    if buffer_length == 0 || buffer_length > SOLID_STATE_STREAM_BUFFER_LENGTH {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "hash stream buffer is outside the measured SOP7 bound",
        ));
    }
    let mut file = open_streaming_file(path, sequential_hint)?;
    observe(FullHashIoEvent::ContentReadStarted)?;
    let mut buffer = vec![0_u8; buffer_length];
    let mut hasher = XxHash64::with_seed(0);
    let mut pending_bytes = 0_u64;
    loop {
        let cancelled = cancel.load(Ordering::Relaxed);
        observe(FullHashIoEvent::CancellationCheck { cancelled })?;
        if cancelled {
            publish_pending_bytes(observe, &mut pending_bytes)?;
            return Err(cancelled_error());
        }
        let bytes = match file.read(&mut buffer) {
            Ok(bytes) => bytes,
            Err(error) => {
                publish_pending_bytes(observe, &mut pending_bytes)?;
                return Err(error);
            }
        };
        if bytes == 0 {
            break;
        }
        hasher.write(&buffer[..bytes]);
        pending_bytes = pending_bytes.saturating_add(bytes as u64);
        if pending_bytes >= FULL_READ_PROGRESS_BYTE_QUANTUM {
            publish_pending_bytes(observe, &mut pending_bytes)?;
        }
    }
    publish_pending_bytes(observe, &mut pending_bytes)?;
    Ok(hasher.finish())
}

pub(crate) fn stream_buffer_length(media: crate::platform::StorageMediaClass) -> usize {
    match media {
        crate::platform::StorageMediaClass::SolidState => SOLID_STATE_STREAM_BUFFER_LENGTH,
        crate::platform::StorageMediaClass::Rotational
        | crate::platform::StorageMediaClass::Unknown => ROTATIONAL_STREAM_BUFFER_LENGTH,
    }
}

pub(crate) fn stream_sequential_hint(media: crate::platform::StorageMediaClass) -> bool {
    media != crate::platform::StorageMediaClass::SolidState
}

fn open_streaming_file(path: &Path, sequential_hint: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "windows")]
    if sequential_hint {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;
        options.custom_flags(FILE_FLAG_SEQUENTIAL_SCAN);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = sequential_hint;
    options.open(path)
}

fn publish_pending_bytes(
    observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
    pending: &mut u64,
) -> io::Result<()> {
    if *pending > 0 {
        observe(FullHashIoEvent::ContentBytesRead(*pending))?;
        *pending = 0;
    }
    Ok(())
}

pub fn hash_data(data: &[u8]) -> u64 {
    let mut hasher = XxHash64::with_seed(0);
    hasher.write(data);
    hasher.finish()
}

fn check_cancelled(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(cancelled_error())
    } else {
        Ok(())
    }
}

fn cancelled_error() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "hashing cancelled")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hasher::repeat_cache;
    use std::fs;
    use std::sync::atomic::AtomicUsize;
    use tempfile::TempDir;

    fn uncached_partial(hash: u64, physical_bytes_read: u64) -> PartialHashRead {
        PartialHashRead {
            hash,
            physical_bytes_read,
            cache_outcome: None,
            cache_stored: false,
            warning: None,
            verified_signature: None,
        }
    }

    #[derive(Default)]
    struct RecordingSink {
        total: Mutex<HashProgressDelta>,
        history: Mutex<Vec<HashProgressDelta>>,
    }

    impl HashProgressSink for RecordingSink {
        fn publish(&self, delta: HashProgressDelta) -> io::Result<()> {
            let snapshot = {
                let mut total = self.total.lock().unwrap();
                total.checked_add_assign(&delta)?;
                total.clone()
            };
            self.history.lock().unwrap().push(snapshot);
            Ok(())
        }

        fn snapshot(&self) -> HashProgressDelta {
            self.total.lock().unwrap().clone()
        }
    }

    struct UniquePartialIo;

    impl HashPipelineIo for UniquePartialIo {
        fn partial_hash(&self, path: &Path, _cancel: &AtomicBool) -> io::Result<PartialHashRead> {
            Ok(uncached_partial(
                hash_data(path.to_string_lossy().as_bytes()),
                PARTIAL_HASH_LENGTH as u64,
            ))
        }

        fn full_hash(
            &self,
            _path: &Path,
            _partial_hash: u64,
            _partial_signature: Option<&repeat_cache::CacheSignatureKey>,
            _media: crate::platform::StorageMediaClass,
            _cancel: &AtomicBool,
            _observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
        ) -> io::Result<FullHashRead> {
            panic!("unique partial hashes must not request a full hash")
        }
    }

    #[derive(Default)]
    struct OrderedPartialIo {
        paths: Mutex<Vec<PathBuf>>,
    }

    impl HashPipelineIo for OrderedPartialIo {
        fn partial_hash(&self, path: &Path, _cancel: &AtomicBool) -> io::Result<PartialHashRead> {
            self.paths.lock().unwrap().push(path.to_path_buf());
            Ok(uncached_partial(
                hash_data(path.to_string_lossy().as_bytes()),
                PARTIAL_HASH_LENGTH as u64,
            ))
        }

        fn full_hash(
            &self,
            _path: &Path,
            _partial_hash: u64,
            _partial_signature: Option<&repeat_cache::CacheSignatureKey>,
            _media: crate::platform::StorageMediaClass,
            _cancel: &AtomicBool,
            _observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
        ) -> io::Result<FullHashRead> {
            panic!("unique partial hashes must not request a full hash")
        }
    }

    #[derive(Default)]
    struct RecordingPipelineIo {
        partial_paths: Mutex<Vec<PathBuf>>,
        full_paths: Mutex<Vec<PathBuf>>,
        partial_opens: AtomicUsize,
        full_opens: AtomicUsize,
    }

    impl HashPipelineIo for RecordingPipelineIo {
        fn partial_hash(&self, path: &Path, _cancel: &AtomicBool) -> io::Result<PartialHashRead> {
            self.partial_opens.fetch_add(1, Ordering::Relaxed);
            self.partial_paths.lock().unwrap().push(path.to_path_buf());
            let name = path.to_string_lossy();
            let hash = if name.starts_with("duplicate-") {
                7
            } else {
                hash_data(name.as_bytes())
            };
            Ok(uncached_partial(hash, PARTIAL_HASH_LENGTH as u64))
        }

        fn full_hash(
            &self,
            path: &Path,
            _partial_hash: u64,
            _partial_signature: Option<&repeat_cache::CacheSignatureKey>,
            _media: crate::platform::StorageMediaClass,
            _cancel: &AtomicBool,
            observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
        ) -> io::Result<FullHashRead> {
            self.full_opens.fetch_add(1, Ordering::Relaxed);
            self.full_paths.lock().unwrap().push(path.to_path_buf());
            observe(FullHashIoEvent::CacheLookup(
                cache::CacheLookupOutcome::Miss,
            ))?;
            observe(FullHashIoEvent::ContentReadStarted)?;
            observe(FullHashIoEvent::ContentBytesRead(4_096))?;
            Ok(FullHashRead {
                hash: 11,
                warning: None,
                cache_outcome: Some(cache::CacheLookupOutcome::Miss),
                cache_stored: false,
            })
        }
    }

    struct TwoRotationalDeviceMapper;

    impl HashDeviceMapper for TwoRotationalDeviceMapper {
        fn device_for_path(&self, path: &Path) -> crate::platform::StorageDevice {
            let key = if path.to_string_lossy().starts_with("disk-a") {
                "physical:a"
            } else {
                "physical:b"
            };
            crate::platform::StorageDevice {
                key: key.to_owned(),
                media: crate::platform::StorageMediaClass::Rotational,
            }
        }
    }

    #[derive(Default)]
    struct SchedulingPipelineIo {
        active_by_device: Mutex<HashMap<String, usize>>,
        maximum_by_device: Mutex<HashMap<String, usize>>,
        active_total: AtomicUsize,
        maximum_total: AtomicUsize,
    }

    impl SchedulingPipelineIo {
        fn read<T>(&self, path: &Path, value: T) -> T {
            let device = if path.to_string_lossy().starts_with("disk-a") {
                "physical:a"
            } else {
                "physical:b"
            };
            let active_for_device = {
                let mut active = self.active_by_device.lock().unwrap();
                let current = active.entry(device.to_owned()).or_default();
                *current += 1;
                *current
            };
            {
                let mut maximum = self.maximum_by_device.lock().unwrap();
                maximum
                    .entry(device.to_owned())
                    .and_modify(|value| *value = (*value).max(active_for_device))
                    .or_insert(active_for_device);
            }
            let active_total = self.active_total.fetch_add(1, Ordering::SeqCst) + 1;
            let mut observed = self.maximum_total.load(Ordering::SeqCst);
            while active_total > observed {
                match self.maximum_total.compare_exchange_weak(
                    observed,
                    active_total,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(current) => observed = current,
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
            self.active_total.fetch_sub(1, Ordering::SeqCst);
            *self
                .active_by_device
                .lock()
                .unwrap()
                .get_mut(device)
                .unwrap() -= 1;
            value
        }
    }

    impl HashPipelineIo for SchedulingPipelineIo {
        fn partial_hash(&self, path: &Path, _cancel: &AtomicBool) -> io::Result<PartialHashRead> {
            Ok(self.read(path, uncached_partial(7, PARTIAL_HASH_LENGTH as u64)))
        }

        fn full_hash(
            &self,
            path: &Path,
            _partial_hash: u64,
            _partial_signature: Option<&repeat_cache::CacheSignatureKey>,
            _media: crate::platform::StorageMediaClass,
            _cancel: &AtomicBool,
            observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
        ) -> io::Result<FullHashRead> {
            observe(FullHashIoEvent::CacheLookup(
                cache::CacheLookupOutcome::Miss,
            ))?;
            observe(FullHashIoEvent::ContentReadStarted)?;
            observe(FullHashIoEvent::ContentBytesRead(4_096))?;
            Ok(self.read(
                path,
                FullHashRead {
                    hash: 11,
                    warning: None,
                    cache_outcome: Some(cache::CacheLookupOutcome::Miss),
                    cache_stored: false,
                },
            ))
        }
    }

    struct LongFullReadIo;

    impl HashPipelineIo for LongFullReadIo {
        fn partial_hash(&self, _path: &Path, _cancel: &AtomicBool) -> io::Result<PartialHashRead> {
            Ok(uncached_partial(7, PARTIAL_HASH_LENGTH as u64))
        }

        fn full_hash(
            &self,
            _path: &Path,
            _partial_hash: u64,
            _partial_signature: Option<&repeat_cache::CacheSignatureKey>,
            _media: crate::platform::StorageMediaClass,
            _cancel: &AtomicBool,
            observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
        ) -> io::Result<FullHashRead> {
            observe(FullHashIoEvent::CacheLookup(
                cache::CacheLookupOutcome::Miss,
            ))?;
            observe(FullHashIoEvent::ContentReadStarted)?;
            observe(FullHashIoEvent::ContentBytesRead(
                FULL_READ_PROGRESS_BYTE_QUANTUM,
            ))?;
            Ok(FullHashRead {
                hash: 11,
                warning: None,
                cache_outcome: Some(cache::CacheLookupOutcome::Miss),
                cache_stored: false,
            })
        }
    }

    #[derive(Clone, Copy)]
    enum FullScenario {
        Hit,
        MissStored,
        LookupErrorFallback,
        ReadFailure,
        StoreError,
    }

    struct ScenarioIo(FullScenario);

    impl HashPipelineIo for ScenarioIo {
        fn partial_hash(&self, _path: &Path, _cancel: &AtomicBool) -> io::Result<PartialHashRead> {
            Ok(uncached_partial(5, PARTIAL_HASH_LENGTH as u64))
        }

        fn full_hash(
            &self,
            _path: &Path,
            _partial_hash: u64,
            _partial_signature: Option<&repeat_cache::CacheSignatureKey>,
            _media: crate::platform::StorageMediaClass,
            _cancel: &AtomicBool,
            observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
        ) -> io::Result<FullHashRead> {
            let lookup = match self.0 {
                FullScenario::Hit => cache::CacheLookupOutcome::Hit,
                FullScenario::LookupErrorFallback => cache::CacheLookupOutcome::Error,
                _ => cache::CacheLookupOutcome::Miss,
            };
            observe(FullHashIoEvent::CacheLookup(lookup))?;
            if matches!(self.0, FullScenario::Hit) {
                return Ok(FullHashRead {
                    hash: 13,
                    warning: None,
                    cache_outcome: Some(lookup),
                    cache_stored: false,
                });
            }
            observe(FullHashIoEvent::ContentReadStarted)?;
            observe(FullHashIoEvent::ContentBytesRead(123))?;
            if matches!(self.0, FullScenario::ReadFailure) {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "injected read failure",
                ));
            }
            Ok(FullHashRead {
                hash: 13,
                warning: match self.0 {
                    FullScenario::LookupErrorFallback => Some("injected lookup error".to_owned()),
                    FullScenario::StoreError => Some("injected store error".to_owned()),
                    _ => None,
                },
                cache_outcome: Some(lookup),
                cache_stored: matches!(self.0, FullScenario::MissStored),
            })
        }
    }

    struct CancelFullIo;

    impl HashPipelineIo for CancelFullIo {
        fn partial_hash(&self, _path: &Path, _cancel: &AtomicBool) -> io::Result<PartialHashRead> {
            Ok(uncached_partial(17, PARTIAL_HASH_LENGTH as u64))
        }

        fn full_hash(
            &self,
            _path: &Path,
            _partial_hash: u64,
            _partial_signature: Option<&repeat_cache::CacheSignatureKey>,
            _media: crate::platform::StorageMediaClass,
            _cancel: &AtomicBool,
            observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
        ) -> io::Result<FullHashRead> {
            observe(FullHashIoEvent::CacheLookup(
                cache::CacheLookupOutcome::Miss,
            ))?;
            observe(FullHashIoEvent::ContentReadStarted)?;
            observe(FullHashIoEvent::ContentBytesRead(123))?;
            observe(FullHashIoEvent::CancellationCheck { cancelled: true })?;
            Err(cancelled_error())
        }
    }

    fn run_scenario(scenario: FullScenario) -> HashOutcome {
        let map = DashMap::new();
        map.insert(4_096, vec![PathBuf::from("left"), PathBuf::from("right")]);
        let sink = RecordingSink::default();
        build_content_hash_map_with_progress(
            map,
            &AtomicBool::new(false),
            &crate::progress::SilentReporter,
            &sink,
            &ScenarioIo(scenario),
        )
        .unwrap()
    }

    #[test]
    fn qualified_repeat_cache_reuses_both_hash_stages_after_reopen_and_rejects_edits() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("candidate.bin");
        let cache_path = temp.path().join("repeat-cache");
        fs::write(&file, vec![0x41; 4_096]).unwrap();
        let cancel = AtomicBool::new(false);

        let (partial_hash, full_hash) = {
            let cache_store = Arc::new(RepeatHashCache::open(&cache_path).unwrap());
            let io = SystemHashPipelineIo::with_repeat_cache(
                cache_store,
                RepeatCachePolicy::RevalidateContent,
            );
            let partial = io.partial_hash(&file, &cancel).unwrap();
            assert_eq!(partial.cache_outcome, None);
            assert!(partial.cache_stored);
            let mut events = Vec::new();
            let full = io
                .full_hash(
                    &file,
                    partial.hash,
                    partial.verified_signature.as_ref(),
                    crate::platform::StorageMediaClass::SolidState,
                    &cancel,
                    &mut |event| {
                        events.push(event);
                        Ok(())
                    },
                )
                .unwrap();
            assert_eq!(full.cache_outcome, None);
            assert!(full.cache_stored);
            assert!(events.contains(&FullHashIoEvent::ContentReadStarted));
            (partial.hash, full.hash)
        };

        {
            let cache_store = Arc::new(RepeatHashCache::open(&cache_path).unwrap());
            let io = SystemHashPipelineIo::with_repeat_cache(
                cache_store,
                RepeatCachePolicy::ReuseVerified,
            );
            let partial = io.partial_hash(&file, &cancel).unwrap();
            assert_eq!(partial.hash, partial_hash);
            assert_eq!(partial.physical_bytes_read, 0);
            assert_eq!(partial.cache_outcome, Some(cache::CacheLookupOutcome::Hit));
            let mut events = Vec::new();
            let full = io
                .full_hash(
                    &file,
                    partial.hash,
                    partial.verified_signature.as_ref(),
                    crate::platform::StorageMediaClass::SolidState,
                    &cancel,
                    &mut |event| {
                        events.push(event);
                        Ok(())
                    },
                )
                .unwrap();
            assert_eq!(full.hash, full_hash);
            assert_eq!(full.cache_outcome, Some(cache::CacheLookupOutcome::Hit));
            assert_eq!(
                events,
                vec![FullHashIoEvent::CacheLookup(cache::CacheLookupOutcome::Hit)]
            );

            std::thread::sleep(std::time::Duration::from_millis(2));
            fs::write(&file, vec![0x42; 4_096]).unwrap();
            let changed = io.partial_hash(&file, &cancel).unwrap();
            assert_ne!(changed.hash, partial_hash);
            assert_ne!(changed.cache_outcome, Some(cache::CacheLookupOutcome::Hit));
            assert!(changed.physical_bytes_read > 0);
        }
    }

    #[test]
    fn repeat_cache_pipeline_reconciles_exact_partial_full_hits_stores_and_bytes() {
        let temp = TempDir::new().unwrap();
        let left = temp.path().join("left.bin");
        let right = temp.path().join("right.bin");
        let cache_path = temp.path().join("repeat-cache");
        fs::write(&left, vec![0x5a; 4_096]).unwrap();
        fs::write(&right, vec![0x5a; 4_096]).unwrap();
        let candidates = || {
            let map = DashMap::new();
            map.insert(4_096, vec![left.clone(), right.clone()]);
            map
        };

        let forced = {
            let io = SystemHashPipelineIo::with_repeat_cache(
                Arc::new(RepeatHashCache::open(&cache_path).unwrap()),
                RepeatCachePolicy::RevalidateContent,
            );
            let sink = RecordingSink::default();
            build_content_hash_map_with_progress(
                candidates(),
                &AtomicBool::new(false),
                &crate::progress::SilentReporter,
                &sink,
                &io,
            )
            .unwrap()
        };
        assert_eq!(forced.confirmed_duplicates.len(), 1);
        assert_eq!(forced.partial_hash_cache_stores, 2);
        assert_eq!(forced.full_hash_cache_stores, 2);
        assert_eq!(
            forced.partial_hash_bytes_read,
            2 * PARTIAL_HASH_LENGTH as u64
        );
        assert_eq!(forced.full_hash_bytes_read, 8_192);
        assert_eq!(forced.partial_hash_cache_hits, 0);
        assert_eq!(forced.full_hash_cache_hits, 0);

        let reused = {
            let io = SystemHashPipelineIo::with_repeat_cache(
                Arc::new(RepeatHashCache::open(&cache_path).unwrap()),
                RepeatCachePolicy::ReuseVerified,
            );
            let sink = RecordingSink::default();
            build_content_hash_map_with_progress(
                candidates(),
                &AtomicBool::new(false),
                &crate::progress::SilentReporter,
                &sink,
                &io,
            )
            .unwrap()
        };
        assert_eq!(reused.confirmed_duplicates.len(), 1);
        assert_eq!(reused.partial_hash_cache_hits, 2);
        assert_eq!(reused.full_hash_cache_hits, 2);
        assert_eq!(reused.partial_hash_bytes_read, 0);
        assert_eq!(reused.full_hash_bytes_read, 0);
        assert_eq!(reused.partial_hash_cache_stores, 0);
        assert_eq!(reused.full_hash_cache_stores, 0);
        assert_eq!(
            *reused.confirmed_duplicates.iter().next().unwrap().key(),
            *forced.confirmed_duplicates.iter().next().unwrap().key()
        );
    }

    #[test]
    fn mutation_between_partial_and_full_never_stores_a_stale_partial_hash() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("changing.bin");
        let cache_path = temp.path().join("repeat-cache");
        fs::write(&file, vec![0x11; 4_096]).unwrap();
        let cancel = AtomicBool::new(false);

        let io = SystemHashPipelineIo::with_repeat_cache(
            Arc::new(RepeatHashCache::open(&cache_path).unwrap()),
            RepeatCachePolicy::RevalidateContent,
        );
        let partial = io.partial_hash(&file, &cancel).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        fs::write(&file, vec![0x22; 4_096]).unwrap();
        let full = io
            .full_hash(
                &file,
                partial.hash,
                partial.verified_signature.as_ref(),
                crate::platform::StorageMediaClass::SolidState,
                &cancel,
                &mut |_| Ok(()),
            )
            .unwrap();
        assert!(!full.cache_stored);
        drop(io);

        let reused = SystemHashPipelineIo::with_repeat_cache(
            Arc::new(RepeatHashCache::open(&cache_path).unwrap()),
            RepeatCachePolicy::ReuseVerified,
        )
        .partial_hash(&file, &cancel)
        .unwrap();
        assert_ne!(reused.hash, partial.hash);
        assert_ne!(reused.cache_outcome, Some(cache::CacheLookupOutcome::Hit));
        assert!(reused.physical_bytes_read > 0);
    }

    #[test]
    fn singleton_buckets_never_open_content_and_other_buckets_keep_both_hash_paths() {
        let map = DashMap::new();
        map.insert(123, vec![PathBuf::from("singleton")]);
        map.insert(
            2_048,
            vec![
                PathBuf::from("partial-left"),
                PathBuf::from("partial-right"),
            ],
        );
        map.insert(
            4_096,
            vec![
                PathBuf::from("duplicate-left"),
                PathBuf::from("duplicate-right"),
            ],
        );
        let sink = RecordingSink::default();
        let io = RecordingPipelineIo::default();
        let outcome = build_content_hash_map_with_progress(
            map,
            &AtomicBool::new(false),
            &crate::progress::SilentReporter,
            &sink,
            &io,
        )
        .unwrap();

        let mut partial_paths = io.partial_paths.lock().unwrap().clone();
        partial_paths.sort();
        assert_eq!(
            partial_paths,
            vec![
                PathBuf::from("duplicate-left"),
                PathBuf::from("duplicate-right"),
                PathBuf::from("partial-left"),
                PathBuf::from("partial-right"),
            ]
        );
        let mut full_paths = io.full_paths.lock().unwrap().clone();
        full_paths.sort();
        assert_eq!(
            full_paths,
            vec![
                PathBuf::from("duplicate-left"),
                PathBuf::from("duplicate-right"),
            ]
        );
        assert_eq!(io.partial_opens.load(Ordering::Relaxed), 4);
        assert_eq!(io.full_opens.load(Ordering::Relaxed), 2);
        assert_eq!(outcome.files_hashed, 4);
        assert_eq!(outcome.partial_hashes_attempted, 4);
        assert_eq!(outcome.partial_screened_files, 4);
        assert_eq!(outcome.partial_screened_bytes, 12_288);
        assert_eq!(outcome.full_hash_requests, 2);
        assert_eq!(outcome.full_hash_content_reads_started, 2);
        assert_eq!(outcome.full_hash_bytes_read, 8_192);
        assert_eq!(outcome.hash_pipeline_resolved_files, 4);
        assert_eq!(outcome.hash_pipeline_resolved_bytes, 12_288);

        let mut duplicate_groups = outcome
            .confirmed_duplicates
            .iter()
            .map(|entry| {
                let mut paths = entry.value().clone();
                paths.sort();
                paths
            })
            .collect::<Vec<_>>();
        duplicate_groups.sort();
        assert_eq!(
            duplicate_groups,
            vec![vec![
                PathBuf::from("duplicate-left"),
                PathBuf::from("duplicate-right"),
            ]]
        );
    }

    #[test]
    fn larger_size_buckets_are_admitted_before_smaller_buckets() {
        let map = DashMap::new();
        map.insert(
            4_096,
            vec![PathBuf::from("medium-left"), PathBuf::from("medium-right")],
        );
        map.insert(
            8_192,
            vec![PathBuf::from("large-left"), PathBuf::from("large-right")],
        );
        map.insert(
            2_048,
            vec![PathBuf::from("small-left"), PathBuf::from("small-right")],
        );
        let io = OrderedPartialIo::default();
        build_content_hash_map_with_progress(
            map,
            &AtomicBool::new(false),
            &crate::progress::SilentReporter,
            &RecordingSink::default(),
            &io,
        )
        .unwrap();
        assert_eq!(
            *io.paths.lock().unwrap(),
            vec![
                PathBuf::from("large-left"),
                PathBuf::from("large-right"),
                PathBuf::from("medium-left"),
                PathBuf::from("medium-right"),
                PathBuf::from("small-left"),
                PathBuf::from("small-right"),
            ]
        );
    }

    #[test]
    fn hash_pipeline_serializes_each_rotational_device_but_overlaps_separate_devices() {
        let map = DashMap::new();
        map.insert(
            4_096,
            (0..8)
                .map(|index| {
                    PathBuf::from(format!(
                        "disk-{}-{index}",
                        if index % 2 == 0 { "a" } else { "b" }
                    ))
                })
                .collect(),
        );
        let sink = RecordingSink::default();
        let io = SchedulingPipelineIo::default();
        let outcome = build_content_hash_map_with_scheduler(
            map,
            &AtomicBool::new(false),
            &crate::progress::SilentReporter,
            &sink,
            &io,
            &TwoRotationalDeviceMapper,
            DeviceReadPolicy::for_test(4, 4),
        )
        .unwrap();

        assert_eq!(
            io.maximum_by_device.lock().unwrap().clone(),
            HashMap::from([("physical:a".to_owned(), 1), ("physical:b".to_owned(), 1)])
        );
        assert_eq!(io.maximum_total.load(Ordering::SeqCst), 2);
        assert_eq!(outcome.partial_hashes_attempted, 8);
        assert_eq!(outcome.full_hash_requests, 8);
        assert_eq!(outcome.full_hash_content_reads_completed, 8);
        assert_eq!(outcome.hash_pipeline_resolved_files, 8);
        let mut duplicates = outcome
            .confirmed_duplicates
            .iter()
            .flat_map(|entry| entry.value().clone())
            .collect::<Vec<_>>();
        duplicates.sort();
        assert_eq!(duplicates.len(), 8);
    }

    #[test]
    fn cache_and_read_outcomes_reconcile_without_mixing_logical_and_physical_bytes() {
        let hit = run_scenario(FullScenario::Hit);
        assert_eq!(hit.full_hash_cache_hits, 2);
        assert_eq!(hit.full_hash_content_reads_started, 0);
        assert_eq!(hit.full_hash_satisfied_files, 2);
        assert_eq!(hit.full_hash_bytes_read, 0);

        let stored = run_scenario(FullScenario::MissStored);
        assert_eq!(stored.full_hash_cache_misses, 2);
        assert_eq!(stored.full_hash_content_reads_started, 2);
        assert_eq!(stored.full_hash_content_reads_completed, 2);
        assert_eq!(stored.full_hash_cache_stores, 2);
        assert_eq!(stored.full_hash_bytes_read, 246);
        assert_eq!(stored.full_hash_satisfied_bytes, 8_192);

        let lookup_error = run_scenario(FullScenario::LookupErrorFallback);
        assert_eq!(lookup_error.full_hash_cache_errors, 2);
        assert_eq!(lookup_error.full_hash_content_reads_completed, 2);
        assert_eq!(lookup_error.warning_count, 2);
        assert_eq!(lookup_error.full_hash_failures, 0);

        let read_failure = run_scenario(FullScenario::ReadFailure);
        assert_eq!(read_failure.full_hash_cache_misses, 2);
        assert_eq!(read_failure.full_hash_content_reads_failed, 2);
        assert_eq!(read_failure.full_hash_failures, 2);
        assert_eq!(read_failure.full_hash_failed_bytes, 8_192);
        assert_eq!(read_failure.full_hash_bytes_read, 246);
        assert_eq!(read_failure.hash_pipeline_resolved_files, 2);
        assert_eq!(read_failure.full_hash_satisfied_files, 0);

        let store_error = run_scenario(FullScenario::StoreError);
        assert_eq!(store_error.full_hash_content_reads_completed, 2);
        assert_eq!(store_error.full_hash_cache_stores, 0);
        assert_eq!(store_error.warning_count, 2);
        assert_eq!(store_error.full_hash_satisfied_files, 2);
    }

    #[test]
    fn cancellation_retains_observed_bytes_without_false_failure_or_warning() {
        let map = DashMap::new();
        map.insert(4_096, vec![PathBuf::from("left"), PathBuf::from("right")]);
        let sink = RecordingSink::default();
        let error = match build_content_hash_map_with_progress(
            map,
            &AtomicBool::new(false),
            &crate::progress::SilentReporter,
            &sink,
            &CancelFullIo,
        ) {
            Ok(_) => panic!("injected cancellation unexpectedly completed"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        let snapshot = sink.snapshot();
        assert!(snapshot.full_hash_bytes_read >= 123);
        assert_eq!(snapshot.cancelled_work_items, 1);
        assert!(snapshot.cancel_checks >= 1);
        assert_eq!(snapshot.full_hash_content_reads_failed, 0);
        assert_eq!(snapshot.full_hash_failures, 0);
        assert_eq!(snapshot.warning_count, 0);
    }

    #[test]
    fn large_bucket_publishes_partial_progress_before_bucket_resolution() {
        let map = DashMap::new();
        map.insert(
            4_096,
            (0..300)
                .map(|index| PathBuf::from(format!("unique-{index}")))
                .collect(),
        );
        let sink = RecordingSink::default();
        let outcome = build_content_hash_map_with_progress(
            map,
            &AtomicBool::new(false),
            &crate::progress::SilentReporter,
            &sink,
            &UniquePartialIo,
        )
        .unwrap();

        let history = sink.history.lock().unwrap();
        assert!(history.iter().any(|snapshot| {
            snapshot.partial_screened_files >= HASH_PROGRESS_FILE_QUANTUM
                && snapshot.hash_pipeline_resolved_files == 0
        }));
        assert!(
            history.len() <= 2,
            "unexpected publication burst: {history:?}"
        );
        assert_eq!(outcome.partial_screened_files, 300);
        assert_eq!(outcome.hash_pipeline_resolved_files, 300);
        assert_eq!(outcome.confirmed_duplicates.len(), 0);
    }

    #[test]
    fn long_full_read_publishes_physical_bytes_before_completion() {
        let map = DashMap::new();
        map.insert(
            FULL_READ_PROGRESS_BYTE_QUANTUM,
            vec![PathBuf::from("left"), PathBuf::from("right")],
        );
        let sink = RecordingSink::default();
        let outcome = build_content_hash_map_with_progress(
            map,
            &AtomicBool::new(false),
            &crate::progress::SilentReporter,
            &sink,
            &LongFullReadIo,
        )
        .unwrap();

        assert!(sink.history.lock().unwrap().iter().any(|snapshot| {
            snapshot.full_hash_bytes_read >= FULL_READ_PROGRESS_BYTE_QUANTUM
                && snapshot.full_hash_content_reads_completed
                    < snapshot.full_hash_content_reads_started
        }));
        assert_eq!(outcome.full_hash_requests, 2);
        assert_eq!(outcome.full_hash_content_reads_completed, 2);
        assert_eq!(outcome.full_hash_satisfied_files, 2);
        assert_eq!(outcome.hash_pipeline_resolved_files, 2);
        assert_eq!(outcome.confirmed_duplicates.len(), 1);
    }

    #[test]
    fn streaming_hash_matches_in_memory_hash_across_buffer_boundaries() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("large.bin");
        let data: Vec<u8> = (0..(ROTATIONAL_STREAM_BUFFER_LENGTH * 3 + 17))
            .map(|index| (index % 251) as u8)
            .collect();
        fs::write(&path, &data).unwrap();
        assert_eq!(
            hash_file_streaming(&path, &AtomicBool::new(false)).unwrap(),
            hash_data(&data)
        );
    }

    #[test]
    fn measured_media_buffers_and_sequential_hint_preserve_exact_hashes() {
        assert_eq!(
            stream_buffer_length(crate::platform::StorageMediaClass::SolidState),
            SOLID_STATE_STREAM_BUFFER_LENGTH
        );
        assert_eq!(
            stream_buffer_length(crate::platform::StorageMediaClass::Rotational),
            ROTATIONAL_STREAM_BUFFER_LENGTH
        );
        assert_eq!(
            stream_buffer_length(crate::platform::StorageMediaClass::Unknown),
            ROTATIONAL_STREAM_BUFFER_LENGTH
        );
        assert!(!stream_sequential_hint(
            crate::platform::StorageMediaClass::SolidState
        ));
        assert!(stream_sequential_hint(
            crate::platform::StorageMediaClass::Rotational
        ));
        assert!(stream_sequential_hint(
            crate::platform::StorageMediaClass::Unknown
        ));
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("media-buffer.bin");
        let data = (0..(SOLID_STATE_STREAM_BUFFER_LENGTH + 17))
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        fs::write(&path, &data).unwrap();
        for buffer_length in [
            ROTATIONAL_STREAM_BUFFER_LENGTH,
            SOLID_STATE_STREAM_BUFFER_LENGTH,
        ] {
            for sequential_hint in [false, true] {
                let hash = hash_file_streaming_observed_with_options(
                    &path,
                    &AtomicBool::new(false),
                    buffer_length,
                    sequential_hint,
                    &mut |_| Ok(()),
                )
                .unwrap();
                assert_eq!(hash, hash_data(&data));
            }
        }
        for invalid in [0, SOLID_STATE_STREAM_BUFFER_LENGTH + 1] {
            assert_eq!(
                hash_file_streaming_observed_with_options(
                    &path,
                    &AtomicBool::new(false),
                    invalid,
                    true,
                    &mut |_| Ok(()),
                )
                .unwrap_err()
                .kind(),
                io::ErrorKind::InvalidInput
            );
        }
    }

    #[test]
    fn observed_stream_reports_exact_remainder_bytes() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("observed.bin");
        let data = vec![7_u8; ROTATIONAL_STREAM_BUFFER_LENGTH * 3 + 17];
        fs::write(&path, &data).unwrap();
        let mut events = Vec::new();
        let hash = hash_file_streaming_observed(&path, &AtomicBool::new(false), &mut |event| {
            events.push(event);
            Ok(())
        })
        .unwrap();
        assert_eq!(hash, hash_data(&data));
        assert_eq!(events.first(), Some(&FullHashIoEvent::ContentReadStarted));
        assert_eq!(
            events
                .iter()
                .filter_map(|event| match event {
                    FullHashIoEvent::ContentBytesRead(bytes) => Some(*bytes),
                    _ => None,
                })
                .sum::<u64>(),
            data.len() as u64
        );
    }

    #[test]
    fn observed_stream_retains_bytes_before_cancellation() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("cancel-observed.bin");
        fs::write(&path, vec![9_u8; ROTATIONAL_STREAM_BUFFER_LENGTH * 2]).unwrap();
        let cancel = AtomicBool::new(false);
        let mut checks = 0;
        let mut observed_bytes = 0;
        let error = hash_file_streaming_observed(&path, &cancel, &mut |event| {
            match event {
                FullHashIoEvent::CancellationCheck { cancelled: false } => {
                    checks += 1;
                    if checks == 1 {
                        cancel.store(true, Ordering::Relaxed);
                    }
                }
                FullHashIoEvent::ContentBytesRead(bytes) => observed_bytes += bytes,
                _ => {}
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(observed_bytes, ROTATIONAL_STREAM_BUFFER_LENGTH as u64);
    }

    #[test]
    fn streaming_hash_honors_preexisting_cancellation() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("cancel.bin");
        fs::write(&path, b"content").unwrap();
        assert_eq!(
            hash_file_streaming(&path, &AtomicBool::new(true))
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted
        );
    }
}
