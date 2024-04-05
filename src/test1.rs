// #![allow(dead_code)]

use rocksdb::{IteratorMode, Options, DB};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::{
    io,
    sync::{Arc, Mutex},
};
use tracing::{error, info};

static TEST1_DB_PATH: &str = "test1.db";

lazy_static::lazy_static! {
    pub static ref TEST1_DB_INSTANCE: Arc<Mutex<DB>> = {
        let mut db_options = Options::default();
        db_options.create_if_missing(true);
        let db_instance = DB::open(&db_options, TEST1_DB_PATH).expect("Failed to open database");
        Arc::new(Mutex::new(db_instance))
    };
}

fn count_keys_in_rocksdb() -> Result<usize, Box<dyn std::error::Error>> {
    let db = TEST1_DB_INSTANCE.lock().unwrap();

    // Initialize count to zero
    let mut count = 0usize;

    // Iterate through the keys and count them
    let iterator = DB::iterator(&db, IteratorMode::Start);
    for _ in iterator {
        count += 1;
    }

    Ok(count)
}

fn print_count() {
    match count_keys_in_rocksdb() {
        Ok(count) => info!("Total keys in hash cache: {}", count),
        Err(e) => error!("Error: {}", e),
    }
}

// fn get_content_hash(file: &PathBuf) -> io::Result<u64> {
//   let db = super::super::hash_cache::DB_INSTANCE.lock().unwrap();

//   // Get canonical path name
//   let canonical_path = fs::canonicalize(file)?.to_string_lossy().into_owned();

//   let metadata = fs::metadata(file)?;
//   let modified: SystemTime = metadata.modified()?;
//   let modified_timestamp = modified
//       .duration_since(UNIX_EPOCH)
//       .map_err(|e| io::Error::new(ErrorKind::Other, e))?;

//   // Serialize the canonical path and the modified timestamp into a Vec<u8>
//   let key = format!("{}|{}", canonical_path, modified_timestamp.as_secs());
//   let db_key = key.into_bytes();

//   // Check if entry exists in RocksDB
//   match db.get(&db_key) {
//       Ok(Some(value)) => {
//           let hash: u64 = bincode::deserialize(&value).unwrap();
//           trace!("found hash for {} in cache", file.display());
//           Ok(hash)
//       }
//       Ok(None) => {
//           // If entry does not exist, calculate hash and save to RocksDB
//           let data = read_full_file(file)?;
//           let hash = hash_data(&data)?;
//           trace!(
//               "No hash found for {} in cache. adding to cache",
//               file.display()
//           );
//           let _ = db.put(&db_key, bincode::serialize(&hash).unwrap());
//           Ok(hash)
//       }
//       Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
//   }
// }

#[derive(Serialize, Deserialize, Debug)]
struct Test1Value {
    field1: u64,
    opt_fld: Option<String>,
}

pub fn test1_() {
    let db = TEST1_DB_INSTANCE.lock().unwrap();
    let val = Test1Value {
        field1: 1,
        opt_fld: None,
    };
    let encoded: Vec<u8> = bincode::serialize(&val).unwrap();
    let key = format!("{}|{}", "key", 1);
    let db_key = key.into_bytes();

    info!("NOT Writing..");
    // db.put(&db_key, encoded).unwrap();

    match db.get(db_key) {
        Ok(Some(value)) => {
            let vresult: Test1Value = bincode::deserialize(&value).unwrap();
            info!("found val {:?} in cache", vresult);
        }
        Ok(None) => {
            info!("Not found bruh");
        }
        Err(e) => {
            error!("Error: {}", e);
        }
    }

    println!("Test1");
}

pub fn test1() {
    let f = fs::canonicalize("C:\\dev\\deduper\\test-data\\folder2\\file1.wav").unwrap();
    let fi = super::file_proc::file_info::to_file_info2(f);

    println!("Test1: {:?}", &fi);
}
