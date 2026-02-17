use super::cache;
use dashmap::DashMap;
use rayon::prelude::*;
use std::fs::File;
use std::hash::Hasher as _;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use twox_hash::XxHash64;

const PARTIAL_HASH_LENGTH: usize = 1024; // 1KB

/// Two-tier hashing strategy:
/// 1. Partial hash (first 1KB via XxHash64) to quickly eliminate non-matches
/// 2. Full content hash only on partial-hash collisions
///
/// Takes a map keyed on file size (each value is a Vec of paths with that size)
/// and returns a map of content_hash → Vec<PathBuf> for confirmed duplicates only.
pub fn build_content_hash_map(
    size_to_file_map: DashMap<u64, Vec<PathBuf>>,
) -> io::Result<DashMap<u64, Vec<PathBuf>>> {
    let confirmed_duplicates: DashMap<u64, Vec<PathBuf>> = DashMap::new();

    let size_to_file_vec: Vec<_> = size_to_file_map.iter().collect();

    size_to_file_vec.par_iter().try_for_each(|files| {
        let partial_hash_to_file_map: DashMap<u64, Vec<PathBuf>> = DashMap::new();
        let full_hash_to_file_map: DashMap<u64, Vec<PathBuf>> = DashMap::new();

        // First pass: partial hash to eliminate non-dupes quickly
        files
            .value()
            .par_iter()
            .try_for_each(|file| populate_partial_hash_map(file, &partial_hash_to_file_map))?;

        // Second pass: full hash only on partial-hash collisions (>1 file)
        let partial_hash_to_file_vec: Vec<_> = partial_hash_to_file_map.iter().collect();
        partial_hash_to_file_vec.par_iter().try_for_each(|files| {
            if files.value().len() > 1 {
                files
                    .value()
                    .par_iter()
                    .try_for_each(|file| {
                        populate_full_hash_map(file, &full_hash_to_file_map)
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

        Ok::<_, io::Error>(())
    })?;

    Ok(confirmed_duplicates)
}

fn populate_partial_hash_map(
    file: &Path,
    partial_hash_to_file_map: &DashMap<u64, Vec<PathBuf>>,
) -> io::Result<()> {
    match read_portion(file) {
        Ok(data) => {
            let hash = hash_data(&data);
            partial_hash_to_file_map
                .entry(hash)
                .or_default()
                .push(file.to_path_buf());
            Ok(())
        }
        Err(e) => {
            tracing::error!("Error processing file '{}': {}", file.display(), e);
            Ok(())
        }
    }
}

fn populate_full_hash_map(
    file: &Path,
    full_hash_to_file_map: &DashMap<u64, Vec<PathBuf>>,
) -> io::Result<()> {
    match cache::get_content_hash(file) {
        Ok(hash) => {
            full_hash_to_file_map
                .entry(hash)
                .or_default()
                .push(file.to_path_buf());
            Ok(())
        }
        Err(e) => {
            tracing::error!("Error processing file '{}': {}", file.display(), e);
            Ok(())
        }
    }
}

fn read_portion(file: &Path) -> io::Result<Vec<u8>> {
    let mut f = File::open(file)?;
    let mut buffer = vec![0; PARTIAL_HASH_LENGTH];
    let bytes_read = f.read(&mut buffer)?;
    buffer.truncate(bytes_read);
    Ok(buffer)
}

pub fn read_full_file(file: &Path) -> io::Result<Vec<u8>> {
    let mut f = File::open(file)?;
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)?;
    Ok(buffer)
}

pub fn hash_data(data: &[u8]) -> u64 {
    let mut hasher = XxHash64::with_seed(0);
    hasher.write(data);
    hasher.finish()
}
