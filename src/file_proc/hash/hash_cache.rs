#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use rocksdb::{IteratorMode, Options, DB};

static DB_FILE_NAME: &str = "content_hash_cache.db";

lazy_static::lazy_static! {
    pub static ref DB_INSTANCE: Arc<Mutex<DB>> = {
        // let db_path = "content_hash_cache.db";
        let mut db_options = Options::default();
        db_options.create_if_missing(true);
        let db_instance = DB::open(&db_options, DB_FILE_NAME).expect("Failed to open database");
        Arc::new(Mutex::new(db_instance))
    };
}

pub fn count_keys_in_rocksdb() -> Result<usize, Box<dyn std::error::Error>> {
    // Open the RocksDB database
    // let mut options = Options::default();
    // options.create_if_missing(false);
    // let db = DB::open(&options, DB_FILE_NAME)?;

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

pub fn print_hash_cache_count() {
    match count_keys_in_rocksdb() {
        Ok(count) => println!("Total keys in hash cache: {}", count),
        Err(e) => eprintln!("Error: {}", e),
    }
}
