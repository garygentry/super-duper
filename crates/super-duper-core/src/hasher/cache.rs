//! Public maintenance entry points for the content-hash cache store.
//!
//! Scans own exactly one [`super::repeat_cache::RepeatHashCache`] handle and pass it through the
//! hash pipeline and exact-folder verification. RocksDB locks its directory even against a second
//! open from the same process, so nothing here keeps a process-global handle.

use std::env;
use std::io;
use std::path::{Path, PathBuf};
use tracing::{error, info};

const DEFAULT_HASH_CACHE_PATH: &str = "content_hash_cache.db";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheLookupOutcome {
    Hit,
    Miss,
    Error,
}

/// `HASH_CACHE_PATH`, or `content_hash_cache.db` in the working directory.
pub fn default_hash_cache_path() -> PathBuf {
    env::var_os("HASH_CACHE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_HASH_CACHE_PATH))
}

/// Count cached file entries without locking the store.
pub fn count_entries(path: &Path) -> io::Result<u64> {
    super::repeat_cache::count_live_entries(path)
}

/// Remove every cached entry. Fails while a scan holds the store.
pub fn clear_all(path: &Path) -> io::Result<()> {
    super::repeat_cache::clear_store(path)?;
    info!("Hash cache '{}' cleared", path.display());
    Ok(())
}

pub fn print_count(path: &Path) {
    match count_entries(path) {
        Ok(count) => info!(
            "Total entries in hash cache '{}': {}",
            path.display(),
            count
        ),
        Err(e) => error!("Error counting hash cache entries: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hasher::repeat_cache::{CacheSignatureKey, RepeatHashCache};
    use tempfile::TempDir;

    #[test]
    fn maintenance_counts_without_locking_and_clears_only_when_unlocked() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("cache");
        assert_eq!(count_entries(&path).unwrap(), 0);

        let cache = RepeatHashCache::open(&path).unwrap();
        let signature = CacheSignatureKey {
            stable_identity: "volume:1:file:1".into(),
            size: 4096,
            modified_unix_nanos: 1,
            content_change_token: "change:1".into(),
        };
        cache.store_full(&signature, 7, 11).unwrap();
        assert_eq!(
            count_entries(&path).unwrap(),
            1,
            "count reads an open store"
        );
        assert!(
            clear_all(&path).is_err(),
            "clear must not race an open handle"
        );
        drop(cache);

        clear_all(&path).unwrap();
        assert_eq!(count_entries(&path).unwrap(), 0);
        let reopened = RepeatHashCache::open(&path).unwrap();
        cache_is_empty(&reopened, &signature);
    }

    fn cache_is_empty(cache: &RepeatHashCache, signature: &CacheSignatureKey) {
        assert_eq!(
            cache.lookup(signature).unwrap(),
            crate::hasher::repeat_cache::RepeatCacheLookup::Miss
        );
    }
}
