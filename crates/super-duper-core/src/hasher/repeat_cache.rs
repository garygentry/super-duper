use crate::storage::models::RepeatCachePolicy;
use rocksdb::{DB, Direction, IteratorMode, Options, WriteBatch};
use serde::{Deserialize, Serialize};
use std::io::{self, ErrorKind};
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) const STORE_SCHEMA_VERSION: u32 = 3;
const SCHEMA_KEY: &[u8] = b"\0super-duper/repeat-cache/schema";
const COUNT_KEY: &[u8] = b"\0super-duper/repeat-cache/count";
const NEXT_SEQUENCE_KEY: &[u8] = b"\0super-duper/repeat-cache/next-sequence";
const ACTIVE_GENERATION_KEY: &[u8] = b"\0super-duper/repeat-cache/active-generation";
const NEXT_GENERATION_KEY: &[u8] = b"\0super-duper/repeat-cache/next-generation";
const ENTRY_PREFIX: &[u8] = b"\0super-duper/repeat-cache/entry/";
const ORDER_PREFIX: &[u8] = b"\0super-duper/repeat-cache/order/";
/// One key per live entry, keyed the same way as `ENTRY_PREFIX` (prefix swapped), value a
/// big-endian `u64` generation. Kept out of `StoredEntry` because its bincode encoding is pinned
/// (`stored_encoding_bytes_are_pinned`); this lets last-seen tracking evolve independently and
/// keeps every existing on-disk entry byte-identical. Written by [`RepeatHashCache::mark_seen`] on
/// a confirmed-unchanged cache hit (the hot path that otherwise performs no write at all) and
/// refreshed whenever an entry is stored or upgraded. [`trim_unseen`] reads it directly off the raw
/// store; an entry with no last-seen key yet (written before this trim existed) falls back to the
/// generation it was created in.
const LAST_SEEN_PREFIX: &[u8] = b"\0super-duper/repeat-cache/last-seen/";

pub(crate) const NORMAL_LIVE_TARGET_ENTRIES: u64 = 5_000_000;
pub(crate) const POST_PRUNE_TARGET_ENTRIES: u64 = 4_500_000;
pub(crate) const ACTIVE_HARD_HIGH_WATER_ENTRIES: u64 = 10_000_000;
pub(crate) const MAXIMUM_STABLE_IDENTITY_BYTES: usize = 512;
pub(crate) const MAXIMUM_CHANGE_TOKEN_BYTES: usize = 256;
pub(crate) const MAXIMUM_ENCODED_KEY_BYTES: usize = 1024;
pub(crate) const MAXIMUM_ENCODED_VALUE_BYTES: usize = 128;
pub(crate) const MAXIMUM_ENCODED_ORDER_KEY_BYTES: usize =
    ORDER_PREFIX.len() + 10 + MAXIMUM_ENCODED_KEY_BYTES;
const PRUNE_BATCH_ENTRIES: usize = 1024;

/// bincode 1.x byte compatibility for persisted keys and values; see the pinned-encoding test.
const STORED_ENCODING: bincode::config::Configuration<
    bincode::config::LittleEndian,
    bincode::config::Fixint,
> = bincode::config::legacy();

