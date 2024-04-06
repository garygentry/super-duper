use crate::file_cache::CacheFile;
use rocksdb::{IteratorMode, Options, DB};
use std::path::PathBuf;
use std::{
    env,
    sync::{Arc, Mutex},
};
use tracing::*;

lazy_static::lazy_static! {
    static ref FILE_CACHE_INSTANCE: Arc<Mutex<DB>> = {
        // Attempt to fetch the value of the environment variable HASH_CACHE_PATH
        let db_path = match env::var("OLD_FILE_CACHE_PATH") {
            Ok(val) => val,
            Err(_) => {
                panic!("OLD_FILE_CACHE_PATH not set");
            }
        };
        debug!("Using '{}' for old file cache", db_path);

        let mut db_options = Options::default();
        db_options.create_if_missing(true);
        let db_instance = DB::open(&db_options, db_path).expect("Failed to cache database");
        Arc::new(Mutex::new(db_instance))
    };
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

// pub fn migrate_old_cache_version() {
//     let db = FILE_CACHE_INSTANCE.lock().unwrap();

//     // Iterate through the keys and count them
//     let iter = db.iterator(IteratorMode::Start);

//     let mut count = 0;

//     for result in iter {
//         match result {
//             Ok((key, value)) => {
//                 // Convert the key and value bytes to strings (or any other desired format)
//                 let key_str = String::from_utf8_lossy(&key);
//                 let hash: u64 = bincode::deserialize(&value).unwrap();
//                 let key_parts: Vec<&str> = key_str.split('|').collect();
//                 let path = PathBuf::from(key_parts[0]);
//                 let mut cf = CacheFile::from_path_buf(&path, None).unwrap();
//                 cf.full_hash = Some(hash);
//                 let _ = cf.put();
//                 count += 1;
//             }
//             Err(err) => {
//                 // Handle the error
//                 eprintln!("Error iterating through keys: {:?}", err);
//             }
//         }
//     }
//     info!("Migrated {} entries from old cache", count);
// }

pub fn migrate_old_cache_version() {
    let db = FILE_CACHE_INSTANCE.lock().unwrap();

    // Iterate through the keys and count them
    let iter = db.iterator(IteratorMode::Start);

    let mut count = 0;

    for result in iter {
        match result {
            Ok((key, value)) => {
                // Convert the key and value bytes to strings (or any other desired format)
                let key_str = String::from_utf8_lossy(&key);
                let hash: u64 = bincode::deserialize(&value).unwrap();
                let key_parts: Vec<&str> = key_str.split('|').collect();
                let path = PathBuf::from(key_parts[0]);
                let mut cf = match CacheFile::from_path_buf(&path, None) {
                    Ok(cf) => cf,
                    Err(err) => {
                        // Handle the error and continue the loop
                        eprintln!("Error creating CacheFile for {}: {:?}", key_str, err);
                        continue;
                    }
                };
                cf.full_hash = Some(hash);
                let _ = cf.put();
                count += 1;
            }
            Err(err) => {
                // Handle the error
                eprintln!("Error iterating through keys: {:?}", err);
            }
        }
    }
    info!("Migrated {} entries from old cache", count);
}
