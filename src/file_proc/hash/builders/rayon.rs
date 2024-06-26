#![allow(dead_code)]

use bincode;
use dashmap::DashMap;
use rayon::prelude::*;
use std::fs::{self, File};
use std::hash::Hasher as _;
use std::io::Read;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::trace;
use twox_hash::XxHash64;

const HASH_LENGTH: usize = 1024; // 1KB

/// Takes a map keyed on file size, with each entry containing vector os paths
/// that are of the that size, and returns a map of unique content hashes with
/// value containing vector of all
pub fn build_content_hash_map(
    size_to_file_map: DashMap<u64, Vec<PathBuf>>,
) -> io::Result<DashMap<u64, Vec<PathBuf>>> {
    let confirmed_duplicates: DashMap<u64, Vec<PathBuf>> = DashMap::new();

    let size_to_file_vec: Vec<_> = size_to_file_map.iter().collect();
    // Iterate over map keyed on file size, with value of all files that match that file_size
    size_to_file_vec.par_iter().try_for_each(|files| {
        // map of files keyed on hash of first few bytes of the file (maybe/likely dupe)
        let partial_hash_to_file_map: DashMap<u64, Vec<PathBuf>> = DashMap::new();
        // map of files with keyed on hash of full contents (definite dupe)
        let full_hash_to_file_map: DashMap<u64, Vec<PathBuf>> = DashMap::new();

        // First iterate all files of same size to elliminate non-dupes as quickly as possible
        files
            .value()
            .par_iter()
            .try_for_each(|file| populate_partial_hash_map(file, &partial_hash_to_file_map))?;

        // Now iterate possible dupes matching first few bytes to fully hash the files to be sure
        let partial_hash_to_file_vec: Vec<_> = partial_hash_to_file_map.iter().collect();
        partial_hash_to_file_vec.par_iter().try_for_each(|files| {
            // if only one entry, there is no dupe..
            if files.value().len() > 1 {
                files
                    .value()
                    .par_iter()
                    .try_for_each(|file| populate_full_hash_map(file, &full_hash_to_file_map))?;
            }
            Ok::<_, std::io::Error>(())
        })?;

        // itereate full content hash map to add confirmed dupes to return map
        let full_hash_to_file_vec: Vec<_> = full_hash_to_file_map.iter().collect();
        full_hash_to_file_vec.par_iter().for_each(|entry| {
            if entry.value().len() > 1 {
                confirmed_duplicates
                    .entry(*entry.key())
                    .or_default()
                    .extend_from_slice(entry.value());
            }
        });
        Ok::<_, std::io::Error>(())
    })?;

    Ok(confirmed_duplicates)
}

fn populate_full_hash_map(
    file: &PathBuf,
    full_hash_to_file_map: &DashMap<u64, Vec<PathBuf>>,
) -> io::Result<()> {
    match get_content_hash(file) {
        Ok(hash) => {
            full_hash_to_file_map
                .entry(hash)
                .or_default()
                .push(file.clone());
            Ok::<_, std::io::Error>(()) // Add type annotation for Ok(())
        }
        Err(e) => {
            log_exception(file, &e);
            Ok::<_, std::io::Error>(()) // Add type annotation for Ok(())
        }
    }
}

fn populate_partial_hash_map(
    file: &PathBuf,
    partial_hash_to_file_map: &DashMap<u64, Vec<PathBuf>>,
) -> io::Result<()> {
    match read_portion(file) {
        Ok(data) => {
            let hash = hash_data(&data)?;
            partial_hash_to_file_map
                .entry(hash)
                .or_default()
                .push(file.clone());
            Ok(())
        }
        Err(e) => {
            log_exception(file, &e);
            Ok::<_, std::io::Error>(()) // Add type annotation for Ok(())
        }
    }
}

fn read_portion(file: &PathBuf) -> std::io::Result<Vec<u8>> {
    let mut f = File::open(file)?;
    let mut buffer = vec![0; HASH_LENGTH];

    // Read up to HASH_LENGTH bytes
    let bytes_read = f.read(&mut buffer)?;

    // Shrink the buffer to the actual number of bytes read
    buffer.truncate(bytes_read);

    Ok(buffer)
}

fn read_full_file(file: &PathBuf) -> io::Result<Vec<u8>> {
    let mut f = File::open(file)?;
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn log_exception(file: &Path, error: &std::io::Error) {
    tracing::error!("Error processing file '{}': {}", file.display(), error);
}

fn hash_data(data: &[u8]) -> io::Result<u64> {
    let mut hasher = XxHash64::with_seed(0); // Initialize hasher with a seed
    hasher.write(data);
    Ok(hasher.finish()) // Obtain the hash as u64
}

fn get_content_hash(file: &PathBuf) -> io::Result<u64> {
    let db = super::super::hash_cache::DB_INSTANCE.lock().unwrap();

    // Get canonical path name
    let canonical_path = fs::canonicalize(file)?.to_string_lossy().into_owned();

    let metadata = fs::metadata(file)?;
    let modified: SystemTime = metadata.modified()?;
    let modified_timestamp = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|e| io::Error::new(ErrorKind::Other, e))?;

    // Serialize the canonical path and the modified timestamp into a Vec<u8>
    let key = format!("{}|{}", canonical_path, modified_timestamp.as_secs());
    let db_key = key.into_bytes();

    // Check if entry exists in RocksDB
    match db.get(&db_key) {
        Ok(Some(value)) => {
            let hash: u64 = bincode::deserialize(&value).unwrap();
            trace!("found hash for {} in cache", file.display());
            Ok(hash)
        }
        Ok(None) => {
            // If entry does not exist, calculate hash and save to RocksDB
            let data = read_full_file(file)?;
            let hash = hash_data(&data)?;
            trace!(
                "No hash found for {} in cache. adding to cache",
                file.display()
            );
            let _ = db.put(&db_key, bincode::serialize(&hash).unwrap());
            Ok(hash)
        }
        Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
    }
}
