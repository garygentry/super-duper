use super::cache;
use crate::progress::ProgressReporter;
use dashmap::DashMap;
use rayon::prelude::*;
use std::fs::File;
use std::hash::Hasher as _;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use twox_hash::XxHash64;

const PARTIAL_HASH_LENGTH: usize = 1024;
const STREAM_BUFFER_LENGTH: usize = 64 * 1024;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PartialHashRead {
    pub hash: u64,
    pub physical_bytes_read: u64,
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
        cancel: &AtomicBool,
        observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
    ) -> io::Result<cache::CachedHash>;
}

#[derive(Debug, Default)]
pub(crate) struct SystemHashPipelineIo;

impl HashPipelineIo for SystemHashPipelineIo {
    fn partial_hash(&self, path: &Path, cancel: &AtomicBool) -> io::Result<PartialHashRead> {
        let data = read_portion(path, cancel)?;
        Ok(PartialHashRead {
            hash: hash_data(&data),
            physical_bytes_read: data.len() as u64,
        })
    }

    fn full_hash(
        &self,
        path: &Path,
        cancel: &AtomicBool,
        observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
    ) -> io::Result<cache::CachedHash> {
        cache::get_content_hash_cancellable_observed(path, cancel, observe)
    }
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
    build_content_hash_map_with_progress(map, cancel, progress, &sink, &SystemHashPipelineIo)
}

