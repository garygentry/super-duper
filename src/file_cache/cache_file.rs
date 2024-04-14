#![allow(dead_code)]

use crate::file_proc::scan::ScanFile;
use rocksdb::{IteratorMode, Options, DB};
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    env,
    sync::{Arc, Mutex},
};
use std::{fs, io};
use tracing::*;

static DEFAULT_FILE_CACHE_PATH: &str = "$file_cache$";

lazy_static::lazy_static! {
    static ref FILE_CACHE_INSTANCE: Arc<Mutex<DB>> = {
        // Attempt to fetch the value of the environment variable HASH_CACHE_PATH
        let db_path = match env::var("FILE_CACHE_PATH") {
            Ok(val) => val,
            Err(_) => {
                String::from(DEFAULT_FILE_CACHE_PATH)
            }
        };
        debug!("Using '{}' for file cache", db_path);

        let mut db_options = Options::default();
        db_options.create_if_missing(true);
        let db_instance = DB::open(&db_options, db_path).expect("Failed to cache database");
        Arc::new(Mutex::new(db_instance))
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheFile {
    pub canonical_path: String,
    pub file_size: i64,
    pub last_modified: SystemTime,
    pub full_hash: Option<u64>,
    pub partial_hash: Option<u64>,
    pub path: PathBuf,
}

impl CacheFile {
    pub fn from_path_buf(path: &PathBuf, metadata: Option<&fs::Metadata>) -> io::Result<CacheFile> {
        let metadata = match metadata {
            Some(meta) => meta.clone(),
            None => fs::metadata(path)?,
        };
        let canonical_name = fs::canonicalize(path)?.to_string_lossy().into_owned();
        let last_modified: SystemTime = metadata.modified()?;

        let file_info = CacheFile {
            canonical_path: canonical_name,
            file_size: metadata.len() as i64,
            last_modified,
            full_hash: None,
            partial_hash: None,
            path: path.clone(),
        };

        Ok(file_info)
    }

    pub fn from_scan_file(scan_file: &ScanFile) -> io::Result<CacheFile> {
        CacheFile::from_path_buf(&scan_file.path_buf, Some(&scan_file.metadata))
    }

    pub fn put(&self) -> io::Result<()> {
        let db = FILE_CACHE_INSTANCE.lock().unwrap();
        let db_key = &self.get_cache_key()?;
        let value = bincode::serialize(&self).unwrap();
        let _ = db.put(db_key, value);
        Ok(())
    }

    pub fn get_cache_key(&self) -> io::Result<Vec<u8>> {
        let modified_timestamp = self
            .last_modified
            .duration_since(UNIX_EPOCH)
            .map_err(|e| io::Error::new(ErrorKind::Other, e))?;
        let key = format!("{}|{}", self.canonical_path, modified_timestamp.as_secs());
        Ok(key.into_bytes())
    }
}

impl ScanFile {
    pub fn load_from_cache(&self) -> io::Result<CacheFile> {
        let cache_file = CacheFile::from_scan_file(self)?;
        let db_key = cache_file.get_cache_key()?;
        let db = FILE_CACHE_INSTANCE.lock().unwrap();
        match db.get(db_key) {
            Ok(Some(value)) => {
                let cf = bincode::deserialize(&value).unwrap();
                Ok(cf)
            }
            Ok(None) => {
                drop(db);
                Ok(cache_file)
            }
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
        }
    }
}

pub fn print_all_cache_values() {
    let db = FILE_CACHE_INSTANCE.lock().unwrap();

    // Iterate through the keys and count them
    let iter = db.iterator(IteratorMode::Start);

    for result in iter {
        match result {
            Ok((key, value)) => {
                // Convert the key and value bytes to strings (or any other desired format)
                let key_str = String::from_utf8_lossy(&key);
                let cf: CacheFile = bincode::deserialize(&value).unwrap();

                // Print the key and value
                println!("Key: {}, Value: {:?}", key_str, cf);
            }
            Err(err) => {
                // Handle the error
                eprintln!("Error iterating through keys: {:?}", err);
            }
        }
    }
}

pub fn get_file_cache_len() -> Result<usize, Box<dyn std::error::Error>> {
    let db = FILE_CACHE_INSTANCE.lock().unwrap();

    // Initialize count to zero
    let mut count = 0usize;

    // Iterate through the keys and count them
    let iterator = DB::iterator(&db, IteratorMode::Start);
    for _ in iterator {
        count += 1;
    }

    Ok(count)
}

pub fn print_count() {
    match get_file_cache_len() {
        Ok(count) => info!("Total keys in hash cache: {}", count),
        Err(e) => error!("Error: {}", e),
    }
}