impl FromStr for RepeatCachePolicy {
    type Err = io::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "reuse_verified" => Ok(Self::ReuseVerified),
            "revalidate_content" => Ok(Self::RevalidateContent),
            _ => Err(io::Error::new(
                ErrorKind::InvalidInput,
                format!("unsupported repeat-cache policy '{value}'"),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CacheSignatureKey {
    pub stable_identity: String,
    pub size: u64,
    pub modified_unix_nanos: i64,
    pub content_change_token: String,
}

pub(crate) trait ContentSignatureProbe: Send + Sync {
    fn observe(&self, path: &Path) -> io::Result<crate::platform::ContentSignatureMetadata>;
}

#[derive(Debug, Default)]
pub(crate) struct SystemContentSignatureProbe;

impl ContentSignatureProbe for SystemContentSignatureProbe {
    fn observe(&self, path: &Path) -> io::Result<crate::platform::ContentSignatureMetadata> {
        crate::platform::content_signature_metadata(path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignatureIneligibleReason {
    MetadataUnavailable,
    StableIdentityUnavailable,
    ModifiedTimeUnavailable,
    CoarseModifiedTime,
    ContentChangeTokenUnavailable,
    InvalidSignature,
}

impl SignatureIneligibleReason {
    #[allow(dead_code)]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::MetadataUnavailable => "metadata_unavailable",
            Self::StableIdentityUnavailable => "stable_identity_unavailable",
            Self::ModifiedTimeUnavailable => "modified_time_unavailable",
            Self::CoarseModifiedTime => "coarse_modified_time",
            Self::ContentChangeTokenUnavailable => "content_change_token_unavailable",
            Self::InvalidSignature => "invalid_signature",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContentSignatureObservation {
    Qualified(CacheSignatureKey),
    Ineligible(SignatureIneligibleReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContentSignatureWindow {
    Unchanged(CacheSignatureKey),
    Changed,
    Ineligible(SignatureIneligibleReason),
}

pub(crate) fn observe_content_signature(
    path: &Path,
    probe: &dyn ContentSignatureProbe,
) -> ContentSignatureObservation {
    let metadata = match probe.observe(path) {
        Ok(metadata) => metadata,
        Err(_) => {
            return ContentSignatureObservation::Ineligible(
                SignatureIneligibleReason::MetadataUnavailable,
            );
        }
    };
    let stable_identity = match metadata.stable_identity {
        Some(value) => value,
        None => {
            return ContentSignatureObservation::Ineligible(
                SignatureIneligibleReason::StableIdentityUnavailable,
            );
        }
    };
    let modified_unix_nanos = match metadata.modified_unix_nanos {
        Some(value) if value > 0 => value,
        _ => {
            return ContentSignatureObservation::Ineligible(
                SignatureIneligibleReason::ModifiedTimeUnavailable,
            );
        }
    };
    if metadata.modified_time_is_coarse {
        return ContentSignatureObservation::Ineligible(
            SignatureIneligibleReason::CoarseModifiedTime,
        );
    }
    let content_change_token = match metadata.content_change_token {
        Some(value) => value,
        None => {
            return ContentSignatureObservation::Ineligible(
                SignatureIneligibleReason::ContentChangeTokenUnavailable,
            );
        }
    };
    let signature = CacheSignatureKey {
        stable_identity,
        size: metadata.size,
        modified_unix_nanos,
        content_change_token,
    };
    match signature.validate() {
        Ok(()) => ContentSignatureObservation::Qualified(signature),
        Err(_) => {
            ContentSignatureObservation::Ineligible(SignatureIneligibleReason::InvalidSignature)
        }
    }
}

pub(crate) fn compare_content_signatures(
    before: ContentSignatureObservation,
    after: ContentSignatureObservation,
) -> ContentSignatureWindow {
    match (before, after) {
        (
            ContentSignatureObservation::Qualified(before),
            ContentSignatureObservation::Qualified(after),
        ) if before == after => ContentSignatureWindow::Unchanged(before),
        (ContentSignatureObservation::Qualified(_), ContentSignatureObservation::Qualified(_)) => {
            ContentSignatureWindow::Changed
        }
        (ContentSignatureObservation::Ineligible(reason), _)
        | (_, ContentSignatureObservation::Ineligible(reason)) => {
            ContentSignatureWindow::Ineligible(reason)
        }
    }
}

impl CacheSignatureKey {
    pub(crate) fn validate(&self) -> io::Result<()> {
        validate_bounded_text(
            &self.stable_identity,
            MAXIMUM_STABLE_IDENTITY_BYTES,
            "stable identity",
        )?;
        validate_bounded_text(
            &self.content_change_token,
            MAXIMUM_CHANGE_TOKEN_BYTES,
            "content-change token",
        )?;
        if self.modified_unix_nanos <= 0 {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "modified time must be a positive nanosecond timestamp",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CachedContentHashes {
    pub partial_hash: u64,
    pub full_hash: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RepeatCacheLookup {
    Hit(CachedContentHashes),
    Miss,
    Ineligible(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepeatCacheStoreOutcome {
    Stored,
    Replayed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RepeatCacheStats {
    pub live_entries: u64,
    pub encoded_key_bytes: u64,
    pub encoded_value_bytes: u64,
}

/// Report from [`trim_unseen`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RepeatCacheTrimReport {
    pub live_entries_before: u64,
    pub removed: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredEntry {
    version: u32,
    sequence: u64,
    generation: u64,
    hashes: CachedContentHashes,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredEntryV2 {
    version: u32,
    sequence: u64,
    hashes: CachedContentHashes,
}

#[derive(Debug, Clone, Copy)]
struct StoreLimits {
    normal_live_target: u64,
    post_prune_target: u64,
    active_hard_high_water: u64,
}

impl StoreLimits {
    fn validate(self) -> io::Result<Self> {
        if self.post_prune_target == 0
            || self.post_prune_target >= self.normal_live_target
            || self.normal_live_target >= self.active_hard_high_water
        {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "repeat-cache limits require 0 < post-prune < normal target < active hard high-water",
            ));
        }
        Ok(self)
    }
}

pub(crate) struct RepeatHashCache {
    db: DB,
    limits: StoreLimits,
    writes: Mutex<()>,
    generation: u64,
    generation_completed: AtomicBool,
}

impl RepeatHashCache {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        Self::open_with_limits(
            path,
            StoreLimits {
                normal_live_target: NORMAL_LIVE_TARGET_ENTRIES,
                post_prune_target: POST_PRUNE_TARGET_ENTRIES,
                active_hard_high_water: ACTIVE_HARD_HIGH_WATER_ENTRIES,
            },
        )
    }

    fn open_with_limits(path: &Path, limits: StoreLimits) -> io::Result<Self> {
        let limits = limits.validate()?;
        let mut options = Options::default();
        options.create_if_missing(true);
        let db = DB::open(&options, path).map_err(rocks_error)?;
        let existing_version = match db.get(SCHEMA_KEY).map_err(rocks_error)? {
            Some(value) => {
                let version = decode_u32(&value, "repeat-cache schema version")?;
                if version != 2 && version != STORE_SCHEMA_VERSION {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "unsupported repeat-cache schema version {version}; expected 2 or {STORE_SCHEMA_VERSION}"
                        ),
                    ));
                }
                version
            }
            None => {
                let mut batch = WriteBatch::default();
                batch.put(SCHEMA_KEY, STORE_SCHEMA_VERSION.to_be_bytes());
                batch.put(COUNT_KEY, 0u64.to_be_bytes());
                batch.put(NEXT_SEQUENCE_KEY, 1u64.to_be_bytes());
                batch.put(NEXT_GENERATION_KEY, 1u64.to_be_bytes());
                db.write(batch).map_err(rocks_error)?;
                STORE_SCHEMA_VERSION
            }
        };
        if existing_version == 2 {
            migrate_v2_entries(&db)?;
        }
        let mut cache = Self {
            db,
            limits,
            writes: Mutex::new(()),
            generation: 0,
            generation_completed: AtomicBool::new(false),
        };
        cache.reconcile()?;
        cache.recover_interrupted_generation()?;
        cache.generation = cache.begin_generation()?;
        Ok(cache)
    }

    pub(crate) fn lookup(&self, signature: &CacheSignatureKey) -> io::Result<RepeatCacheLookup> {
        signature.validate()?;
        let entry_key = encode_entry_key(signature)?;
        let Some(value) = self.db.get(entry_key).map_err(rocks_error)? else {
            return Ok(RepeatCacheLookup::Miss);
        };
        let stored = match decode_entry(&value) {
            Ok(stored) => stored,
            Err(error) => return Ok(RepeatCacheLookup::Ineligible(error.to_string())),
        };
        Ok(RepeatCacheLookup::Hit(stored.hashes))
    }

    #[allow(dead_code)]
    pub(crate) fn store(
        &self,
        signature: &CacheSignatureKey,
        hashes: CachedContentHashes,
    ) -> io::Result<RepeatCacheStoreOutcome> {
        let _write = self
            .writes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.store_unlocked(signature, hashes)
    }

    fn store_unlocked(
        &self,
        signature: &CacheSignatureKey,
        hashes: CachedContentHashes,
    ) -> io::Result<RepeatCacheStoreOutcome> {
        signature.validate()?;
        let entry_key = encode_entry_key(signature)?;
        if let Some(value) = self.db.get(&entry_key).map_err(rocks_error)? {
            let stored = decode_entry(&value)?;
            if stored.hashes == hashes {
                return Ok(RepeatCacheStoreOutcome::Replayed);
            }
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "repeat-cache conflict for an identical verified signature",
            ));
        }

        let count = self.read_count()?;
        if count >= self.limits.active_hard_high_water {
            return Err(io::Error::other(
                "repeat-cache active generation reached its hard high-water mark",
            ));
        }
        let sequence = self.read_next_sequence()?;
        let next_sequence = sequence.checked_add(1).ok_or_else(|| {
            io::Error::new(ErrorKind::InvalidData, "repeat-cache sequence exhausted")
        })?;
        let next_count = count.checked_add(1).ok_or_else(|| {
            io::Error::new(ErrorKind::InvalidData, "repeat-cache entry count overflow")
        })?;
        let value = encode_entry(StoredEntry {
            version: STORE_SCHEMA_VERSION,
            sequence,
            generation: self.generation,
            hashes,
        })?;
        let order_key = encode_order_key(sequence, &entry_key)?;
        let last_seen_key = last_seen_key_from_entry_key(&entry_key);
        let mut batch = WriteBatch::default();
        batch.put(&entry_key, value);
        batch.put(order_key, []);
        batch.put(last_seen_key, self.generation.to_be_bytes());
        batch.put(COUNT_KEY, next_count.to_be_bytes());
        batch.put(NEXT_SEQUENCE_KEY, next_sequence.to_be_bytes());
        self.db.write(batch).map_err(rocks_error)?;
        Ok(RepeatCacheStoreOutcome::Stored)
    }

    /// Record that `signature`'s entry was confirmed unchanged in the current generation, so
    /// [`trim_unseen`] doesn't age it out while scans keep verifying it. This is the only write on
    /// the hot cache-hit path in `hasher::xxhash`, which otherwise returns the cached hash without
    /// touching the store at all.
    pub(crate) fn mark_seen(&self, signature: &CacheSignatureKey) -> io::Result<()> {
        let entry_key = encode_entry_key(signature)?;
        let last_seen_key = last_seen_key_from_entry_key(&entry_key);
        self.db
            .put(last_seen_key, self.generation.to_be_bytes())
            .map_err(rocks_error)
    }

    pub(crate) fn store_partial(
        &self,
        signature: &CacheSignatureKey,
        partial_hash: u64,
    ) -> io::Result<RepeatCacheStoreOutcome> {
        let _write = self
            .writes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match self.lookup(signature)? {
            RepeatCacheLookup::Hit(existing) if existing.partial_hash == partial_hash => {
                Ok(RepeatCacheStoreOutcome::Replayed)
            }
            RepeatCacheLookup::Hit(_) => Err(io::Error::new(
                ErrorKind::InvalidData,
                "repeat-cache partial hash conflicts with an identical verified signature",
            )),
            RepeatCacheLookup::Miss => self.store_unlocked(
                signature,
                CachedContentHashes {
                    partial_hash,
                    full_hash: None,
                },
            ),
            RepeatCacheLookup::Ineligible(reason) => Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("repeat-cache entry is ineligible: {reason}"),
            )),
        }
    }

    pub(crate) fn store_full(
        &self,
        signature: &CacheSignatureKey,
        partial_hash: u64,
        full_hash: u64,
    ) -> io::Result<RepeatCacheStoreOutcome> {
        let _write = self
            .writes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match self.lookup(signature)? {
            RepeatCacheLookup::Hit(existing)
                if existing.partial_hash == partial_hash
                    && existing.full_hash == Some(full_hash) =>
            {
                Ok(RepeatCacheStoreOutcome::Replayed)
            }
            RepeatCacheLookup::Hit(existing)
                if existing.partial_hash == partial_hash && existing.full_hash.is_none() =>
            {
                self.replace_hashes(
                    signature,
                    CachedContentHashes {
                        partial_hash,
                        full_hash: Some(full_hash),
                    },
                )
            }
            RepeatCacheLookup::Hit(_) => Err(io::Error::new(
                ErrorKind::InvalidData,
                "repeat-cache full hash conflicts with an identical verified signature",
            )),
            RepeatCacheLookup::Miss => self.store_unlocked(
                signature,
                CachedContentHashes {
                    partial_hash,
                    full_hash: Some(full_hash),
                },
            ),
            RepeatCacheLookup::Ineligible(reason) => Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("repeat-cache entry is ineligible: {reason}"),
            )),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn stats(&self) -> io::Result<RepeatCacheStats> {
        let mut stats = RepeatCacheStats::default();
        for item in self
            .db
            .iterator(IteratorMode::From(ENTRY_PREFIX, Direction::Forward))
        {
            let (key, value) = item.map_err(rocks_error)?;
            if !key.starts_with(ENTRY_PREFIX) {
                break;
            }
            stats.live_entries = stats.live_entries.checked_add(1).ok_or_else(|| {
                io::Error::new(ErrorKind::InvalidData, "repeat-cache stats overflow")
            })?;
            stats.encoded_key_bytes = stats
                .encoded_key_bytes
                .checked_add(key.len() as u64)
                .ok_or_else(|| {
                    io::Error::new(ErrorKind::InvalidData, "cache key bytes overflow")
                })?;
            stats.encoded_value_bytes = stats
                .encoded_value_bytes
                .checked_add(value.len() as u64)
                .ok_or_else(|| {
                io::Error::new(ErrorKind::InvalidData, "cache value bytes overflow")
            })?;
        }
        Ok(stats)
    }

    fn reconcile(&self) -> io::Result<()> {
        let mut count = 0u64;
        let mut maximum_sequence = 0u64;
        let mut repairs = WriteBatch::default();
        let mut repair_count = 0usize;
        for item in self
            .db
            .iterator(IteratorMode::From(ENTRY_PREFIX, Direction::Forward))
        {
            let (key, value) = item.map_err(rocks_error)?;
            if !key.starts_with(ENTRY_PREFIX) {
                break;
            }
            count = count.checked_add(1).ok_or_else(|| {
                io::Error::new(ErrorKind::InvalidData, "repeat-cache entry count overflow")
            })?;
            let sequence = match decode_entry(&value) {
                Ok(stored) => {
                    maximum_sequence = maximum_sequence.max(stored.sequence);
                    stored.sequence
                }
                Err(_) => 0,
            };
            let order_key = encode_order_key(sequence, &key)?;
            if self.db.get(&order_key).map_err(rocks_error)?.is_none() {
                repairs.put(order_key, []);
                repair_count += 1;
            }
            if repair_count == PRUNE_BATCH_ENTRIES {
                self.db.write(repairs).map_err(rocks_error)?;
                repairs = WriteBatch::default();
                repair_count = 0;
            }
        }
        if repair_count != 0 {
            self.db.write(repairs).map_err(rocks_error)?;
        }

        self.remove_orphan_order_keys()?;
        self.remove_orphan_last_seen_keys()?;
        let persisted_next = self.read_next_sequence().unwrap_or(1);
        let next_sequence = persisted_next
            .max(maximum_sequence.saturating_add(1))
            .max(1);
        let mut metadata = WriteBatch::default();
        metadata.put(COUNT_KEY, count.to_be_bytes());
        metadata.put(NEXT_SEQUENCE_KEY, next_sequence.to_be_bytes());
        self.db.write(metadata).map_err(rocks_error)?;
        if count > self.limits.active_hard_high_water {
            self.prune_to_target(self.limits.post_prune_target, None)?;
        }
        Ok(())
    }

    fn begin_generation(&self) -> io::Result<u64> {
        let generation = self.read_next_generation().unwrap_or(1).max(1);
        let next = generation.checked_add(1).ok_or_else(|| {
            io::Error::new(ErrorKind::InvalidData, "repeat-cache generation exhausted")
        })?;
        let mut batch = WriteBatch::default();
        batch.put(ACTIVE_GENERATION_KEY, generation.to_be_bytes());
        batch.put(NEXT_GENERATION_KEY, next.to_be_bytes());
        self.db.write(batch).map_err(rocks_error)?;
        Ok(generation)
    }

    fn recover_interrupted_generation(&self) -> io::Result<()> {
        if self
            .db
            .get(ACTIVE_GENERATION_KEY)
            .map_err(rocks_error)?
            .is_some()
        {
            self.db.delete(ACTIVE_GENERATION_KEY).map_err(rocks_error)?;
        }
        // A cleanly finished generation is already at this target. Repeating the
        // bounded prune also closes the crash window between clearing the active
        // marker and completing finalization without churning entries mid-scan.
        self.prune_to_target(self.limits.post_prune_target, None)?;
        Ok(())
    }

    pub(crate) fn finish_generation(&self) -> io::Result<()> {
        if self.generation_completed.load(Ordering::Acquire) {
            return Ok(());
        }
        let _write = self
            .writes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.generation_completed.load(Ordering::Acquire) {
            return Ok(());
        }
        let active = self
            .db
            .get(ACTIVE_GENERATION_KEY)
            .map_err(rocks_error)?
            .map(|value| decode_u64(&value, "repeat-cache active generation"))
            .transpose()?;
        if active == Some(self.generation) {
            self.db.delete(ACTIVE_GENERATION_KEY).map_err(rocks_error)?;
        }
        self.prune_to_target(self.limits.post_prune_target, None)?;
        self.generation_completed.store(true, Ordering::Release);
        Ok(())
    }

    fn remove_orphan_order_keys(&self) -> io::Result<()> {
        loop {
            let mut deletes = WriteBatch::default();
            let mut delete_count = 0usize;
            for item in self
                .db
                .iterator(IteratorMode::From(ORDER_PREFIX, Direction::Forward))
            {
                let (key, _) = item.map_err(rocks_error)?;
                if !key.starts_with(ORDER_PREFIX) {
                    break;
                }
                let missing = match decode_order_entry_key(&key) {
                    Ok(entry_key) => self.db.get(entry_key).map_err(rocks_error)?.is_none(),
                    Err(_) => true,
                };
                if missing {
                    deletes.delete(key);
                    delete_count += 1;
                    if delete_count == PRUNE_BATCH_ENTRIES {
                        break;
                    }
                }
            }
            if delete_count == 0 {
                return Ok(());
            }
            self.db.write(deletes).map_err(rocks_error)?;
        }
    }

    /// Defensive cleanup for a last-seen key whose entry disappeared (for example through a
    /// process that deletes an entry without going through this crate). Normal entry removal
    /// already deletes both together (`prune_to_target`, `trim_unseen`).
    fn remove_orphan_last_seen_keys(&self) -> io::Result<()> {
        loop {
            let mut deletes = WriteBatch::default();
            let mut delete_count = 0usize;
            for item in self
                .db
                .iterator(IteratorMode::From(LAST_SEEN_PREFIX, Direction::Forward))
            {
                let (key, _) = item.map_err(rocks_error)?;
                if !key.starts_with(LAST_SEEN_PREFIX) {
                    break;
                }
                let entry_key = entry_key_from_last_seen_key(&key);
                if self.db.get(&entry_key).map_err(rocks_error)?.is_none() {
                    deletes.delete(key);
                    delete_count += 1;
                    if delete_count == PRUNE_BATCH_ENTRIES {
                        break;
                    }
                }
            }
            if delete_count == 0 {
                return Ok(());
            }
            self.db.write(deletes).map_err(rocks_error)?;
        }
    }

    fn prune_to_target(&self, target: u64, protected_generation: Option<u64>) -> io::Result<()> {
        let mut count = self.read_count()?;
        while count > target {
            let wanted = (count - target).min(PRUNE_BATCH_ENTRIES as u64) as usize;
            let mut batch = WriteBatch::default();
            let mut removed = 0usize;
            for retain_full in [true, false] {
                for item in self
                    .db
                    .iterator(IteratorMode::From(ORDER_PREFIX, Direction::Forward))
                {
                    let (order_key, _) = item.map_err(rocks_error)?;
                    if !order_key.starts_with(ORDER_PREFIX) || removed == wanted {
                        break;
                    }
                    let entry_key = decode_order_entry_key(&order_key)?;
                    let stored = self
                        .db
                        .get(entry_key)
                        .map_err(rocks_error)?
                        .and_then(|value| decode_entry(&value).ok());
                    if stored
                        .as_ref()
                        .is_some_and(|entry| protected_generation == Some(entry.generation))
                        || stored
                            .as_ref()
                            .is_some_and(|entry| entry.hashes.full_hash.is_some() == retain_full)
                    {
                        continue;
                    }
                    let last_seen_key = last_seen_key_from_entry_key(entry_key);
                    batch.delete(entry_key);
                    batch.delete(order_key);
                    batch.delete(last_seen_key);
                    removed += 1;
                }
                if removed == wanted {
                    break;
                }
            }
            if removed == 0 {
                if protected_generation.is_some() {
                    return Ok(());
                }
                return Err(io::Error::other(
                    "repeat-cache cannot prune below the active-generation protection boundary",
                ));
            }
            count -= removed as u64;
            batch.put(COUNT_KEY, count.to_be_bytes());
            self.db.write(batch).map_err(rocks_error)?;
        }
        Ok(())
    }

    fn read_count(&self) -> io::Result<u64> {
        read_u64_key(&self.db, COUNT_KEY, "repeat-cache entry count")
    }

    fn read_next_sequence(&self) -> io::Result<u64> {
        read_u64_key(&self.db, NEXT_SEQUENCE_KEY, "repeat-cache next sequence")
    }

    fn read_next_generation(&self) -> io::Result<u64> {
        read_u64_key(
            &self.db,
            NEXT_GENERATION_KEY,
            "repeat-cache next generation",
        )
    }

    fn replace_hashes(
        &self,
        signature: &CacheSignatureKey,
        hashes: CachedContentHashes,
    ) -> io::Result<RepeatCacheStoreOutcome> {
        let entry_key = encode_entry_key(signature)?;
        let value = self
            .db
            .get(&entry_key)
            .map_err(rocks_error)?
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::NotFound,
                    "repeat-cache entry disappeared during upgrade",
                )
            })?;
        let mut stored = decode_entry(&value)?;
        stored.hashes = hashes;
        let last_seen_key = last_seen_key_from_entry_key(&entry_key);
        let mut batch = WriteBatch::default();
        batch.put(entry_key, encode_entry(stored)?);
        batch.put(last_seen_key, self.generation.to_be_bytes());
        self.db.write(batch).map_err(rocks_error)?;
        Ok(RepeatCacheStoreOutcome::Stored)
    }
}

impl Drop for RepeatHashCache {
    fn drop(&mut self) {
        if let Err(error) = self.finish_generation() {
            tracing::warn!("Unable to finalize repeat-cache generation: {error}");
        }
    }
}

/// Count live entries through a read-only handle. Read-only opens take no RocksDB lock, so this
/// neither fails against nor blocks a scan that holds the store. A missing store has no entries.
pub(crate) fn count_live_entries(path: &Path) -> io::Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let db = DB::open_for_read_only(&Options::default(), path, false).map_err(rocks_error)?;
    let mut count = 0u64;
    for item in db.iterator(IteratorMode::From(ENTRY_PREFIX, Direction::Forward)) {
        let (key, _) = item.map_err(rocks_error)?;
        if !key.starts_with(ENTRY_PREFIX) {
            break;
        }
        count = count.checked_add(1).ok_or_else(|| {
            io::Error::new(ErrorKind::InvalidData, "repeat-cache entry count overflow")
        })?;
    }
    Ok(count)
}

/// Remove every key, including store metadata and keys written by earlier cache formats; the next
/// [`RepeatHashCache::open`] reinitializes the store. This takes the store lock, so it fails rather
/// than racing while any handle (including a scan in this process) has the store open.
pub(crate) fn clear_store(path: &Path) -> io::Result<()> {
    let mut options = Options::default();
    options.create_if_missing(true);
    let db = DB::open(&options, path).map_err(rocks_error)?;
    let mut batch = WriteBatch::default();
    let mut batch_count = 0usize;
    for item in db.iterator(IteratorMode::Start) {
        let (key, _) = item.map_err(rocks_error)?;
        batch.delete(&key);
        batch_count += 1;
        if batch_count == PRUNE_BATCH_ENTRIES {
            db.write(batch).map_err(rocks_error)?;
            batch = WriteBatch::default();
            batch_count = 0;
        }
    }
    if batch_count != 0 {
        db.write(batch).map_err(rocks_error)?;
    }
    Ok(())
}

/// Remove entries not confirmed unchanged, or created, within the last `max_unseen_generations`
/// generations. One generation is assigned per [`RepeatHashCache::open`], which in this codebase
/// means one per scan (`engine.rs` opens exactly one cache handle per scan and shares it). An entry
/// with no last-seen record yet (written before this trim existed) falls back to the generation it
/// was created in, so it ages out normally once scans resume.
///
/// Opens the store directly rather than through [`RepeatHashCache::open`], so this never assigns a
/// new generation of its own (a trim run isn't a scan) and, like [`clear_store`], fails rather than
/// racing while a scan holds the store open.
pub(crate) fn trim_unseen(
    path: &Path,
    max_unseen_generations: u64,
) -> io::Result<RepeatCacheTrimReport> {
    let mut options = Options::default();
    options.create_if_missing(true);
    let db = DB::open(&options, path).map_err(rocks_error)?;
    let current_generation = match db.get(NEXT_GENERATION_KEY).map_err(rocks_error)? {
        Some(value) => decode_u64(&value, "repeat-cache next generation")?.saturating_sub(1),
        None => 0,
    };

    let mut live_entries_before = 0u64;
    let mut removed = 0u64;
    let mut batch = WriteBatch::default();
    let mut batch_count = 0usize;
    for item in db.iterator(IteratorMode::From(ENTRY_PREFIX, Direction::Forward)) {
        let (key, value) = item.map_err(rocks_error)?;
        if !key.starts_with(ENTRY_PREFIX) {
            break;
        }
        live_entries_before += 1;
        let decoded = decode_entry(&value).ok();
        let last_seen_key = last_seen_key_from_entry_key(&key);
        let last_seen = match db.get(&last_seen_key).map_err(rocks_error)? {
            Some(raw) => decode_u64(&raw, "repeat-cache last-seen generation")?,
            None => decoded.as_ref().map(|entry| entry.generation).unwrap_or(0),
        };
        if current_generation.saturating_sub(last_seen) <= max_unseen_generations {
            continue;
        }
        batch.delete(&key);
        batch.delete(last_seen_key);
        if let Some(entry) = decoded {
            batch.delete(encode_order_key(entry.sequence, &key)?);
        }
        removed += 1;
        batch_count += 1;
        if batch_count == PRUNE_BATCH_ENTRIES {
            db.write(std::mem::take(&mut batch)).map_err(rocks_error)?;
            batch_count = 0;
        }
    }
    if batch_count != 0 {
        db.write(batch).map_err(rocks_error)?;
    }
    if removed > 0 {
        let remaining = live_entries_before.saturating_sub(removed);
        db.put(COUNT_KEY, remaining.to_be_bytes())
            .map_err(rocks_error)?;
    }
    Ok(RepeatCacheTrimReport {
        live_entries_before,
        removed,
    })
}

fn migrate_v2_entries(db: &DB) -> io::Result<()> {
    let mut batch = WriteBatch::default();
    let mut batch_count = 0usize;
    for item in db.iterator(IteratorMode::From(ENTRY_PREFIX, Direction::Forward)) {
        let (key, value) = item.map_err(rocks_error)?;
        if !key.starts_with(ENTRY_PREFIX) {
            break;
        }
        if let Ok((entry, _)) =
            bincode::serde::decode_from_slice::<StoredEntryV2, _>(&value, STORED_ENCODING)
            && entry.version == 2
        {
            batch.put(
                key,
                encode_entry(StoredEntry {
                    version: STORE_SCHEMA_VERSION,
                    sequence: entry.sequence,
                    generation: 0,
                    hashes: entry.hashes,
                })?,
            );
            batch_count += 1;
        }
        if batch_count == PRUNE_BATCH_ENTRIES {
            db.write(batch).map_err(rocks_error)?;
            batch = WriteBatch::default();
            batch_count = 0;
        }
    }
    if batch_count != 0 {
        db.write(batch).map_err(rocks_error)?;
    }
    let mut metadata = WriteBatch::default();
    metadata.put(SCHEMA_KEY, STORE_SCHEMA_VERSION.to_be_bytes());
    if db.get(NEXT_GENERATION_KEY).map_err(rocks_error)?.is_none() {
        metadata.put(NEXT_GENERATION_KEY, 1u64.to_be_bytes());
    }
    db.write(metadata).map_err(rocks_error)
}

fn validate_bounded_text(value: &str, maximum: usize, field: &str) -> io::Result<()> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("{field} must contain 1 to {maximum} non-control UTF-8 bytes"),
        ));
    }
    Ok(())
}

