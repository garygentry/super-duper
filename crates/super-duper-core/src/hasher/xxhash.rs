use super::cache;
use crate::progress::ProgressReporter;
use dashmap::DashMap;
use rayon::prelude::*;
use std::fs::File;
use std::hash::Hasher as _;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use twox_hash::XxHash64;

const PARTIAL_HASH_LENGTH: usize = 1024; // 1KB
const STREAM_BUFFER_LENGTH: usize = 64 * 1024;

pub struct HashOutcome {
    pub confirmed_duplicates: DashMap<u64, Vec<PathBuf>>,
    /// Files successfully processed by the hashing phase (partial hash or cache/full hash).
    pub files_hashed: usize,
    pub warning_count: usize,
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

    let size_to_file_vec: Vec<_> = size_to_file_map.iter().collect();

    size_to_file_vec.par_iter().try_for_each(|files| {
        if cancel_token.load(Ordering::Relaxed) {
            return Ok(());
        }

        let partial_hash_to_file_map: DashMap<u64, Vec<PathBuf>> = DashMap::new();
        let full_hash_to_file_map: DashMap<u64, Vec<PathBuf>> = DashMap::new();

        // First pass: partial hash to eliminate non-dupes quickly
        files.value().par_iter().try_for_each(|file| {
            if cancel_token.load(Ordering::Relaxed) {
                return Ok::<_, io::Error>(());
            }
            if populate_partial_hash_map(file, &partial_hash_to_file_map, cancel_token)? {
                files_hashed.fetch_add(1, Ordering::Relaxed);
            } else {
                warning_count.fetch_add(1, Ordering::Relaxed);
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
                files.value().par_iter().try_for_each(|file| {
                    if cancel_token.load(Ordering::Relaxed) {
                        return Ok::<_, io::Error>(());
                    }
                    if let Some(cache_warning) =
                        populate_full_hash_map(file, &full_hash_to_file_map, cancel_token)?
                    {
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
    })
}

fn populate_partial_hash_map(
    file: &Path,
    partial_hash_to_file_map: &DashMap<u64, Vec<PathBuf>>,
    cancel_token: &AtomicBool,
) -> io::Result<bool> {
    match read_portion(file, cancel_token) {
        Ok(data) => {
            let hash = hash_data(&data);
            partial_hash_to_file_map
                .entry(hash)
                .or_default()
                .push(file.to_path_buf());
            Ok(true)
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::Interrupted {
                return Err(e);
            }
            tracing::error!("Error processing file '{}': {}", file.display(), e);
            Ok(false)
        }
    }
}

fn populate_full_hash_map(
    file: &Path,
    full_hash_to_file_map: &DashMap<u64, Vec<PathBuf>>,
    cancel_token: &AtomicBool,
) -> io::Result<Option<String>> {
    match cache::get_content_hash_cancellable(file, cancel_token) {
        Ok(outcome) => {
            full_hash_to_file_map
                .entry(outcome.hash)
                .or_default()
                .push(file.to_path_buf());
            Ok(outcome.warning)
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::Interrupted {
                return Err(e);
            }
            tracing::error!("Error processing file '{}': {}", file.display(), e);
            Ok(Some(e.to_string()))
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