pub(crate) fn build_content_hash_map_with_progress(
    map: DashMap<u64, Vec<PathBuf>>,
    cancel: &AtomicBool,
    progress: &dyn ProgressReporter,
    sink: &dyn HashProgressSink,
    io: &dyn HashPipelineIo,
) -> io::Result<HashOutcome> {
    let confirmed_duplicates: DashMap<u64, Vec<PathBuf>> = DashMap::new();
    let total_files: usize = map.iter().map(|entry| entry.value().len()).sum();
    let files_processed = AtomicUsize::new(0);
    let batcher = HashProgressBatcher::new(sink);
    let buckets: Vec<_> = map.iter().collect();

    let result = buckets.par_iter().try_for_each(|bucket| {
        batcher.check_cancelled(cancel)?;
        let file_size = *bucket.key();
        let partial_groups: DashMap<u64, Vec<PathBuf>> = DashMap::new();
        let full_groups: DashMap<u64, Vec<PathBuf>> = DashMap::new();

        bucket.value().par_iter().try_for_each(|file| {
            batcher.check_cancelled(cancel)?;
            let mut delta = HashProgressDelta {
                partial_hashes_attempted: 1,
                partial_screened_files: 1,
                partial_screened_bytes: file_size,
                ..Default::default()
            };
            match io.partial_hash(file, cancel) {
                Ok(read) => {
                    partial_groups
                        .entry(read.hash)
                        .or_default()
                        .push(file.to_path_buf());
                    delta.files_hashed = 1;
                    delta.partial_hashes_succeeded = 1;
                    delta.partial_hash_bytes_read = read.physical_bytes_read;
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                    batcher.cancellation_check(true)?;
                    return Err(error);
                }
                Err(error) => {
                    tracing::error!("Error processing file '{}': {}", file.display(), error);
                    delta.partial_hashes_failed = 1;
                    delta.warning_count = 1;
                }
            }
            batcher.record(delta, 1, false)
        })?;

        let groups: Vec<_> = partial_groups.iter().collect();
        let collision_bucket_count = groups.iter().filter(|g| g.value().len() > 1).count() as u64;
        let collision_files = groups
            .iter()
            .filter(|g| g.value().len() > 1)
            .map(|g| g.value().len() as u64)
            .sum::<u64>();
        let collision_bytes = collision_files.saturating_mul(file_size);
        let screened_files = bucket.value().len() as u64;
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

        groups.par_iter().try_for_each(|group| {
            batcher.check_cancelled(cancel)?;
            if group.value().len() <= 1 {
                return Ok::<_, io::Error>(());
            }
            group.value().par_iter().try_for_each(|file| {
                batcher.check_cancelled(cancel)?;
                populate_full_hash_map(file, file_size, &full_groups, cancel, &batcher, io)
            })
        })?;

        let complete_groups: Vec<_> = full_groups.iter().collect();
        complete_groups.par_iter().for_each(|entry| {
            if entry.value().len() > 1 {
                confirmed_duplicates
                    .entry(*entry.key())
                    .or_default()
                    .extend_from_slice(entry.value());
            }
        });

        let processed = files_processed.fetch_add(bucket.value().len(), Ordering::Relaxed)
            + bucket.value().len();
        if processed % (HASH_PROGRESS_FILE_QUANTUM as usize) < bucket.value().len() {
            emit_legacy_progress(progress, &sink.snapshot(), total_files);
        }
        Ok::<_, io::Error>(())
    });

    batcher.flush()?;
    result?;
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

fn populate_full_hash_map(
    file: &Path,
    file_size: u64,
    full_groups: &DashMap<u64, Vec<PathBuf>>,
    cancel: &AtomicBool,
    batcher: &HashProgressBatcher<'_>,
    io: &dyn HashPipelineIo,
) -> io::Result<()> {
    let mut lookup = None;
    let mut content_started = false;
    let result = io.full_hash(file, cancel, &mut |event| match event {
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
        FullHashIoEvent::CancellationCheck { cancelled } => batcher.cancellation_check(cancelled),
    });

    match result {
        Ok(outcome) => {
            let observed = lookup.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "full hash I/O omitted cache lookup outcome",
                )
            })?;
            if observed != outcome.cache_outcome {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "full hash I/O returned conflicting cache lookup outcome",
                ));
            }
            if outcome.cache_outcome != cache::CacheLookupOutcome::Hit && !content_started {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "cache fallback completed without content-read start",
                ));
            }
            full_groups
                .entry(outcome.hash)
                .or_default()
                .push(file.to_path_buf());
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
            batcher.record(delta, 1, false)
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
            batcher.record(delta, 1, false)
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
    let mut file = File::open(path)?;
    observe(FullHashIoEvent::ContentReadStarted)?;
    let mut buffer = vec![0_u8; STREAM_BUFFER_LENGTH];
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
    use std::fs;
    use tempfile::TempDir;

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
            Ok(PartialHashRead {
                hash: hash_data(path.to_string_lossy().as_bytes()),
                physical_bytes_read: PARTIAL_HASH_LENGTH as u64,
            })
        }

        fn full_hash(
            &self,
            _path: &Path,
            _cancel: &AtomicBool,
            _observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
        ) -> io::Result<cache::CachedHash> {
            panic!("unique partial hashes must not request a full hash")
        }
    }

    struct LongFullReadIo;

    impl HashPipelineIo for LongFullReadIo {
        fn partial_hash(&self, _path: &Path, _cancel: &AtomicBool) -> io::Result<PartialHashRead> {
            Ok(PartialHashRead {
                hash: 7,
                physical_bytes_read: PARTIAL_HASH_LENGTH as u64,
            })
        }

        fn full_hash(
            &self,
            _path: &Path,
            _cancel: &AtomicBool,
            observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
        ) -> io::Result<cache::CachedHash> {
            observe(FullHashIoEvent::CacheLookup(
                cache::CacheLookupOutcome::Miss,
            ))?;
            observe(FullHashIoEvent::ContentReadStarted)?;
            observe(FullHashIoEvent::ContentBytesRead(
                FULL_READ_PROGRESS_BYTE_QUANTUM,
            ))?;
            Ok(cache::CachedHash {
                hash: 11,
                warning: None,
                cache_outcome: cache::CacheLookupOutcome::Miss,
                content_bytes_read: FULL_READ_PROGRESS_BYTE_QUANTUM,
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
            Ok(PartialHashRead {
                hash: 5,
                physical_bytes_read: PARTIAL_HASH_LENGTH as u64,
            })
        }

        fn full_hash(
            &self,
            _path: &Path,
            _cancel: &AtomicBool,
            observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
        ) -> io::Result<cache::CachedHash> {
            let lookup = match self.0 {
                FullScenario::Hit => cache::CacheLookupOutcome::Hit,
                FullScenario::LookupErrorFallback => cache::CacheLookupOutcome::Error,
                _ => cache::CacheLookupOutcome::Miss,
            };
            observe(FullHashIoEvent::CacheLookup(lookup))?;
            if matches!(self.0, FullScenario::Hit) {
                return Ok(cache::CachedHash {
                    hash: 13,
                    warning: None,
                    cache_outcome: lookup,
                    content_bytes_read: 0,
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
            Ok(cache::CachedHash {
                hash: 13,
                warning: match self.0 {
                    FullScenario::LookupErrorFallback => Some("injected lookup error".to_owned()),
                    FullScenario::StoreError => Some("injected store error".to_owned()),
                    _ => None,
                },
                cache_outcome: lookup,
                content_bytes_read: 123,
                cache_stored: matches!(self.0, FullScenario::MissStored),
            })
        }
    }

    struct CancelFullIo;

    impl HashPipelineIo for CancelFullIo {
        fn partial_hash(&self, _path: &Path, _cancel: &AtomicBool) -> io::Result<PartialHashRead> {
            Ok(PartialHashRead {
                hash: 17,
                physical_bytes_read: PARTIAL_HASH_LENGTH as u64,
            })
        }

        fn full_hash(
            &self,
            _path: &Path,
            _cancel: &AtomicBool,
            observe: &mut dyn FnMut(FullHashIoEvent) -> io::Result<()>,
        ) -> io::Result<cache::CachedHash> {
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
        let data: Vec<u8> = (0..(STREAM_BUFFER_LENGTH * 3 + 17))
            .map(|index| (index % 251) as u8)
            .collect();
        fs::write(&path, &data).unwrap();
        assert_eq!(
            hash_file_streaming(&path, &AtomicBool::new(false)).unwrap(),
            hash_data(&data)
        );
    }

    #[test]
    fn observed_stream_reports_exact_remainder_bytes() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("observed.bin");
        let data = vec![7_u8; STREAM_BUFFER_LENGTH * 3 + 17];
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
        fs::write(&path, vec![9_u8; STREAM_BUFFER_LENGTH * 2]).unwrap();
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
        assert_eq!(observed_bytes, STREAM_BUFFER_LENGTH as u64);
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