fn encode_entry_key(signature: &CacheSignatureKey) -> io::Result<Vec<u8>> {
    let encoded =
        bincode::serde::encode_to_vec(signature, STORED_ENCODING).map_err(bincode_error)?;
    let mut key = Vec::with_capacity(ENTRY_PREFIX.len() + encoded.len());
    key.extend_from_slice(ENTRY_PREFIX);
    key.extend_from_slice(&encoded);
    if key.len() > MAXIMUM_ENCODED_KEY_BYTES {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "repeat-cache encoded key exceeds its fixed bound",
        ));
    }
    Ok(key)
}

/// `entry_key` must start with `ENTRY_PREFIX` (true of every key produced by
/// [`encode_entry_key`] and every key yielded by an `ENTRY_PREFIX` iteration).
fn last_seen_key_from_entry_key(entry_key: &[u8]) -> Vec<u8> {
    let suffix = &entry_key[ENTRY_PREFIX.len()..];
    let mut key = Vec::with_capacity(LAST_SEEN_PREFIX.len() + suffix.len());
    key.extend_from_slice(LAST_SEEN_PREFIX);
    key.extend_from_slice(suffix);
    key
}

/// Inverse of [`last_seen_key_from_entry_key`]; `last_seen_key` must start with
/// `LAST_SEEN_PREFIX`.
fn entry_key_from_last_seen_key(last_seen_key: &[u8]) -> Vec<u8> {
    let suffix = &last_seen_key[LAST_SEEN_PREFIX.len()..];
    let mut key = Vec::with_capacity(ENTRY_PREFIX.len() + suffix.len());
    key.extend_from_slice(ENTRY_PREFIX);
    key.extend_from_slice(suffix);
    key
}

