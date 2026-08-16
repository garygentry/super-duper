use rocksdb::{IteratorMode, Options, DB};
use std::env;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, trace};

static DEFAULT_HASH_CACHE_PATH: &str = "content_hash_cache.db";

lazy_static::lazy_static! {
    pub static ref DB_INSTANCE: Arc<Mutex<Result<DB, String>>> = {
        let db_path = env::var("HASH_CACHE_PATH")
            .unwrap_or_else(|_| String::from(DEFAULT_HASH_CACHE_PATH));
        debug!("Using '{}' for hash cache", db_path);

        let mut db_options = Options::default();
        db_options.create_if_missing(true);
        let db_instance = DB::open(&db_options, db_path).map_err(|error| error.to_string());
        Arc::new(Mutex::new(db_instance))
    };
}

#[derive(Debug)]
pub struct CachedHash {
    pub hash: u64,
    pub warning: Option<String>,
}

/// Compatibility entry point for callers that do not need cancellation or cache warnings.
pub fn get_content_hash(file: &Path) -> io::Result<u64> {
    let cancel = AtomicBool::new(false);
    Ok(get_content_hash_cancellable(file, &cancel)?.hash)
}

/// Look up a file hash with short cache critical sections. Cache failures are returned as a
/// warning after the file is hashed so callers can safely continue the scan.
pub fn get_content_hash_cancellable(
    file: &Path,
    cancel_token: &AtomicBool,
) -> io::Result<CachedHash> {
    let canonical_path = fs::canonicalize(file)?.to_string_lossy().into_owned();
    let metadata = fs::metadata(file)?;
    let size = metadata.len();
    let modified_timestamp = metadata_modified_timestamp(&metadata)?;

    // Include subsec_nanos for precision (fixes second-granularity cache key issue)
    let key = format!(
        "{}|{}|{}.{}",
        canonical_path,
        size,
        modified_timestamp.as_secs(),
        modified_timestamp.subsec_nanos()
    );
    let db_key = key.into_bytes();

    let lookup = DB_INSTANCE
        .lock()
        .map_err(|e| io::Error::new(ErrorKind::Other, format!("Failed to lock cache: {e}")))
        .and_then(|db| match db.as_ref() {
            Ok(db) => db
                .get(&db_key)
                .map_err(|e| io::Error::new(ErrorKind::Other, e)),
            Err(error) => Err(io::Error::new(ErrorKind::Other, error.clone())),
        });

    let mut warning = None;
    match lookup {
        Ok(Some(value)) => match bincode::deserialize::<u64>(&value) {
            Ok(hash) => {
                trace!("Found hash for {} in cache", file.display());
                return Ok(CachedHash {
                    hash,
                    warning: None,
                });
            }
            Err(error) => {
                warning = Some(format!("Hash cache entry could not be decoded: {error}"));
            }
        },
        Ok(None) => {}
        Err(error) => {
            warning = Some(format!("Hash cache lookup failed: {error}"));
        }
    }

    let hash = super::xxhash::hash_file_streaming(file, cancel_token)?;
    let metadata_after_hash = fs::metadata(file)?;
    if metadata_after_hash.len() != size
        || metadata_modified_timestamp(&metadata_after_hash)? != modified_timestamp
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "file changed while it was being hashed",
        ));
    }
    trace!(
        "No usable hash found for {} in cache, adding",
        file.display()
    );
    let store = bincode::serialize(&hash)
        .map_err(|e| io::Error::new(ErrorKind::Other, format!("Serialize error: {e}")))
        .and_then(|serialized| {
            DB_INSTANCE
                .lock()
                .map_err(|e| io::Error::new(ErrorKind::Other, format!("Failed to lock cache: {e}")))
                .and_then(|db| match db.as_ref() {
                    Ok(db) => db
                        .put(&db_key, serialized)
                        .map_err(|e| io::Error::new(ErrorKind::Other, e)),
                    Err(error) => Err(io::Error::new(ErrorKind::Other, error.clone())),
                })
        });
    if let Err(error) = store {
        let message = format!("Hash cache store failed: {error}");
        warning = Some(match warning {
            Some(previous) => format!("{previous}; {message}"),
            None => message,
        });
    }
    Ok(CachedHash { hash, warning })
}

fn metadata_modified_timestamp(metadata: &fs::Metadata) -> io::Result<std::time::Duration> {
    let modified: SystemTime = metadata.modified()?;
    modified
        .duration_since(UNIX_EPOCH)
        .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))
}

pub fn count_keys() -> Result<usize, io::Error> {
    let db = DB_INSTANCE
        .lock()
        .map_err(|e| io::Error::new(ErrorKind::Other, format!("Failed to lock cache: {}", e)))?;
    let db = db
        .as_ref()
        .map_err(|error| io::Error::new(ErrorKind::Other, error.clone()))?;

    let mut count = 0usize;
    let iterator = DB::iterator(db, IteratorMode::Start);
    for _ in iterator {
        count += 1;
    }
    Ok(count)
}

pub fn clear_all() -> io::Result<()> {
    let db = DB_INSTANCE
        .lock()
        .map_err(|e| io::Error::new(ErrorKind::Other, format!("Failed to lock cache: {}", e)))?;
    let db = db
        .as_ref()
        .map_err(|error| io::Error::new(ErrorKind::Other, error.clone()))?;

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
