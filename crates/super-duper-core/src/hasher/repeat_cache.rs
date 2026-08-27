use rocksdb::{Direction, IteratorMode, Options, WriteBatch, DB};
use serde::{Deserialize, Serialize};
use std::io::{self, ErrorKind};
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;

pub(crate) const STORE_SCHEMA_VERSION: u32 = 2;
const SCHEMA_KEY: &[u8] = b"\0super-duper/repeat-cache/schema";
const COUNT_KEY: &[u8] = b"\0super-duper/repeat-cache/count";
const NEXT_SEQUENCE_KEY: &[u8] = b"\0super-duper/repeat-cache/next-sequence";
const ENTRY_PREFIX: &[u8] = b"\0super-duper/repeat-cache/entry/";
const ORDER_PREFIX: &[u8] = b"\0super-duper/repeat-cache/order/";

pub(crate) const MAXIMUM_LIVE_ENTRIES: u64 = 1_500_000;
pub(crate) const PRUNE_TARGET_ENTRIES: u64 = 1_350_000;
pub(crate) const MAXIMUM_STABLE_IDENTITY_BYTES: usize = 512;
pub(crate) const MAXIMUM_CHANGE_TOKEN_BYTES: usize = 256;
pub(crate) const MAXIMUM_ENCODED_KEY_BYTES: usize = 1024;
pub(crate) const MAXIMUM_ENCODED_VALUE_BYTES: usize = 128;
pub(crate) const MAXIMUM_ENCODED_ORDER_KEY_BYTES: usize =
    ORDER_PREFIX.len() + 10 + MAXIMUM_ENCODED_KEY_BYTES;
const PRUNE_BATCH_ENTRIES: usize = 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatCachePolicy {
    ReuseVerified,
    #[default]
    RevalidateContent,
}

impl RepeatCachePolicy {
    #[allow(dead_code)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReuseVerified => "reuse_verified",
            Self::RevalidateContent => "revalidate_content",
        }
    }
}

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
            )
        }
    };
    let stable_identity = match metadata.stable_identity {
        Some(value) => value,
        None => {
            return ContentSignatureObservation::Ineligible(
                SignatureIneligibleReason::StableIdentityUnavailable,
            )
        }
    };
    let modified_unix_nanos = match metadata.modified_unix_nanos {
        Some(value) if value > 0 => value,
        _ => {
            return ContentSignatureObservation::Ineligible(
                SignatureIneligibleReason::ModifiedTimeUnavailable,
            )
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
            )
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

#[derive(Debug, Serialize, Deserialize)]
struct StoredEntry {
    version: u32,
    sequence: u64,
    hashes: CachedContentHashes,
}

#[derive(Debug, Clone, Copy)]
struct StoreLimits {
    maximum_entries: u64,
    prune_target: u64,
}

impl StoreLimits {
    fn validate(self) -> io::Result<Self> {
        if self.maximum_entries == 0 || self.prune_target >= self.maximum_entries {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "repeat-cache limits require 0 < prune target < maximum entries",
            ));
        }
        Ok(self)
    }
}

pub(crate) struct RepeatHashCache {
    db: DB,
    limits: StoreLimits,
    writes: Mutex<()>,
}