fn encode_entry(entry: StoredEntry) -> io::Result<Vec<u8>> {
    let encoded = bincode::serde::encode_to_vec(&entry, STORED_ENCODING).map_err(bincode_error)?;
    if encoded.len() > MAXIMUM_ENCODED_VALUE_BYTES {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "repeat-cache encoded value exceeds its fixed bound",
        ));
    }
    Ok(encoded)
}

fn decode_entry(value: &[u8]) -> io::Result<StoredEntry> {
    let (entry, _) = bincode::serde::decode_from_slice::<StoredEntry, _>(value, STORED_ENCODING)
        .map_err(bincode_error)?;
    if entry.version != STORE_SCHEMA_VERSION {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            format!("unsupported repeat-cache entry version {}", entry.version),
        ));
    }
    Ok(entry)
}

fn encode_order_key(sequence: u64, entry_key: &[u8]) -> io::Result<Vec<u8>> {
    if entry_key.len() > u16::MAX as usize {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "repeat-cache entry key is too large for its order index",
        ));
    }
    let mut key = Vec::with_capacity(ORDER_PREFIX.len() + 10 + entry_key.len());
    key.extend_from_slice(ORDER_PREFIX);
    key.extend_from_slice(&sequence.to_be_bytes());
    key.extend_from_slice(&(entry_key.len() as u16).to_be_bytes());
    key.extend_from_slice(entry_key);
    if key.len() > MAXIMUM_ENCODED_ORDER_KEY_BYTES {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "repeat-cache encoded order key exceeds its fixed bound",
        ));
    }
    Ok(key)
}

