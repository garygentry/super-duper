#![allow(dead_code)]

use super::status::*;
use crate::file_cache::*;
use crate::file_proc::scan::ScanFile;
use dashmap::DashMap;
use rayon::prelude::*;
use std::fs::File;
use std::hash::Hasher as _;
use std::io::Read;
use std::path::Path;
use std::sync::{mpsc, Arc};
use std::time::Duration;
use std::{io, thread};
use tracing::*;
use twox_hash::XxHash64;

const HASH_LENGTH: usize = 1024; // 1KB

/// Takes a map keyed on file size, with each entry containing vector os paths
/// that are of the that size, and returns a map of unique content hashes with
/// value containing vector of all
pub fn build_content_hash_map(
    size_to_file_map: &DashMap<u64, Vec<ScanFile>>,
    tx_status: &Arc<dyn Fn(StatusMessage) + Send + Sync>,
) -> Result<DashMap<u64, Vec<CacheFile>>, Box<dyn std::error::Error>> {
    tx_status(StatusMessage::HashBegin);

    let confirmed_duplicates: DashMap<u64, Vec<CacheFile>> = DashMap::new();

    // Convert the DashMap to a vector we can iterate over
    let size_to_file_vec: Vec<_> = size_to_file_map.iter().collect();

    // Iterate over map keyed on file size, with value of all files that match that file_size
    // size_to_file_vec.par_iter().try_for_each(|scan_files| {
    size_to_file_vec.iter().try_for_each(|scan_files| {
        tx_status(StatusMessage::HashProcScanFiles(
            HashProcScanFilesStatusMessage {
                count: scan_files.len(),
                file_size: *scan_files.key(),
            },
        ));

        // TODO: Remove this sleep after testing
        thread::sleep(Duration::from_millis(10));

        // Load all cache files for the scan files into new vector of CacheFile type
        let cache_files: Vec<CacheFile> = scan_files
            .value()
            .iter()
            .filter_map(|scan_file| {
                scan_file
                    .load_from_cache()
                    .map_err(|err| {
                        // Log the file name and the error, then continue processing
                        error!(
                            "Error loading cache file: {:?} - {}",
                            scan_file.path_buf, err
                        );
                    })
                    .ok() // Skip the element if there's an error
            })
            .collect();

        if cache_files.is_empty() {
            // If all files encountered errors, just return early
            return Ok(());
        }

        // If all files are fully cached, we can skip the hashing
        if is_fully_cached(cache_files.clone()) {
            cache_files.iter().for_each(|file| {
                confirmed_duplicates
                    .entry(file.full_hash.unwrap())
                    .or_default()
                    .push(file.clone());
            });
            trace!("All files are fully cached for size: {}", scan_files.key());
            return Ok(());
        } else {
            // map of files keyed on hash of first few bytes of the file (maybe/likely dupe)
            let partial_hash_to_file_map: DashMap<u64, Vec<CacheFile>> = DashMap::new();
            // map of files with keyed on hash of full contents (definite dupe)
            let full_hash_to_file_map: DashMap<u64, Vec<CacheFile>> = DashMap::new();

            // Iterate cache_files to populate on partial hash to quickly eliminate non-dupes
            cache_files.par_iter().try_for_each(|scan_file| {
                populate_partial_hash_map(scan_file, &partial_hash_to_file_map)
            })?;

            // Now iterate possible dupes matching first few bytes to fully hash the files to be sure
            let partial_hash_to_file_vec: Vec<_> = partial_hash_to_file_map.iter().collect();
            partial_hash_to_file_vec
                .par_iter()
                .try_for_each(|cache_files| {
                    // if only one entry, there is no dupe..
                    if cache_files.value().len() > 1 {
                        cache_files.value().par_iter().try_for_each(|file| {
                            populate_full_hash_map(file, &full_hash_to_file_map)
                        })?;
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
        }

        Ok::<_, std::io::Error>(())
    })?;

    tx_status(StatusMessage::HashEnd);

    Ok(confirmed_duplicates)
}

fn is_fully_cached(cache_files: Vec<CacheFile>) -> bool {
    cache_files.iter().all(|file| file.full_hash.is_some())
}

fn populate_partial_hash_map(
    cache_file: &CacheFile,
    map: &DashMap<u64, Vec<CacheFile>>,
) -> io::Result<()> {
    // check whether the file is already partially hashed (from cache)
    if cache_file.partial_hash.is_some() {
        trace!(
            "Partial Hash IS cached for {}: {}",
            cache_file.canonical_path,
            cache_file.partial_hash.unwrap()
        );
        map.entry(cache_file.partial_hash.unwrap())
            .or_default()
            .push(cache_file.clone());
    } else {
        // create a new hash for the partial file contents
        let mut cache_file = cache_file.clone(); // Change the type of cache_file to CacheFile
        let hash = build_partial_hash(&cache_file)?; // Pass a reference to cache_file
        cache_file.partial_hash = Some(hash);
        trace!(
            "Partial Hash NOT cached. Put partial hash for {}: {}",
            cache_file.canonical_path,
            hash
        );
        // save file to cache with new hash
        let _ = cache_file.put();
        // add file to map
        map.entry(hash).or_default().push(cache_file);
    }

    Ok(())
}

fn build_partial_hash(cache_file: &CacheFile) -> io::Result<u64> {
    match read_portion(cache_file) {
        Ok(data) => {
            let hash = hash_data(&data)?;
            Ok(hash)
        }
        Err(e) => Err(e),
    }
}

fn build_full_hash(cache_file: &CacheFile) -> io::Result<u64> {
    match read_full_file(cache_file) {
        Ok(data) => {
            let hash = hash_data(&data)?;
            Ok(hash)
        }
        Err(e) => Err(e),
    }
}

fn read_portion(file: &CacheFile) -> std::io::Result<Vec<u8>> {
    let mut f = File::open(&file.canonical_path)?;
    let mut buffer = vec![0; HASH_LENGTH];

    // Read up to HASH_LENGTH bytes
    let bytes_read = f.read(&mut buffer)?;

    // Shrink the buffer to the actual number of bytes read
    buffer.truncate(bytes_read);

    Ok(buffer)
}

fn populate_full_hash_map(
    cache_file: &CacheFile,
    map: &DashMap<u64, Vec<CacheFile>>,
) -> io::Result<()> {
    // check whether the file is already partially hashed (from cache)
    if cache_file.full_hash.is_some() {
        trace!(
            "Full Hash IS cached for {}: {}",
            cache_file.canonical_path,
            cache_file.full_hash.unwrap()
        );
        map.entry(cache_file.full_hash.unwrap())
            .or_default()
            .push(cache_file.clone());
    } else {
        // create a new hash for the partial file contents
        let mut cache_file = cache_file.clone(); // Change the type of cache_file to CacheFile
        let hash = build_full_hash(&cache_file)?; // Pass a reference to cache_file
        cache_file.full_hash = Some(hash);
        trace!(
            "Full Hash NOT cached. Put full hash for {}: {}",
            cache_file.canonical_path,
            hash
        );

        // save file to cache with new hash
        let _ = cache_file.put();
        // add file to map
        map.entry(hash).or_default().push(cache_file);
    }

    Ok(())
}

fn read_full_file(file: &CacheFile) -> io::Result<Vec<u8>> {
    let mut f = File::open(&file.canonical_path)?;
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

// fn get_content_hash(file: &PathBuf) -> io::Result<u64> {
//     let db = super::super::hash_cache::DB_INSTANCE.lock().unwrap();

//     // Get canonical path name
//     let canonical_path = fs::canonicalize(file)?.to_string_lossy().into_owned();

//     let metadata = fs::metadata(file)?;
//     let modified: SystemTime = metadata.modified()?;
//     let modified_timestamp = modified
//         .duration_since(UNIX_EPOCH)
//         .map_err(|e| io::Error::new(ErrorKind::Other, e))?;

//     // Serialize the canonical path and the modified timestamp into a Vec<u8>
//     let key = format!("{}|{}", canonical_path, modified_timestamp.as_secs());
//     let db_key = key.into_bytes();

//     // Check if entry exists in RocksDB
//     match db.get(&db_key) {
//         Ok(Some(value)) => {
//             let hash: u64 = bincode::deserialize(&value).unwrap();
//             trace!("found hash for {} in cache", file.display());
//             Ok(hash)
//         }
//         Ok(None) => {
//             // If entry does not exist, calculate hash and save to RocksDB
//             let data = read_full_file(file)?;
//             let hash = hash_data(&data)?;
//             trace!(
//                 "No hash found for {} in cache. adding to cache",
//                 file.display()
//             );
//             let _ = db.put(&db_key, bincode::serialize(&hash).unwrap());
//             Ok(hash)
//         }
//         Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
//     }
// }
