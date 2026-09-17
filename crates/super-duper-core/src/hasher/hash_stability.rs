//! Pinned XxHash64 values.
//!
//! Content hashes, directory fingerprints, and cache keys are persisted in SQLite and in the hash
//! cache, and are compared across runs and across app versions. Any change to the hasher, its
//! version, or the byte patterns fed to it silently invalidates stored results instead of failing,
//! so these values are asserted literally. If a hasher upgrade changes them, the upgrade is not
//! byte-compatible and needs a stored-result migration rather than a new expectation here.

use std::hash::Hasher as _;
use std::sync::atomic::AtomicBool;
use twox_hash::XxHash64;

/// Seeds used by persisted hashes across the crate.
const CONTENT_SEED: u64 = 0;
const STRUCTURAL_SEED: u64 = 0x5354_5255_4354;
const VERIFIED_SEED: u64 = 0x5645_5249_4649;
const PREFERENCE_SEED: u64 = 0x5355_5045_525f_4455;

fn pattern_bytes(length: usize) -> Vec<u8> {
    (0..length).map(|index| (index % 251) as u8).collect()
}

#[test]
fn seeded_byte_hashes_are_pinned() {
    let cases: [(u64, &[u8], u64); 8] = [
        (CONTENT_SEED, b"", 0xef46_db37_51d8_e999),
        (CONTENT_SEED, b"super-duper", 0x3f0d_621f_0af8_e4f5),
        (STRUCTURAL_SEED, b"", 0x6d9e_317b_8dbb_c87e),
        (STRUCTURAL_SEED, b"super-duper", 0x0990_a7f0_e3cb_4c5d),
        (VERIFIED_SEED, b"", 0xa337_9b1a_9d70_cdff),
        (VERIFIED_SEED, b"super-duper", 0xff5d_53b0_da97_3220),
        (PREFERENCE_SEED, b"", 0x1366_67de_7e8a_2a40),
        (PREFERENCE_SEED, b"super-duper", 0x3bdd_7b48_ff24_48d5),
    ];
    for (seed, input, expected) in cases {
        let mut hasher = XxHash64::with_seed(seed);
        hasher.write(input);
        assert_eq!(
            hasher.finish(),
            expected,
            "seed {seed:#x} over {} bytes",
            input.len()
        );
    }
}

#[test]
fn typed_write_sequences_are_pinned() {
    // The fingerprint and signature helpers mix these Hasher methods; a version that changes any
    // of their byte encodings (for example native- to little-endian) changes persisted values.
    let mut hasher = XxHash64::with_seed(STRUCTURAL_SEED);
    hasher.write_u8(1);
    hasher.write_usize(11);
    hasher.write(b"super-duper");
    hasher.write_i64(-4_096);
    hasher.write_u64(u64::MAX);
    assert_eq!(hasher.finish(), 0xdc11_4ca3_0fda_c0a7);
}

#[test]
fn streamed_and_partial_content_hashes_are_pinned() {
    let data = pattern_bytes(4096);
    assert_eq!(super::xxhash::hash_data(&data), 0x122a_8c8d_994a_d3ec);
    assert_eq!(
        super::xxhash::hash_data(&data[..1024]),
        0x138e_26c6_5048_ce29
    );

    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("pinned.bin");
    std::fs::write(&path, &data).unwrap();
    let streamed = super::xxhash::hash_file_streaming(&path, &AtomicBool::new(false)).unwrap();
    assert_eq!(streamed, 0x122a_8c8d_994a_d3ec);
    assert_eq!(
        streamed,
        super::xxhash::hash_data(&data),
        "streaming and in-memory hashing must agree"
    );
}
