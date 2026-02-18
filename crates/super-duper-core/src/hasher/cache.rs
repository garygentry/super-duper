use rocksdb::{IteratorMode, Options, DB};
use std::env;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, trace};

static DEFAULT_HASH_CACHE_PATH: &str = "content_hash_cache.db";

lazy_static::lazy_static! {
    pub static ref DB_INSTANCE: Arc<Mutex<DB>> = {
        let db_path = env::var("HASH_CACHE_PATH")
            .unwrap_or_else(|_| String::from(DEFAULT_HASH_CACHE_PATH));
        debug!("Using '{}' for hash cache", db_path);

        let mut db_options = Options::default();
        db_options.create_if_missing(true);
        let db_instance = DB::open(&db_options, db_path)
            .expect("Failed to open RocksDB hash cache");
        Arc::new(Mutex::new(db_instance))
    };
}

/// Look up a file's content hash in the RocksDB cache.
/// Cache key includes subsecond timestamp precision to avoid stale entries.
/// On cache miss, reads the full file, hashes it, and stores the result.
pub fn get_content_hash(file: &Path) -> io::Result<u64> {
    let db = DB_INSTANCE
        .lock()
        .map_err(|e| io::Error::new(ErrorKind::Other, format!("Failed to lock cache: {}", e)))?;

    let canonical_path = fs::canonicalize(file)?.to_string_lossy().into_owned();
    let metadata = fs::metadata(file)?;
    let modified: SystemTime = metadata.modified()?;
    let modified_timestamp = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|e| io::Error::new(ErrorKind::Other, e))?;

    // Include subsec_nanos for precision (fixes second-granularity cache key issue)
    let key = format!(
        "{}|{}.{}",
        canonical_path,
        modified_timestamp.as_secs(),
        modified_timestamp.subsec_nanos()
    );
    let db_key = key.into_bytes();

    match db.get(&db_key) {
        Ok(Some(value)) => {
            let hash: u64 = bincode::deserialize(&value)
                .map_err(|e| {
                    io::Error::new(ErrorKind::Other, format!("Deserialize error: {}", e))
                })?;
            trace!("Found hash for {} in cache", file.display());
            Ok(hash)
        }
        Ok(None) => {
            let data = super::xxhash::read_full_file(file)?;
            let hash = super::xxhash::hash_data(&data);
            trace!("No hash found for {} in cache, adding", file.display());
            let serialized = bincode::serialize(&hash)
                .map_err(|e| {
                    io::Error::new(ErrorKind::Other, format!("Serialize error: {}", e))
                })?;
            let _ = db.put(&db_key, serialized);
            Ok(hash)
        }
        Err(e) => Err(io::Error::new(ErrorKind::Other, e)),
    }
}

pub fn count_keys() -> Result<usize, io::Error> {
    let db = DB_INSTANCE
        .lock()
        .map_err(|e| io::Error::new(ErrorKind::Other, format!("Failed to lock cache: {}", e)))?;

    let mut count = 0usize;
    let iterator = DB::iterator(&db, IteratorMode::Start);
    for _ in iterator {
        count += 1;
    }
    Ok(count)
}

pub fn clear_all() -> io::Result<()> {
    let db = DB_INSTANCE
        .lock()
        .map_err(|e| io::Error::new(ErrorKind::Other, format!("Failed to lock cache: {}", e)))?;

    let mut batch = rocksdb::WriteBatch::default();
    for item in db.iterator(IteratorMode::Start) {
        let (key, _) = item.map_err(|e| io::Error::new(ErrorKind::Other, e))?;
        batch.delete(&key);
    }
    db.write(batch)
        .map_err(|e| io::Error::new(ErrorKind::Other, e))?;
    info!("Hash cache cleared");
    Ok(())
}

pub fn print_count() {
    match count_keys() {
        Ok(count) => info!("Total keys in hash cache: {}", count),
        Err(e) => error!("Error counting cache keys: {}", e),
    }
}
