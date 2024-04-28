use super::hash;
use super::scan;
use super::status;
use super::status::{ CacheToDupeProcStatusMessage, StatusMessage };
use crate::store::dupe_file::DupeFile;
use crate::file_cache::CacheFile;
use dashmap::DashMap;
use rayon::iter::{ IntoParallelRefIterator, ParallelIterator };
use std::io;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Finds duplicate files based on input paths and ignore patterns.
///
/// This function takes a vector of input paths and a vector of ignore patterns as parameters.
/// It also takes a reference to a function that handles status messages as an `Arc<dyn Fn(status::StatusMessage) + Send + Sync>`.
/// The function builds a map of all files with non-distinct sizes, keyed on file size.
/// It then builds a map of all files with non-distinct content hashes, keyed on content hash.
/// Finally, it converts the hash map to a vector of `DupeFile` structs and returns it.
///
/// # Arguments
///
/// * `input_paths` - A vector of input paths to scan for duplicate files.
/// * `ignore_patterns` - A vector of ignore patterns to exclude certain files from the scan.
/// * `tx_status` - A reference to a function that handles status messages.
///
/// # Returns
///
/// A `Result` containing a vector of `DupeFile` structs if successful, or a boxed error if an error occurs.
pub fn build_dupe_files(
    input_paths: Vec<String>,
    ignore_patterns: Vec<String>,
    tx_status: &Arc<dyn Fn(status::StatusMessage) + Send + Sync>
) -> Result<Vec<DupeFile>, Box<dyn std::error::Error + Send>> {
    // build map of all files with non-distinct sizes keyed on file size
    let size_map = scan
        ::build_size_to_files_map(&input_paths, &ignore_patterns, tx_status)
        .unwrap();

    // build map of all files with non-distinct content hashes, keyed on content hash
    let hash_map = hash::build_content_hash_map(&size_map, tx_status).unwrap();

    // convert the hash map to a vector of DupeFile structs
    let dupe_files: Vec<DupeFile> = cache_file_map_to_dupe_files(
        hash_map,
        tx_status
    ).unwrap();

    // return the vector of DupeFile structs
    Ok(dupe_files)
}

/// Converts a DashMap of CacheFile structs to a vector of DupeFile structs.
///
/// This function takes a DashMap of CacheFile structs as input, along with a reference to a function that handles status messages.
/// It iterates over the entries in the DashMap in parallel, converting each CacheFile to a DupeFile.
/// The converted DupeFiles are collected into a vector and returned.
///
/// # Arguments
///
/// * `map` - A DashMap containing CacheFile structs, keyed on file size.
/// * `tx_status` - A reference to a function that handles status messages.
///
/// # Returns
///
/// An `io::Result` containing a vector of DupeFile structs if successful, or an `io::Error` if an error occurs.
fn cache_file_map_to_dupe_files(
    map: DashMap<u64, Vec<CacheFile>>,
    tx_status: &Arc<dyn Fn(status::StatusMessage) + Send + Sync>
) -> io::Result<Vec<DupeFile>> {
    tx_status(StatusMessage::CacheToDupeStart);
    let entries: Vec<_> = map.iter().collect();
    let dupe_file_vec: Result<Vec<_>, io::Error> = entries
        .par_iter()
        .flat_map(|entry| {
            let cache_files = entry.value();

            // TODO: Remove this sleep after testing
            thread::sleep(
                Duration::from_millis(
                    crate::debug::DEBUG_CACHE_TO_VEC_SLEEP_TIME
                )
            );

            tx_status(
                StatusMessage::CacheToDupeProc(CacheToDupeProcStatusMessage {
                    count: cache_files.len(),
                })
            );

            cache_files.par_iter().map(move |cache_file| {
                let dupe_file = DupeFile::from_cache_file(cache_file);
                Ok(dupe_file)
            })
        })
        .collect();
    tx_status(StatusMessage::CacheToDupeFinish);
    dupe_file_vec
}