impl RepeatHashCache {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        Self::open_with_limits(
            path,
            StoreLimits {
                maximum_entries: MAXIMUM_LIVE_ENTRIES,
                prune_target: PRUNE_TARGET_ENTRIES,
            },
        )
    }

    fn open_with_limits(path: &Path, limits: StoreLimits) -> io::Result<Self> {
        let limits = limits.validate()?;
        let mut options = Options::default();
        options.create_if_missing(true);
        let db = DB::open(&options, path).map_err(rocks_error)?;
        match db.get(SCHEMA_KEY).map_err(rocks_error)? {
            Some(value) => {
                let version = decode_u32(&value, "repeat-cache schema version")?;
                if version != STORE_SCHEMA_VERSION {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "unsupported repeat-cache schema version {version}; expected {STORE_SCHEMA_VERSION}"
                        ),
                    ));
                }
            }
            None => {
                let mut batch = WriteBatch::default();
                batch.put(SCHEMA_KEY, STORE_SCHEMA_VERSION.to_be_bytes());
                batch.put(COUNT_KEY, 0u64.to_be_bytes());
                batch.put(NEXT_SEQUENCE_KEY, 1u64.to_be_bytes());
                db.write(batch).map_err(rocks_error)?;
            }
        }
        let cache = Self {
            db,
            limits,
            writes: Mutex::new(()),
        };
        cache.reconcile()?;
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

        let mut count = self.read_count()?;
        if count >= self.limits.maximum_entries {
            self.prune_to_target()?;
            count = self.read_count()?;
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
            hashes,
        })?;
        let order_key = encode_order_key(sequence, &entry_key)?;
        let mut batch = WriteBatch::default();
        batch.put(&entry_key, value);
        batch.put(order_key, []);
        batch.put(COUNT_KEY, next_count.to_be_bytes());
        batch.put(NEXT_SEQUENCE_KEY, next_sequence.to_be_bytes());
        self.db.write(batch).map_err(rocks_error)?;
        Ok(RepeatCacheStoreOutcome::Stored)
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
        let persisted_next = self.read_next_sequence().unwrap_or(1);
        let next_sequence = persisted_next
            .max(maximum_sequence.saturating_add(1))
            .max(1);
        let mut metadata = WriteBatch::default();
        metadata.put(COUNT_KEY, count.to_be_bytes());
        metadata.put(NEXT_SEQUENCE_KEY, next_sequence.to_be_bytes());
        self.db.write(metadata).map_err(rocks_error)?;
        if count > self.limits.maximum_entries {
            self.prune_to_target()?;
        }
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

    fn prune_to_target(&self) -> io::Result<()> {
        let mut count = self.read_count()?;
        while count > self.limits.prune_target {
            let wanted =
                (count - self.limits.prune_target).min(PRUNE_BATCH_ENTRIES as u64) as usize;
            let mut batch = WriteBatch::default();
            let mut removed = 0usize;
            for item in self
                .db
                .iterator(IteratorMode::From(ORDER_PREFIX, Direction::Forward))
            {
                let (order_key, _) = item.map_err(rocks_error)?;
                if !order_key.starts_with(ORDER_PREFIX) || removed == wanted {
                    break;
                }
                let entry_key = decode_order_entry_key(&order_key)?;
                batch.delete(entry_key);
                batch.delete(order_key);
                removed += 1;
            }
            if removed == 0 {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "repeat-cache order index cannot satisfy bounded pruning",
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
        self.db
            .put(entry_key, encode_entry(stored)?)
            .map_err(rocks_error)?;
        Ok(RepeatCacheStoreOutcome::Stored)
    }
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
    let encoded = bincode::serialize(signature).map_err(bincode_error)?;
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

fn encode_entry(entry: StoredEntry) -> io::Result<Vec<u8>> {
    let encoded = bincode::serialize(&entry).map_err(bincode_error)?;
    if encoded.len() > MAXIMUM_ENCODED_VALUE_BYTES {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "repeat-cache encoded value exceeds its fixed bound",
        ));
    }
    Ok(encoded)
}

fn decode_entry(value: &[u8]) -> io::Result<StoredEntry> {
    let entry: StoredEntry = bincode::deserialize(value).map_err(bincode_error)?;
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
    io::Error::new(ErrorKind::Other, error)
}

fn bincode_error(error: bincode::Error) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, error)
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

    fn hashes(id: usize) -> CachedContentHashes {
        CachedContentHashes {
            partial_hash: id as u64 + 10,
            full_hash: (id % 2 == 0).then_some(id as u64 + 100),
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
    fn policy_contract_is_closed_and_defaults_to_forced_revalidation() {
        assert_eq!(
            RepeatCachePolicy::default(),
            RepeatCachePolicy::RevalidateContent
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

        let options = Options::default();
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
                maximum_entries: 2,
                prune_target: 1,
            },
        )
        .unwrap();
        assert!(matches!(
            cache.lookup(&corrupt).unwrap(),
            RepeatCacheLookup::Ineligible(_)
        ));
        cache.store(&signature(1), hashes(1)).unwrap();
        cache.store(&signature(2), hashes(2)).unwrap();
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
        assert!(cache
            .db
            .get(encode_order_key(41, &encoded_key).unwrap())
            .unwrap()
            .is_some());
        assert!(cache
            .db
            .get(encode_order_key(5, b"missing-entry").unwrap())
            .unwrap()
            .is_none());
    }

    #[test]
    fn deterministic_pruning_keeps_newest_entries_with_fixed_encoded_bounds() {
        let temp = TempDir::new().unwrap();
        let cache = RepeatHashCache::open_with_limits(
            temp.path(),
            StoreLimits {
                maximum_entries: 5,
                prune_target: 3,
            },
        )
        .unwrap();
        for id in 0..6 {
            cache.store(&signature(id), hashes(id)).unwrap();
        }
        let stats = cache.stats().unwrap();
        assert_eq!(stats.live_entries, 4);
        assert!(stats.encoded_key_bytes <= stats.live_entries * MAXIMUM_ENCODED_KEY_BYTES as u64);
        assert!(
            stats.encoded_value_bytes <= stats.live_entries * MAXIMUM_ENCODED_VALUE_BYTES as u64
        );
        for id in 0..2 {
            assert_eq!(
                cache.lookup(&signature(id)).unwrap(),
                RepeatCacheLookup::Miss
            );
        }
        for id in 2..6 {
            assert_eq!(
                cache.lookup(&signature(id)).unwrap(),
                RepeatCacheLookup::Hit(hashes(id))
            );
        }
    }
}
