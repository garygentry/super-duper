#![allow(dead_code)]

use super::super::hash_cache;
use bincode;
use dashmap::DashMap;
use rayon::prelude::*;
use rocksdb::{Options, DB};
use std::fs::{self, File};
use std::hash::Hasher as _;
use std::io::Read;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::trace;
use twox_hash::XxHash64;

const HASH_LENGTH: usize = 1024; // 1KB

// lazy_static::lazy_static! {
//     static ref DB_INSTANCE: Arc<Mutex<DB>> = {
//         let db_path = "content_hash_cache.db";
//         let mut db_options = Options::default();
//         db_options.create_if_missing(true);
//         let db_instance = DB::open(&db_options, db_path).expect("Failed to open database");
//         Arc::new(Mutex::new(db_instance))
//     };
// }

pub fn build_content_hash_map(
    size_to_file_map: DashMap<u64, Vec<PathBuf>>,
) -> io::Result<DashMap<u64, Vec<PathBuf>>> {
    let partial_hash_to_file_map: DashMap<u64, Vec<PathBuf>> = DashMap::new();
    let confirmed_duplicates: DashMap<u64, Vec<PathBuf>> = DashMap::new();

    let size_to_file_vec: Vec<_> = size_to_file_map.iter().collect();
    size_to_file_vec.par_iter().try_for_each(|files| {
        files
            .value()
            .par_iter()
            .try_for_each(|file| match read_portion(file) {
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
            })
    })?;

    let partial_hash_to_file_vec: Vec<_> = partial_hash_to_file_map.iter().collect();
    partial_hash_to_file_vec.par_iter().try_for_each(|files| {
        if files.value().len() > 1 {
            files
                .value()
                .par_iter()
                .try_for_each(|file| match get_content_hash(file) {
                    Ok(hash) => {
                        // let hash = hash_data(&data)?;
                        confirmed_duplicates
                            .entry(hash)
                            .or_default()
                            .push(file.clone());
                        Ok::<_, std::io::Error>(()) // Add type annotation for Ok(())
                    }
                    Err(e) => {
                        log_exception(file, &e);
                        Ok::<_, std::io::Error>(()) // Add type annotation for Ok(())
                    }
                })
        } else {
            Ok(())
        }
    })?;

    Ok(confirmed_duplicates)
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
