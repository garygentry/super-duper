#![allow(dead_code)]

use tracing::*;

use crate::model::ScanFile;
use rocksdb::{IteratorMode, Options, DB};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::MutexGuard;
use std::{
    env,
    sync::{Arc, Mutex},
};
use thiserror::Error;
use tracing::*;

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};

static DEFAULT_FILE_CACHE_PATH: &str = "cache";

lazy_static::lazy_static! {
    static ref FILE_CACHE_INSTANCE: Arc<Mutex<DB>> = {
        // Attempt to fetch the value of the environment variable HASH_CACHE_PATH
        let db_path = match env::var("CACHE_PATH") {
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
    pub canonical_name: String,
    pub file_size: i64,
    pub last_modified: SystemTime,
    pub full_hash: Option<u64>,
    pub partial_hash: Option<u64>,
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
            canonical_name,
            file_size: metadata.len() as i64,
            last_modified,
            full_hash: None,
            partial_hash: None,
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

    // pub fn from_scan_file(scan_file: &ScanFile) -> Result<CacheFile, Box<dyn std::error::Error>> {
    //     // let file_info = CacheFile::from_path_buf(&scan_file.path_buf, Some(&scan_file.metadata));
    //     let file_info = CacheFile::from_path_buf(&scan_file.path_buf, Some(&scan_file.metadata));
    //     // let metadata = &scan_file.metadata;

    //     // let canonical_name = fs::canonicalize(&scan_file.path_buf)?
    //     //     .to_string_lossy()
    //     //     .into_owned();
    //     // let last_modified: SystemTime = metadata.modified()?;

    //     // let file_info = CacheFile {
    //     //     canonical_name,
    //     //     file_size: metadata.len() as i64,
    //     //     last_modified,
    //     //     full_hash: None,
    //     //     partial_hash: None,
    //     // };

    //     Ok(file_info)
    // }

    // pub fn hash_key(&self) -> Result<Vec<u8>, HashKeyError> {
    pub fn get_cache_key(&self) -> io::Result<Vec<u8>> {
        let modified_timestamp = self
            .last_modified
            .duration_since(UNIX_EPOCH)
            .map_err(|e| io::Error::new(ErrorKind::Other, e))?;
        let key = format!("{}|{}", self.canonical_name, modified_timestamp.as_secs());
        Ok(key.into_bytes())
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

// #[derive(Debug, Error)]
// pub enum CacheError {
//     #[error("Mutex lock error: {0}")]
//     Mutex(#[from] std::sync::PoisonError<MutexGuard<'static, DB>>),
//     #[error("Deserialization error: {0}")]
//     Deserialization(#[from] bincode::Error),
//     #[error("Error retrieving value from cache: {0}")]
//     Retrieval(#[from] Box<dyn std::error::Error>),
// }

impl ScanFile {
    pub fn load_from_cache(&self) -> io::Result<CacheFile> {
        let cache_file = CacheFile::from_scan_file(&self)?;
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

// pub fn get_put_cache_file_path_buf(scan_file: &ScanFile) -> io::Result<CacheFile> {
//     let cf = CacheFile::from_scan_file(scan_file)?;
//     let db_key = cf.get_cache_key()?;
//     let db = FILE_CACHE_INSTANCE.lock().unwrap();
//     match db.get(db_key) {
//         Ok(Some(value)) => {
//             let cf = bincode::deserialize(&value).unwrap();
//             // trace!("found hash for {} in cache", scan_file.path_buf.display());
//             Ok(cf)
//         }
//         Ok(None) => {
//             drop(db);
//             put_cache_file(&cf).map_err(CacheError::Retrieval)?;
//             Ok(cf)
//         }
//         Err(e) => Err(CacheError::Retrieval(Box::new(e))),
//     }
// }

// pub fn get_put_cache_file_path_buf(scan_file: &ScanFile) -> Result<CacheFile, CacheError> {
//     let cf = CacheFile::from_scan_file(scan_file)?;
//     let db_key = cf.get_cache_key()?;
//     let db = FILE_CACHE_INSTANCE.lock().map_err(CacheError::Mutex)?;
//     match db.get(db_key) {
//         Ok(Some(value)) => {
//             let cf = bincode::deserialize(&value).map_err(CacheError::Deserialization)?;
//             // trace!("found hash for {} in cache", scan_file.path_buf.display());
//             Ok(cf)
//         }
//         Ok(None) => {
//             drop(db);
//             put_cache_file(&cf).map_err(CacheError::Retrieval)?;
//             Ok(cf)
//         }
//         Err(e) => Err(CacheError::Retrieval(Box::new(e))),
//     }
// }

// pub fn put_cache_file(cache_file: &CacheFile) -> Result<(), Box<dyn std::error::Error>> {
//     let db = FILE_CACHE_INSTANCE.lock()?;
//     let db_key = cache_file.get_cache_key()?;
//     let value = bincode::serialize(cache_file)?;
//     let _ = db.put(db_key, value);
//     Ok(())
// }

// pub fn delete_cache_file_path_buf(file: &PathBuf) -> Result<(), CacheError> {
//     let cf = CacheFile::from_path_buf(file, None)?;
//     let db_key = cf.get_cache_key()?;
//     let db = FILE_CACHE_INSTANCE.lock().map_err(CacheError::Mutex)?;
//     let _ = db.delete(db_key);
//     Ok(())
// }
