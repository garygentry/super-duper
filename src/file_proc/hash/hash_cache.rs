// #![allow(dead_code)]

use rocksdb::{IteratorMode, Options, DB};
use std::{
    env,
    sync::{Arc, Mutex},
};
use tracing::{debug, error, info};

static DEFAULT_HASH_CACHE_PATH: &str = "content_hash_cache.db";

lazy_static::lazy_static! {
    pub static ref DB_INSTANCE: Arc<Mutex<DB>> = {
        // Attempt to fetch the value of the environment variable HASH_CACHE_PATH
        let db_path = match env::var("HASH_CACHE_PATH") {
            Ok(val) => val,
            Err(_) => {
                String::from(DEFAULT_HASH_CACHE_PATH)
            }
        };
        debug!("Using '{}' for hash cache", db_path);

        let mut db_options = Options::default();
        db_options.create_if_missing(true);
        let db_instance = DB::open(&db_options, db_path).expect("Failed to open database");
        Arc::new(Mutex::new(db_instance))
    };
}

pub fn count_keys_in_rocksdb() -> Result<usize, Box<dyn std::error::Error>> {
    let db = DB_INSTANCE.lock().unwrap();

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
    match count_keys_in_rocksdb() {
        Ok(count) => info!("Total keys in hash cache: {}", count),
        Err(e) => error!("Error: {}", e),
    }
}