fn decode_order_entry_key(key: &[u8]) -> io::Result<&[u8]> {
    let header = ORDER_PREFIX.len() + 10;
    if key.len() < header || !key.starts_with(ORDER_PREFIX) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "malformed repeat-cache order key",
        ));
    }
    let length_offset = ORDER_PREFIX.len() + 8;
    let length = u16::from_be_bytes([key[length_offset], key[length_offset + 1]]) as usize;
    if key.len() != header + length {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "malformed repeat-cache order key length",
        ));
    }
    Ok(&key[header..])
}

fn read_u64_key(db: &DB, key: &[u8], field: &str) -> io::Result<u64> {
    let value = db
        .get(key)
        .map_err(rocks_error)?
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, format!("missing {field}")))?;
    decode_u64(&value, field)
}

fn decode_u32(value: &[u8], field: &str) -> io::Result<u32> {
    value
        .try_into()
        .map(u32::from_be_bytes)
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, format!("malformed {field}")))
}

fn decode_u64(value: &[u8], field: &str) -> io::Result<u64> {
    value
        .try_into()
        .map(u64::from_be_bytes)
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, format!("malformed {field}")))
}

fn rocks_error(error: rocksdb::Error) -> io::Error {
    io::Error::other(error)
}

fn bincode_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    struct FakeSignatureProbe {
        observations: Mutex<VecDeque<io::Result<crate::platform::ContentSignatureMetadata>>>,
    }

    impl FakeSignatureProbe {
        fn new(observations: Vec<io::Result<crate::platform::ContentSignatureMetadata>>) -> Self {
            Self {
                observations: Mutex::new(observations.into()),
            }
        }
    }

    impl ContentSignatureProbe for FakeSignatureProbe {
        fn observe(&self, _path: &Path) -> io::Result<crate::platform::ContentSignatureMetadata> {
            self.observations
                .lock()
                .unwrap()
                .pop_front()
                .expect("fake signature observation")
        }
    }

    fn metadata(
        id: &str,
        modified: i64,
        change: &str,
    ) -> crate::platform::ContentSignatureMetadata {
        crate::platform::ContentSignatureMetadata {
            stable_identity: Some(id.to_owned()),
            size: 4096,
            modified_unix_nanos: Some(modified),
            modified_time_is_coarse: false,
            content_change_token: Some(change.to_owned()),
        }
    }

    fn signature(id: usize) -> CacheSignatureKey {
        CacheSignatureKey {
            stable_identity: format!("volume:1:file:{id}"),
            size: 4096 + id as u64,
            modified_unix_nanos: 1_000_000_000 + id as i64,
            content_change_token: format!("change:{id}"),
        }
    }

    /// Entry keys and values are persisted in the hash cache and are looked up by exact bytes, so
    /// an encoder change orphans every stored entry instead of failing. Pin the encoding.
    #[test]
    fn stored_encoding_bytes_are_pinned() {
        let pinned_signature = CacheSignatureKey {
            stable_identity: "volume:1:file:2".to_owned(),
            size: 4096,
            modified_unix_nanos: 1_700_000_000_000_000_000,
            content_change_token: "change:7".to_owned(),
        };
        let key = encode_entry_key(&pinned_signature).unwrap();
        assert_eq!(key, PINNED_ENTRY_KEY);

        let value = encode_entry(StoredEntry {
            version: STORE_SCHEMA_VERSION,
            sequence: 9,
            generation: 4,
            hashes: CachedContentHashes {
                partial_hash: 0x0102_0304_0506_0708,
                full_hash: Some(0x1112_1314_1516_1718),
            },
        })
        .unwrap();
        assert_eq!(value, PINNED_ENTRY_VALUE);
        let decoded = decode_entry(PINNED_ENTRY_VALUE).unwrap();
        assert_eq!(decoded.sequence, 9);
        assert_eq!(decoded.generation, 4);
        assert_eq!(decoded.hashes.full_hash, Some(0x1112_1314_1516_1718));

        // A v2 entry written by an earlier release must still migrate.
        let legacy = bincode::serde::encode_to_vec(
            &StoredEntryV2 {
                version: 2,
                sequence: 3,
                hashes: CachedContentHashes {
                    partial_hash: 5,
                    full_hash: None,
                },
            },
            STORED_ENCODING,
        )
        .unwrap();
        assert_eq!(legacy, PINNED_V2_ENTRY_VALUE);
    }

    const PINNED_ENTRY_KEY: &[u8] = &[
        0, 115, 117, 112, 101, 114, 45, 100, 117, 112, 101, 114, 47, 114, 101, 112, 101, 97, 116,
        45, 99, 97, 99, 104, 101, 47, 101, 110, 116, 114, 121, 47, 15, 0, 0, 0, 0, 0, 0, 0, 118,
        111, 108, 117, 109, 101, 58, 49, 58, 102, 105, 108, 101, 58, 50, 0, 16, 0, 0, 0, 0, 0, 0,
        0, 0, 42, 54, 254, 156, 151, 23, 8, 0, 0, 0, 0, 0, 0, 0, 99, 104, 97, 110, 103, 101, 58,
        55,
    ];
    const PINNED_ENTRY_VALUE: &[u8] = &[
        3, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 1, 24,
        23, 22, 21, 20, 19, 18, 17,
    ];
    const PINNED_V2_ENTRY_VALUE: &[u8] = &[
        2, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    fn hashes(id: usize) -> CachedContentHashes {
        CachedContentHashes {
            partial_hash: id as u64 + 10,
            full_hash: id.is_multiple_of(2).then_some(id as u64 + 100),
        }
    }

    #[test]
    fn signature_window_accepts_path_aliases_only_when_all_content_evidence_matches() {
        let unchanged = metadata("volume:1:file:7", 1_234_567_890, "change:9");
        let probe = FakeSignatureProbe::new(vec![Ok(unchanged.clone()), Ok(unchanged)]);
        let before = observe_content_signature(Path::new("first-name"), &probe);
        let after = observe_content_signature(Path::new("renamed-or-linked-alias"), &probe);
        assert_eq!(
            compare_content_signatures(before, after),
            ContentSignatureWindow::Unchanged(CacheSignatureKey {
                stable_identity: "volume:1:file:7".to_owned(),
                size: 4096,
                modified_unix_nanos: 1_234_567_890,
                content_change_token: "change:9".to_owned(),
            })
        );
    }

    #[test]
    fn signature_window_rejects_preserved_modified_time_content_edits_and_identity_reuse() {
        let preserved_modified = 1_234_567_890;
        let changed_content = compare_content_signatures(
            ContentSignatureObservation::Qualified(CacheSignatureKey {
                stable_identity: "volume:1:file:7".to_owned(),
                size: 4096,
                modified_unix_nanos: preserved_modified,
                content_change_token: "change:9".to_owned(),
            }),
            ContentSignatureObservation::Qualified(CacheSignatureKey {
                stable_identity: "volume:1:file:7".to_owned(),
                size: 4096,
                modified_unix_nanos: preserved_modified,
                content_change_token: "change:10".to_owned(),
            }),
        );
        assert_eq!(changed_content, ContentSignatureWindow::Changed);

        let reused_identity = compare_content_signatures(
            ContentSignatureObservation::Qualified(CacheSignatureKey {
                stable_identity: "volume:1:file:7".to_owned(),
                size: 4096,
                modified_unix_nanos: preserved_modified,
                content_change_token: "change:9".to_owned(),
            }),
            ContentSignatureObservation::Qualified(CacheSignatureKey {
                stable_identity: "volume:1:file:8".to_owned(),
                size: 4096,
                modified_unix_nanos: preserved_modified,
                content_change_token: "change:9".to_owned(),
            }),
        );
        assert_eq!(reused_identity, ContentSignatureWindow::Changed);
    }

    #[test]
    fn signature_qualification_fails_closed_for_every_unavailable_or_coarse_field() {
        let mut cases = Vec::new();
        cases.push((
            Err(io::Error::new(ErrorKind::PermissionDenied, "blocked")),
            SignatureIneligibleReason::MetadataUnavailable,
        ));
        let mut missing_identity = metadata("id", 1_234_567_890, "change");
        missing_identity.stable_identity = None;
        cases.push((
            Ok(missing_identity),
            SignatureIneligibleReason::StableIdentityUnavailable,
        ));
        let mut missing_modified = metadata("id", 1_234_567_890, "change");
        missing_modified.modified_unix_nanos = None;
        cases.push((
            Ok(missing_modified),
            SignatureIneligibleReason::ModifiedTimeUnavailable,
        ));
        let mut coarse = metadata("id", 1_000_000_000, "change");
        coarse.modified_time_is_coarse = true;
        cases.push((Ok(coarse), SignatureIneligibleReason::CoarseModifiedTime));
        let mut missing_change = metadata("id", 1_234_567_890, "change");
        missing_change.content_change_token = None;
        cases.push((
            Ok(missing_change),
            SignatureIneligibleReason::ContentChangeTokenUnavailable,
        ));
        let mut invalid = metadata("id", 1_234_567_890, "change");
        invalid.stable_identity = Some("\0".to_owned());
        cases.push((Ok(invalid), SignatureIneligibleReason::InvalidSignature));

        for (observation, expected) in cases {
            let probe = FakeSignatureProbe::new(vec![observation]);
            assert_eq!(
                observe_content_signature(Path::new("candidate"), &probe),
                ContentSignatureObservation::Ineligible(expected),
                "{}",
                expected.as_str()
            );
        }
    }

    #[test]
    fn policy_contract_is_closed_and_defaults_to_measured_verified_reuse() {
        assert_eq!(
            RepeatCachePolicy::default(),
            RepeatCachePolicy::ReuseVerified
        );
        for policy in [
            RepeatCachePolicy::ReuseVerified,
            RepeatCachePolicy::RevalidateContent,
        ] {
            assert_eq!(
                policy.as_str().parse::<RepeatCachePolicy>().unwrap(),
                policy
            );
            assert_eq!(
                serde_json::to_string(&policy).unwrap(),
                format!("\"{}\"", policy.as_str())
            );
        }
        assert_eq!(
            "path_size_time"
                .parse::<RepeatCachePolicy>()
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidInput
        );
    }

    #[test]
    fn store_reopens_replays_and_rejects_conflicting_verified_content() {
        let temp = TempDir::new().unwrap();
        let key = signature(1);
        let expected = hashes(1);
        {
            let cache = RepeatHashCache::open(temp.path()).unwrap();
            assert_eq!(cache.lookup(&key).unwrap(), RepeatCacheLookup::Miss);
            assert_eq!(
                cache.store(&key, expected).unwrap(),
                RepeatCacheStoreOutcome::Stored
            );
            assert_eq!(
                cache.store(&key, expected).unwrap(),
                RepeatCacheStoreOutcome::Replayed
            );
        }
        let cache = RepeatHashCache::open(temp.path()).unwrap();
        assert_eq!(
            cache.lookup(&key).unwrap(),
            RepeatCacheLookup::Hit(expected)
        );
        let conflict = CachedContentHashes {
            partial_hash: expected.partial_hash + 1,
            full_hash: expected.full_hash,
        };
        assert_eq!(
            cache.store(&key, conflict).unwrap_err().kind(),
            ErrorKind::InvalidData
        );
        assert_eq!(cache.stats().unwrap().live_entries, 1);
    }

    #[test]
    fn concurrent_pipeline_stores_preserve_exact_count_and_unique_order() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(RepeatHashCache::open(temp.path()).unwrap());
        let threads = (0..32)
            .map(|id| {
                let cache = cache.clone();
                std::thread::spawn(move || cache.store(&signature(id), hashes(id)).unwrap())
            })
            .collect::<Vec<_>>();
        for thread in threads {
            assert_eq!(thread.join().unwrap(), RepeatCacheStoreOutcome::Stored);
        }
        assert_eq!(cache.stats().unwrap().live_entries, 32);
        assert_eq!(cache.read_count().unwrap(), 32);
        let mut order_sequences = cache
            .db
            .iterator(IteratorMode::From(ORDER_PREFIX, Direction::Forward))
            .take_while(|item| {
                item.as_ref()
                    .is_ok_and(|(key, _)| key.starts_with(ORDER_PREFIX))
            })
            .map(|item| {
                let (key, _) = item.unwrap();
                u64::from_be_bytes(
                    key[ORDER_PREFIX.len()..ORDER_PREFIX.len() + 8]
                        .try_into()
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        order_sequences.sort_unstable();
        order_sequences.dedup();
        assert_eq!(order_sequences.len(), 32);
    }

    #[test]
    fn store_rejects_unbounded_signatures_and_newer_schema_without_modification() {
        let temp = TempDir::new().unwrap();
        let cache = RepeatHashCache::open(temp.path()).unwrap();
        let mut oversized = signature(1);
        oversized.stable_identity = "x".repeat(MAXIMUM_STABLE_IDENTITY_BYTES + 1);
        assert_eq!(
            cache.store(&oversized, hashes(1)).unwrap_err().kind(),
            ErrorKind::InvalidInput
        );
        drop(cache);

        let mut options = Options::default();
        options.create_if_missing(true);
        let db = DB::open(&options, temp.path()).unwrap();
        db.put(SCHEMA_KEY, (STORE_SCHEMA_VERSION + 1).to_be_bytes())
            .unwrap();
        drop(db);
        let error = match RepeatHashCache::open(temp.path()) {
            Ok(_) => panic!("newer repeat-cache schema must be rejected"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), ErrorKind::InvalidData);
        let db = DB::open(&options, temp.path()).unwrap();
        assert_eq!(
            decode_u32(&db.get(SCHEMA_KEY).unwrap().unwrap(), "schema").unwrap(),
            STORE_SCHEMA_VERSION + 1
        );
    }

    #[test]
    fn corrupt_entries_are_ineligible_and_legacy_entries_are_ignored() {
        let temp = TempDir::new().unwrap();
        let cache = RepeatHashCache::open(temp.path()).unwrap();
        let key = signature(2);
        let encoded_key = encode_entry_key(&key).unwrap();
        cache.db.put(encoded_key, b"not-bincode").unwrap();
        cache
            .db
            .put(b"legacy|path|4096|1.0", 42u64.to_be_bytes())
            .unwrap();
        assert!(matches!(
            cache.lookup(&key).unwrap(),
            RepeatCacheLookup::Ineligible(_)
        ));
        assert_eq!(cache.stats().unwrap().live_entries, 1);
        assert_eq!(
            cache
                .db
                .get(b"legacy|path|4096|1.0")
                .unwrap()
                .unwrap()
                .len(),
            8
        );
    }

    #[test]
    fn corrupt_entries_remain_bounded_and_are_pruned_before_valid_entries() {
        let temp = TempDir::new().unwrap();
        let corrupt = signature(0);
        {
            let cache = RepeatHashCache::open(temp.path()).unwrap();
            cache
                .db
                .put(encode_entry_key(&corrupt).unwrap(), b"not-bincode")
                .unwrap();
            cache
                .db
                .put([ORDER_PREFIX, b"malformed"].concat(), [])
                .unwrap();
        }
        let cache = RepeatHashCache::open_with_limits(
            temp.path(),
            StoreLimits {
                normal_live_target: 3,
                post_prune_target: 2,
                active_hard_high_water: 4,
            },
        )
        .unwrap();
        assert!(matches!(
            cache.lookup(&corrupt).unwrap(),
            RepeatCacheLookup::Ineligible(_)
        ));
        cache.store(&signature(1), hashes(1)).unwrap();
        cache.store(&signature(2), hashes(2)).unwrap();
        cache.finish_generation().unwrap();
        assert_eq!(cache.lookup(&corrupt).unwrap(), RepeatCacheLookup::Miss);
        assert_eq!(
            cache.lookup(&signature(1)).unwrap(),
            RepeatCacheLookup::Hit(hashes(1))
        );
        assert_eq!(
            cache.lookup(&signature(2)).unwrap(),
            RepeatCacheLookup::Hit(hashes(2))
        );
        assert_eq!(cache.stats().unwrap().live_entries, 2);
    }

    #[test]
    fn interrupted_metadata_and_order_state_reconciles_on_reopen() {
        let temp = TempDir::new().unwrap();
        let key = signature(3);
        let encoded_key = encode_entry_key(&key).unwrap();
        {
            let cache = RepeatHashCache::open(temp.path()).unwrap();
            let value = encode_entry(StoredEntry {
                version: STORE_SCHEMA_VERSION,
                sequence: 41,
                generation: 0,
                hashes: hashes(3),
            })
            .unwrap();
            cache.db.put(&encoded_key, value).unwrap();
            cache.db.put(COUNT_KEY, 0u64.to_be_bytes()).unwrap();
            cache.db.put(NEXT_SEQUENCE_KEY, 1u64.to_be_bytes()).unwrap();
            cache
                .db
                .put(encode_order_key(5, b"missing-entry").unwrap(), [])
                .unwrap();
        }
        let cache = RepeatHashCache::open(temp.path()).unwrap();
        assert_eq!(
            cache.lookup(&key).unwrap(),
            RepeatCacheLookup::Hit(hashes(3))
        );
        assert_eq!(cache.read_count().unwrap(), 1);
        assert_eq!(cache.read_next_sequence().unwrap(), 42);
        assert!(
            cache
                .db
                .get(encode_order_key(41, &encoded_key).unwrap())
                .unwrap()
                .is_some()
        );
        assert!(
            cache
                .db
                .get(encode_order_key(5, b"missing-entry").unwrap())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn deterministic_pruning_keeps_newest_entries_with_fixed_encoded_bounds() {
        let temp = TempDir::new().unwrap();
        let cache = RepeatHashCache::open_with_limits(
            temp.path(),
            StoreLimits {
                normal_live_target: 5,
                post_prune_target: 3,
                active_hard_high_water: 7,
            },
        )
        .unwrap();
        for id in 0..6 {
            cache.store(&signature(id), hashes(id)).unwrap();
        }
        cache.finish_generation().unwrap();
        let stats = cache.stats().unwrap();
        assert_eq!(stats.live_entries, 3);
        assert!(stats.encoded_key_bytes <= stats.live_entries * MAXIMUM_ENCODED_KEY_BYTES as u64);
        assert!(
            stats.encoded_value_bytes <= stats.live_entries * MAXIMUM_ENCODED_VALUE_BYTES as u64
        );
        for id in 0..6 {
            let expected = if id % 2 == 0 {
                RepeatCacheLookup::Hit(hashes(id))
            } else {
                RepeatCacheLookup::Miss
            };
            assert_eq!(cache.lookup(&signature(id)).unwrap(), expected);
        }
    }

    #[test]
    fn schema_two_entries_migrate_once_as_completed_generation() {
        let temp = TempDir::new().unwrap();
        let key = signature(41);
        let encoded_key = encode_entry_key(&key).unwrap();
        let mut options = Options::default();
        options.create_if_missing(true);
        {
            let db = DB::open(&options, temp.path()).unwrap();
            db.put(SCHEMA_KEY, 2u32.to_be_bytes()).unwrap();
            db.put(COUNT_KEY, 1u64.to_be_bytes()).unwrap();
            db.put(NEXT_SEQUENCE_KEY, 2u64.to_be_bytes()).unwrap();
            db.put(
                &encoded_key,
                bincode::serde::encode_to_vec(
                    &StoredEntryV2 {
                        version: 2,
                        sequence: 1,
                        hashes: hashes(41),
                    },
                    STORED_ENCODING,
                )
                .unwrap(),
            )
            .unwrap();
            db.put(encode_order_key(1, &encoded_key).unwrap(), [])
                .unwrap();
        }

        let cache = RepeatHashCache::open(temp.path()).unwrap();
        assert_eq!(
            cache.lookup(&key).unwrap(),
            RepeatCacheLookup::Hit(hashes(41))
        );
        let stored = decode_entry(&cache.db.get(&encoded_key).unwrap().unwrap()).unwrap();
        assert_eq!(stored.generation, 0);
        assert_eq!(
            decode_u32(&cache.db.get(SCHEMA_KEY).unwrap().unwrap(), "schema").unwrap(),
            STORE_SCHEMA_VERSION
        );
        drop(cache);

        let reopened = RepeatHashCache::open(temp.path()).unwrap();
        assert_eq!(
            reopened.lookup(&key).unwrap(),
            RepeatCacheLookup::Hit(hashes(41))
        );
        assert_eq!(reopened.stats().unwrap().live_entries, 1);
    }

    #[test]
    fn mark_seen_updates_last_seen_generation_for_confirmed_hits() {
        let temp = TempDir::new().unwrap();
        let key = signature(7);
        {
            let cache = RepeatHashCache::open(temp.path()).unwrap();
            cache.store(&key, hashes(7)).unwrap();
        }
        let entry_key = encode_entry_key(&key).unwrap();
        let last_seen_key = last_seen_key_from_entry_key(&entry_key);
        let read_last_seen = || {
            let db = DB::open_for_read_only(&Options::default(), temp.path(), false).unwrap();
            decode_u64(&db.get(&last_seen_key).unwrap().unwrap(), "last seen").unwrap()
        };
        assert_eq!(read_last_seen(), 1);

        {
            let cache = RepeatHashCache::open(temp.path()).unwrap();
            assert_eq!(cache.generation, 2);
            cache.mark_seen(&key).unwrap();
        }
        assert_eq!(read_last_seen(), 2);
    }

    /// Exercises the trim boundary directly: an entry re-marked seen in a later generation
    /// survives a bound that removes an entry never touched again after its creation generation,
    /// and a scan after the trim (a fresh store, plus lookups) still verifies correctly.
    #[test]
    fn trim_unseen_removes_only_entries_past_the_bound_and_a_scan_still_verifies_after() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        // Generation 1: three entries created, none touched again.
        {
            let cache = RepeatHashCache::open(path).unwrap();
            for id in 0..3 {
                cache.store(&signature(id), hashes(id)).unwrap();
            }
        }
        // Generation 2: entry 0 is re-confirmed (as a real hit would), entry 3 is created fresh.
        {
            let cache = RepeatHashCache::open(path).unwrap();
            cache.mark_seen(&signature(0)).unwrap();
            cache.store(&signature(3), hashes(3)).unwrap();
        }
        // Generation 3: no activity, just advances the current-generation reference point.
        {
            RepeatHashCache::open(path).unwrap();
        }

        // current generation is 3; entries 1 and 2 (last seen at generation 1) are 2 generations
        // stale, entries 0 and 3 (last seen at generation 2) are only 1 generation stale.
        let report = trim_unseen(path, 1).unwrap();
        assert_eq!(report.live_entries_before, 4);
        assert_eq!(report.removed, 2);

        let cache = RepeatHashCache::open(path).unwrap();
        assert_eq!(
            cache.lookup(&signature(0)).unwrap(),
            RepeatCacheLookup::Hit(hashes(0))
        );
        assert_eq!(
            cache.lookup(&signature(1)).unwrap(),
            RepeatCacheLookup::Miss
        );
        assert_eq!(
            cache.lookup(&signature(2)).unwrap(),
            RepeatCacheLookup::Miss
        );
        assert_eq!(
            cache.lookup(&signature(3)).unwrap(),
            RepeatCacheLookup::Hit(hashes(3))
        );
        assert_eq!(cache.stats().unwrap().live_entries, 2);
        assert_eq!(cache.read_count().unwrap(), 2);

        // A scan after the trim still stores and verifies correctly.
        cache.store(&signature(4), hashes(4)).unwrap();
        assert_eq!(
            cache.lookup(&signature(4)).unwrap(),
            RepeatCacheLookup::Hit(hashes(4))
        );
        assert_eq!(cache.read_count().unwrap(), 3);
    }

    /// A store written before last-seen tracking existed has entries but no last-seen keys; trim
    /// must still open it and fall back to each entry's creation generation.
    #[test]
    fn trim_unseen_opens_and_trims_a_store_predating_last_seen_tracking() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();
        {
            let cache = RepeatHashCache::open(path).unwrap();
            cache.store(&signature(0), hashes(0)).unwrap();
            cache.store(&signature(1), hashes(1)).unwrap();
            // Simulate entries written before this trim existed: no last-seen key at all.
            let key0 = encode_entry_key(&signature(0)).unwrap();
            let key1 = encode_entry_key(&signature(1)).unwrap();
            cache
                .db
                .delete(last_seen_key_from_entry_key(&key0))
                .unwrap();
            cache
                .db
                .delete(last_seen_key_from_entry_key(&key1))
                .unwrap();
        }
        // Advance two more generations without touching either entry.
        for _ in 0..2 {
            RepeatHashCache::open(path).unwrap();
        }

        let report = trim_unseen(path, 1).unwrap();
        assert_eq!(report.live_entries_before, 2);
        assert_eq!(report.removed, 2);

        let cache = RepeatHashCache::open(path).unwrap();
        assert_eq!(
            cache.lookup(&signature(0)).unwrap(),
            RepeatCacheLookup::Miss
        );
        assert_eq!(
            cache.lookup(&signature(1)).unwrap(),
            RepeatCacheLookup::Miss
        );
        assert_eq!(cache.stats().unwrap().live_entries, 0);
    }

    #[test]
    fn trim_unseen_fails_while_a_scan_holds_the_store() {
        let temp = TempDir::new().unwrap();
        let cache = RepeatHashCache::open(temp.path()).unwrap();
        cache.store(&signature(0), hashes(0)).unwrap();
        assert!(trim_unseen(temp.path(), 0).is_err());
        drop(cache);
        assert!(trim_unseen(temp.path(), 0).is_ok());
    }

    #[test]
    fn active_generation_is_protected_until_hard_high_water_and_full_hashes_win_pruning() {
        let temp = TempDir::new().unwrap();
        let limits = StoreLimits {
            normal_live_target: 3,
            post_prune_target: 2,
            active_hard_high_water: 5,
        };
        {
            let cache = RepeatHashCache::open_with_limits(temp.path(), limits).unwrap();
            for id in 0..5 {
                cache.store(&signature(id), hashes(id)).unwrap();
            }
            let error = cache.store(&signature(5), hashes(5)).unwrap_err();
            assert!(error.to_string().contains("hard high-water"));
            assert_eq!(cache.stats().unwrap().live_entries, 5);
            cache.finish_generation().unwrap();
            assert_eq!(cache.stats().unwrap().live_entries, 2);
            assert_eq!(
                cache.lookup(&signature(2)).unwrap(),
                RepeatCacheLookup::Hit(hashes(2))
            );
            assert_eq!(
                cache.lookup(&signature(4)).unwrap(),
                RepeatCacheLookup::Hit(hashes(4))
            );
        }

        let cache = RepeatHashCache::open_with_limits(temp.path(), limits).unwrap();
        for id in 5..8 {
            cache.store(&signature(id), hashes(id)).unwrap();
        }
        for id in 5..8 {
            assert_eq!(
                cache.lookup(&signature(id)).unwrap(),
                RepeatCacheLookup::Hit(hashes(id))
            );
        }
        assert_eq!(
            cache.lookup(&signature(2)).unwrap(),
            RepeatCacheLookup::Hit(hashes(2))
        );
        assert_eq!(
            cache.lookup(&signature(4)).unwrap(),
            RepeatCacheLookup::Hit(hashes(4))
        );
        cache.finish_generation().unwrap();
        assert_eq!(cache.stats().unwrap().live_entries, 2);
        assert_eq!(
            cache.lookup(&signature(6)).unwrap(),
            RepeatCacheLookup::Hit(hashes(6))
        );
        assert_eq!(
            cache.lookup(&signature(4)).unwrap(),
            RepeatCacheLookup::Hit(hashes(4))
        );
    }

    #[test]
    #[ignore = "SOP10d generated-store scale fixture"]
    fn generated_store_above_legacy_cap_retains_early_and_late_hits_after_reopen() {
        const ENTRY_COUNT: usize = 1_500_002;

        let temp = TempDir::new().unwrap();
        let cache = RepeatHashCache::open(temp.path()).unwrap();
        let mut batch = WriteBatch::default();
        let mut batch_count = 0usize;
        for id in 0..ENTRY_COUNT {
            let signature = signature(id);
            let entry_key = encode_entry_key(&signature).unwrap();
            let sequence = id as u64 + 1;
            batch.put(
                &entry_key,
                encode_entry(StoredEntry {
                    version: STORE_SCHEMA_VERSION,
                    sequence,
                    generation: cache.generation,
                    hashes: hashes(id),
                })
                .unwrap(),
            );
            batch.put(encode_order_key(sequence, &entry_key).unwrap(), []);
            batch_count += 1;
            if batch_count == PRUNE_BATCH_ENTRIES {
                cache.db.write(batch).unwrap();
                batch = WriteBatch::default();
                batch_count = 0;
            }
        }
        if batch_count != 0 {
            cache.db.write(batch).unwrap();
        }
        let mut metadata = WriteBatch::default();
        metadata.put(COUNT_KEY, (ENTRY_COUNT as u64).to_be_bytes());
        metadata.put(NEXT_SEQUENCE_KEY, (ENTRY_COUNT as u64 + 1).to_be_bytes());
        cache.db.write(metadata).unwrap();
        cache.finish_generation().unwrap();
        drop(cache);

        let reopened = RepeatHashCache::open(temp.path()).unwrap();
        assert_eq!(
            reopened.lookup(&signature(0)).unwrap(),
            RepeatCacheLookup::Hit(hashes(0))
        );
        assert_eq!(
            reopened.lookup(&signature(ENTRY_COUNT - 1)).unwrap(),
            RepeatCacheLookup::Hit(hashes(ENTRY_COUNT - 1))
        );
        let stats = reopened.stats().unwrap();
        assert_eq!(stats.live_entries, ENTRY_COUNT as u64);
        assert!(stats.encoded_key_bytes <= stats.live_entries * MAXIMUM_ENCODED_KEY_BYTES as u64);
        assert!(
            stats.encoded_value_bytes <= stats.live_entries * MAXIMUM_ENCODED_VALUE_BYTES as u64
        );
    }
}
