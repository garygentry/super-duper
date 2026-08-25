use super::cache;
use crate::progress::ProgressReporter;
use dashmap::DashMap;
use rayon::prelude::*;
use std::fs::File;
use std::hash::Hasher as _;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use twox_hash::XxHash64;

const PARTIAL_HASH_LENGTH: usize = 1024; // 1KB
const STREAM_BUFFER_LENGTH: usize = 64 * 1024;

pub struct HashOutcome {
    pub confirmed_duplicates: DashMap<u64, Vec<PathBuf>>,
    /// Files successfully processed by the hashing phase (partial hash or cache/full hash).
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
    pub full_hash_cache_hits: u64,
    pub full_hash_cache_misses: u64,
    pub full_hash_cache_errors: u64,
    pub full_hash_cache_stores: u64,
    pub full_hash_content_reads_started: u64,
    pub full_hash_content_reads_completed: u64,
    pub full_hash_bytes_read: u64,
    pub unavailable_counters: u64,
}

struct FullHashAttempt {
    warning: Option<String>,
    cache_outcome: Option<cache::CacheLookupOutcome>,
    content_bytes_read: u64,
    cache_stored: bool,
}

/// Two-tier hashing strategy:
/// 1. Partial hash (first 1KB via XxHash64) to quickly eliminate non-matches
/// 2. Full content hash only on partial-hash collisions
///
/// Takes a map keyed on file size (each value is a Vec of paths with that size)
/// and returns a map of content_hash → Vec<PathBuf> for confirmed duplicates only.
pub fn build_content_hash_map(
    size_to_file_map: DashMap<u64, Vec<PathBuf>>,
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> io::Result<DashMap<u64, Vec<PathBuf>>> {
    Ok(
        build_content_hash_map_with_stats(size_to_file_map, cancel_token, progress)?
            .confirmed_duplicates,
    )
}

pub fn build_content_hash_map_with_stats(
    size_to_file_map: DashMap<u64, Vec<PathBuf>>,
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> io::Result<HashOutcome> {
    let confirmed_duplicates: DashMap<u64, Vec<PathBuf>> = DashMap::new();

    // Count total files for progress reporting
    let total_files: usize = size_to_file_map.iter().map(|e| e.value().len()).sum();
    let files_processed = AtomicUsize::new(0);
    let files_hashed = AtomicUsize::new(0);
    let warning_count = AtomicUsize::new(0);
    let partial_hashes_attempted = AtomicU64::new(0);
    let partial_hashes_succeeded = AtomicU64::new(0);
    let partial_hashes_failed = AtomicU64::new(0);
    let partial_hash_bytes_read = AtomicU64::new(0);
    let partial_collision_buckets = AtomicU64::new(0);
    let partial_collision_files = AtomicU64::new(0);
    let partial_collision_bytes = AtomicU64::new(0);
    let full_hash_requests = AtomicU64::new(0);
    let full_hash_cache_hits = AtomicU64::new(0);
    let full_hash_cache_misses = AtomicU64::new(0);
    let full_hash_cache_errors = AtomicU64::new(0);
    let full_hash_cache_stores = AtomicU64::new(0);
    let full_hash_content_reads_started = AtomicU64::new(0);
    let full_hash_content_reads_completed = AtomicU64::new(0);
    let full_hash_bytes_read = AtomicU64::new(0);
    let unavailable_counters = AtomicU64::new(0);

    let size_to_file_vec: Vec<_> = size_to_file_map.iter().collect();

    size_to_file_vec.par_iter().try_for_each(|files| {
        if cancel_token.load(Ordering::Relaxed) {
            return Ok(());
        }

        let file_size = *files.key();
        let partial_hash_to_file_map: DashMap<u64, Vec<PathBuf>> = DashMap::new();
        let full_hash_to_file_map: DashMap<u64, Vec<PathBuf>> = DashMap::new();

        // First pass: partial hash to eliminate non-dupes quickly
        files.value().par_iter().try_for_each(|file| {
            if cancel_token.load(Ordering::Relaxed) {
                return Ok::<_, io::Error>(());
            }
            partial_hashes_attempted.fetch_add(1, Ordering::Relaxed);
            match populate_partial_hash_map(file, &partial_hash_to_file_map, cancel_token)? {
                Some(bytes_read) => {
                    files_hashed.fetch_add(1, Ordering::Relaxed);
                    partial_hashes_succeeded.fetch_add(1, Ordering::Relaxed);
                    partial_hash_bytes_read.fetch_add(bytes_read, Ordering::Relaxed);
                }
                None => {
                    partial_hashes_failed.fetch_add(1, Ordering::Relaxed);
                    warning_count.fetch_add(1, Ordering::Relaxed);
                }
            }
            Ok::<_, io::Error>(())
        })?;

        // Second pass: full hash only on partial-hash collisions (>1 file)
        let partial_hash_to_file_vec: Vec<_> = partial_hash_to_file_map.iter().collect();
        partial_hash_to_file_vec.par_iter().try_for_each(|files| {
            if cancel_token.load(Ordering::Relaxed) {
                return Ok::<_, io::Error>(());
            }
            if files.value().len() > 1 {
                let collision_files = files.value().len() as u64;
                partial_collision_buckets.fetch_add(1, Ordering::Relaxed);
                partial_collision_files.fetch_add(collision_files, Ordering::Relaxed);
                partial_collision_bytes
                    .fetch_add(collision_files.saturating_mul(file_size), Ordering::Relaxed);
                files.value().par_iter().try_for_each(|file| {
                    if cancel_token.load(Ordering::Relaxed) {
                        return Ok::<_, io::Error>(());
                    }
                    full_hash_requests.fetch_add(1, Ordering::Relaxed);
                    let attempt =
                        populate_full_hash_map(file, &full_hash_to_file_map, cancel_token)?;
                    match attempt.cache_outcome {
                        Some(cache::CacheLookupOutcome::Hit) => {
                            full_hash_cache_hits.fetch_add(1, Ordering::Relaxed);
                        }
                        Some(cache::CacheLookupOutcome::Miss) => {
                            full_hash_cache_misses.fetch_add(1, Ordering::Relaxed);
                        }
                        Some(cache::CacheLookupOutcome::Error) => {
                            full_hash_cache_errors.fetch_add(1, Ordering::Relaxed);
                        }
                        None => {
                            unavailable_counters.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    if attempt.content_bytes_read > 0 {
                        full_hash_content_reads_started.fetch_add(1, Ordering::Relaxed);
                        full_hash_content_reads_completed.fetch_add(1, Ordering::Relaxed);
                        full_hash_bytes_read
                            .fetch_add(attempt.content_bytes_read, Ordering::Relaxed);
                    }
                    if attempt.cache_stored {
                        full_hash_cache_stores.fetch_add(1, Ordering::Relaxed);
                    }
                    if let Some(cache_warning) = attempt.warning {
                        warning_count.fetch_add(1, Ordering::Relaxed);
                        tracing::warn!("{}: {}", file.display(), cache_warning);
                    }
                    Ok::<_, io::Error>(())
                })?;
            }
            Ok::<_, io::Error>(())
        })?;

        // Collect confirmed duplicates (full hash groups with >1 file)
        let full_hash_to_file_vec: Vec<_> = full_hash_to_file_map.iter().collect();
        full_hash_to_file_vec.par_iter().for_each(|entry| {
            if entry.value().len() > 1 {
                confirmed_duplicates
                    .entry(*entry.key())
                    .or_default()
                    .extend_from_slice(entry.value());
            }
        });

        // Update progress
        let processed =
            files_processed.fetch_add(files.value().len(), Ordering::Relaxed) + files.value().len();
        if processed % 256 < files.value().len() {
            progress.on_hash_progress_detailed(
                files_hashed.load(Ordering::Relaxed),
                total_files,
                warning_count.load(Ordering::Relaxed),
                None,
            );
        }

        Ok::<_, io::Error>(())
    })?;

    progress.on_hash_progress_detailed(
        files_hashed.load(Ordering::Relaxed),
        total_files,
        warning_count.load(Ordering::Relaxed),
        None,
    );
    Ok(HashOutcome {
        confirmed_duplicates,
        files_hashed: files_hashed.load(Ordering::Relaxed),
        warning_count: warning_count.load(Ordering::Relaxed),
        partial_hashes_attempted: partial_hashes_attempted.load(Ordering::Relaxed),
        partial_hashes_succeeded: partial_hashes_succeeded.load(Ordering::Relaxed),
        partial_hashes_failed: partial_hashes_failed.load(Ordering::Relaxed),
        partial_hash_bytes_read: partial_hash_bytes_read.load(Ordering::Relaxed),
        partial_collision_buckets: partial_collision_buckets.load(Ordering::Relaxed),
        partial_collision_files: partial_collision_files.load(Ordering::Relaxed),
        partial_collision_bytes: partial_collision_bytes.load(Ordering::Relaxed),
        full_hash_requests: full_hash_requests.load(Ordering::Relaxed),
        full_hash_cache_hits: full_hash_cache_hits.load(Ordering::Relaxed),
        full_hash_cache_misses: full_hash_cache_misses.load(Ordering::Relaxed),
        full_hash_cache_errors: full_hash_cache_errors.load(Ordering::Relaxed),
        full_hash_cache_stores: full_hash_cache_stores.load(Ordering::Relaxed),
        full_hash_content_reads_started: full_hash_content_reads_started.load(Ordering::Relaxed),
        full_hash_content_reads_completed: full_hash_content_reads_completed
            .load(Ordering::Relaxed),
        full_hash_bytes_read: full_hash_bytes_read.load(Ordering::Relaxed),
        unavailable_counters: unavailable_counters.load(Ordering::Relaxed),
    })
}

fn populate_partial_hash_map(
    file: &Path,
    partial_hash_to_file_map: &DashMap<u64, Vec<PathBuf>>,
    cancel_token: &AtomicBool,
) -> io::Result<Option<u64>> {
    match read_portion(file, cancel_token) {
        Ok(data) => {
            let hash = hash_data(&data);
            partial_hash_to_file_map
                .entry(hash)
                .or_default()
                .push(file.to_path_buf());
            Ok(Some(data.len() as u64))
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::Interrupted {
                return Err(e);
            }
            tracing::error!("Error processing file '{}': {}", file.display(), e);
            Ok(None)
        }
    }
}

fn populate_full_hash_map(
    file: &Path,
    full_hash_to_file_map: &DashMap<u64, Vec<PathBuf>>,
    cancel_token: &AtomicBool,
) -> io::Result<FullHashAttempt> {
    match cache::get_content_hash_cancellable(file, cancel_token) {
        Ok(outcome) => {
            full_hash_to_file_map
                .entry(outcome.hash)
                .or_default()
                .push(file.to_path_buf());
            Ok(FullHashAttempt {
                warning: outcome.warning,
                cache_outcome: Some(outcome.cache_outcome),
                content_bytes_read: outcome.content_bytes_read,
                cache_stored: outcome.cache_stored,
            })
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::Interrupted {
                return Err(e);
            }
            tracing::error!("Error processing file '{}': {}", file.display(), e);
            Ok(FullHashAttempt {
                warning: Some(e.to_string()),
                cache_outcome: None,
                content_bytes_read: 0,
                cache_stored: false,
            })
        }
    }
}

fn read_portion(file: &Path, cancel_token: &AtomicBool) -> io::Result<Vec<u8>> {
    check_cancelled(cancel_token)?;
    let mut f = File::open(file)?;
    let mut buffer = vec![0; PARTIAL_HASH_LENGTH];
    let bytes_read = f.read(&mut buffer)?;
    buffer.truncate(bytes_read);
    Ok(buffer)
}

pub fn hash_file_streaming(file: &Path, cancel_token: &AtomicBool) -> io::Result<u64> {
    let mut f = File::open(file)?;
    let mut buffer = vec![0u8; STREAM_BUFFER_LENGTH];
    let mut hasher = XxHash64::with_seed(0);
    loop {
        check_cancelled(cancel_token)?;
        let bytes_read = f.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.write(&buffer[..bytes_read]);
    }
    Ok(hasher.finish())
}

pub fn hash_data(data: &[u8]) -> u64 {
    let mut hasher = XxHash64::with_seed(0);
    hasher.write(data);
    hasher.finish()
}

fn check_cancelled(cancel_token: &AtomicBool) -> io::Result<()> {
    if cancel_token.load(Ordering::Relaxed) {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "hashing cancelled",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn streaming_hash_matches_in_memory_hash_across_buffer_boundaries() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("large.bin");
        let data: Vec<u8> = (0..(STREAM_BUFFER_LENGTH * 3 + 17))
            .map(|index| (index % 251) as u8)
            .collect();
        fs::write(&path, &data).unwrap();

        let hash = hash_file_streaming(&path, &AtomicBool::new(false)).unwrap();

        assert_eq!(hash, hash_data(&data));
    }

    #[test]
    fn streaming_hash_honors_preexisting_cancellation() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("cancel.bin");
        fs::write(&path, b"content").unwrap();

        let error = hash_file_streaming(&path, &AtomicBool::new(true)).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    }
}
