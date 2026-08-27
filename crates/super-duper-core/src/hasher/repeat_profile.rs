use super::repeat_cache::{
    RepeatCachePolicy, RepeatHashCache, MAXIMUM_LIVE_ENTRIES, PRUNE_TARGET_ENTRIES,
    STORE_SCHEMA_VERSION,
};
use super::xxhash::{
    build_content_hash_map_with_progress, HashProgressDelta, HashProgressSink, SystemHashPipelineIo,
};
use crate::progress::SilentReporter;
use crate::telemetry::{SamplerPlatform, WindowsSamplerPlatform};
use dashmap::DashMap;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const FILE_COUNT: usize = 128;
const FILE_BYTES: u64 = 4 * 1024 * 1024;
const PAIR_COUNT: usize = FILE_COUNT / 2;
const PARTIAL_BYTES: u64 = 1024;
const TOTAL_FIXTURE_BYTES: u64 = FILE_COUNT as u64 * FILE_BYTES;
const ARM_ORDER: [&str; 4] = [
    "forced_seed",
    "reuse_same_process",
    "reuse_reopened_store",
    "forced_revalidate_tail",
];

#[derive(Default)]
struct ProfileSink {
    totals: Mutex<HashProgressDelta>,
}

impl HashProgressSink for ProfileSink {
    fn publish(&self, delta: HashProgressDelta) -> io::Result<()> {
        self.totals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .checked_add_assign(&delta)
    }

    fn snapshot(&self) -> HashProgressDelta {
        self.totals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

fn write_fixture(root: &Path, file_count: usize, file_bytes: u64) -> io::Result<Vec<PathBuf>> {
    if file_count < 4 || file_count % 2 != 0 || file_bytes < PARTIAL_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "SOP8 fixture requires an even count of at least four files and at least 1 KiB per file",
        ));
    }
    fs::create_dir_all(root)?;
    let mut paths = Vec::with_capacity(file_count);
    let mut buffer = vec![0_u8; file_bytes.min(1024 * 1024) as usize];
    for pair in 0..file_count / 2 {
        for copy in 0..2 {
            let path = root.join(format!("pair-{pair:03}-copy-{copy}.bin"));
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            let mut writer = BufWriter::with_capacity(buffer.len(), file);
            let mut remaining = file_bytes;
            let mut chunk_index = 0_u64;
            while remaining > 0 {
                let count = remaining.min(buffer.len() as u64) as usize;
                let mut state = 0x9e37_79b9_7f4a_7c15_u64
                    ^ (pair as u64).wrapping_mul(0xd6e8_feb8_6659_fd93)
                    ^ chunk_index;
                for chunk in buffer[..count].chunks_mut(8) {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    chunk.copy_from_slice(&state.to_le_bytes()[..chunk.len()]);
                }
                if chunk_index == 0 {
                    buffer[..PARTIAL_BYTES as usize].fill(0x5a);
                }
                writer.write_all(&buffer[..count])?;
                remaining -= count as u64;
                chunk_index += 1;
            }
            writer.flush()?;
            writer.get_ref().sync_all()?;
            paths.push(path);
        }
    }
    Ok(paths)
}

fn candidates(paths: &[PathBuf]) -> DashMap<u64, Vec<PathBuf>> {
    let map = DashMap::new();
    map.insert(FILE_BYTES, paths.to_vec());
    map
}

fn result_signature(outcome: &super::xxhash::HashOutcome) -> Vec<String> {
    let mut groups = outcome
        .confirmed_duplicates
        .iter()
        .map(|group| format!("{:016x}:{}", group.key(), group.value().len()))
        .collect::<Vec<_>>();
    groups.sort_unstable();
    groups
}

fn profile_arm(
    label: &str,
    policy: RepeatCachePolicy,
    cache: Arc<RepeatHashCache>,
    paths: &[PathBuf],
    sampler: &mut WindowsSamplerPlatform,
    descriptor: &crate::telemetry::DeviceDescriptor,
) -> io::Result<Value> {
    let host_before = sampler.sample_host();
    let _ = sampler.sample_devices(std::slice::from_ref(descriptor));
    let sink = ProfileSink::default();
    let started = Instant::now();
    let outcome = build_content_hash_map_with_progress(
        candidates(paths),
        &AtomicBool::new(false),
        &SilentReporter,
        &sink,
        &SystemHashPipelineIo::with_repeat_cache(cache, policy),
    )?;
    let wall_nanos = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
    let device_after = sampler
        .sample_devices(std::slice::from_ref(descriptor))
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "SOP8 device sample missing"))?;
    let host_after = sampler.sample_host();
    Ok(json!({
        "arm": label,
        "policy": policy.as_str(),
        "wallNanos": wall_nanos,
        "processCpuNanos": host_after.process_cpu_nanos.zip(host_before.process_cpu_nanos).map(|(after, before)| after.saturating_sub(before)),
        "processReadOperations": host_after.process_read_operations.zip(host_before.process_read_operations).map(|(after, before)| after.saturating_sub(before)),
        "processReadBytes": host_after.process_read_bytes.zip(host_before.process_read_bytes).map(|(after, before)| after.saturating_sub(before)),
        "privateBytesBefore": host_before.process_private_bytes,
        "privateBytesAfter": host_after.process_private_bytes,
        "workingSetBytesBefore": host_before.process_working_set_bytes,
        "workingSetBytesAfter": host_after.process_working_set_bytes,
        "peakWorkingSetBytesAfter": host_after.process_peak_working_set_bytes,
        "deviceReadBytesPerSecond": device_after.read_bytes_per_second,
        "deviceReadIopsMillis": device_after.read_iops_millis,
        "deviceAverageReadLatencyMicros": device_after.average_read_latency_micros,
        "deviceActiveMillisPerSecond": device_after.active_millis_per_second,
        "deviceQueueDepthMillis": device_after.queue_depth_millis,
        "deviceUnavailableCounterCount": device_after.unavailable_counter_count,
        "partialHashBytesRead": outcome.partial_hash_bytes_read,
        "fullHashBytesRead": outcome.full_hash_bytes_read,
        "partialCacheHits": outcome.partial_hash_cache_hits,
        "partialCacheMisses": outcome.partial_hash_cache_misses,
        "partialCacheErrors": outcome.partial_hash_cache_errors,
        "partialCacheStores": outcome.partial_hash_cache_stores,
        "fullCacheHits": outcome.full_hash_cache_hits,
        "fullCacheMisses": outcome.full_hash_cache_misses,
        "fullCacheErrors": outcome.full_hash_cache_errors,
        "fullCacheStores": outcome.full_hash_cache_stores,
        "confirmedDuplicateGroups": outcome.confirmed_duplicates.len(),
        "confirmedPhysicalItems": outcome.confirmed_duplicates.iter().map(|group| group.value().len()).sum::<usize>(),
        "warningCount": outcome.warning_count,
        "cancelledWorkItems": outcome.cancelled_work_items,
        "resultSignature": result_signature(&outcome),
    }))
}

fn median(values: &[u64]) -> u64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    if sorted.len() % 2 == 0 {
        sorted[sorted.len() / 2 - 1].saturating_add(sorted[sorted.len() / 2]) / 2
    } else {
        sorted[sorted.len() / 2]
    }
}

fn validate_samples(samples: &[Value]) -> io::Result<Value> {
    if samples.len() != ARM_ORDER.len()
        || samples
            .iter()
            .zip(ARM_ORDER)
            .any(|(sample, expected)| sample["arm"] != expected)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SOP8 arm order is incomplete or changed",
        ));
    }
    let signature = &samples[0]["resultSignature"];
    if samples.iter().any(|sample| {
        sample["resultSignature"] != *signature
            || sample["confirmedDuplicateGroups"] != PAIR_COUNT
            || sample["confirmedPhysicalItems"] != FILE_COUNT
            || sample["warningCount"] != 0
            || sample["cancelledWorkItems"] != 0
            || sample["wallNanos"].as_u64().unwrap_or_default() == 0
    }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SOP8 arms disagree on results, warnings, cancellation, or timing",
        ));
    }
    for sample in [samples.first().unwrap(), samples.last().unwrap()] {
        if sample["partialHashBytesRead"] != FILE_COUNT as u64 * PARTIAL_BYTES
            || sample["fullHashBytesRead"] != TOTAL_FIXTURE_BYTES
            || sample["partialCacheHits"] != 0
            || sample["fullCacheHits"] != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "forced SOP8 arm did not perform the declared exact reads",
            ));
        }
    }
    for sample in &samples[1..3] {
        if sample["partialHashBytesRead"] != 0
            || sample["fullHashBytesRead"] != 0
            || sample["partialCacheHits"] != FILE_COUNT
            || sample["fullCacheHits"] != FILE_COUNT
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "reuse SOP8 arm did not save the declared exact reads",
            ));
        }
    }
    let forced = [
        samples[0]["wallNanos"].as_u64().unwrap(),
        samples[3]["wallNanos"].as_u64().unwrap(),
    ];
    let reused = [
        samples[1]["wallNanos"].as_u64().unwrap(),
        samples[2]["wallNanos"].as_u64().unwrap(),
    ];
    let forced_median = median(&forced);
    let reuse_median = median(&reused);
    let improvement_basis_points = forced_median
        .saturating_sub(reuse_median)
        .saturating_mul(10_000)
        .checked_div(forced_median.max(1))
        .unwrap_or_default();
    Ok(json!({
        "forcedMedianWallNanos": forced_median,
        "forcedTailWallNanos": forced.into_iter().max().unwrap(),
        "reuseMedianWallNanos": reuse_median,
        "reuseTailWallNanos": reused.into_iter().max().unwrap(),
        "reuseWallImprovementBasisPoints": improvement_basis_points,
        "selectedDefault": if reuse_median < forced_median { "reuse_verified" } else { "revalidate_content" },
        "selectionReason": if reuse_median < forced_median {
            "both reuse arms saved exactly 128 KiB of partial reads and 512 MiB of full reads, preserved exact results, and improved aggregate wall time"
        } else {
            "verified reuse did not improve aggregate wall time despite exact read savings"
        }
    }))
}

fn remove_with_bounded_retry(path: &Path) -> io::Result<()> {
    let mut last_error = None;
    for attempt in 0..10 {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => last_error = Some(error),
        }
        if attempt < 9 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::new(io::ErrorKind::Other, "cleanup failed")))
}

fn write_once_json(path: &Path, value: &Value) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut file, value).map_err(io::Error::other)?;
    file.write_all(b"\n")?;
    file.sync_all()
}

#[test]
fn small_fixture_and_evidence_contract_is_exact() {
    assert_eq!(TOTAL_FIXTURE_BYTES, 512 * 1024 * 1024);
    assert_eq!(PAIR_COUNT, 64);
    let temp = tempfile::tempdir().unwrap();
    let fixture = temp.path().join("fixture");
    let paths = write_fixture(&fixture, 4, 4096).unwrap();
    let contents = paths
        .iter()
        .map(fs::read)
        .collect::<io::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(contents[0], contents[1]);
    assert_eq!(contents[2], contents[3]);
    assert_ne!(contents[0], contents[2]);
    assert!(contents
        .iter()
        .all(|content| content[..PARTIAL_BYTES as usize] == contents[0][..PARTIAL_BYTES as usize]));
    assert!(Path::new("docs/evidence/scan-repeat-cache-policy-20260827.json").is_relative());
}

#[test]
fn evidence_validator_selects_only_exact_faster_reuse() {
    let signature = json!(vec!["0000000000000001:2"; PAIR_COUNT]);
    let sample = |arm: &str, wall: u64, forced: bool| {
        json!({
            "arm": arm,
            "wallNanos": wall,
            "resultSignature": signature,
            "confirmedDuplicateGroups": PAIR_COUNT,
            "confirmedPhysicalItems": FILE_COUNT,
            "warningCount": 0,
            "cancelledWorkItems": 0,
            "partialHashBytesRead": if forced { FILE_COUNT as u64 * PARTIAL_BYTES } else { 0 },
            "fullHashBytesRead": if forced { TOTAL_FIXTURE_BYTES } else { 0 },
            "partialCacheHits": if forced { 0 } else { FILE_COUNT },
            "fullCacheHits": if forced { 0 } else { FILE_COUNT },
        })
    };
    let samples = vec![
        sample(ARM_ORDER[0], 1_000, true),
        sample(ARM_ORDER[1], 100, false),
        sample(ARM_ORDER[2], 120, false),
        sample(ARM_ORDER[3], 900, true),
    ];
    let decision = validate_samples(&samples).unwrap();
    assert_eq!(decision["selectedDefault"], "reuse_verified");
    let mut invalid = samples;
    invalid[2]["fullHashBytesRead"] = json!(1);
    assert_eq!(
        validate_samples(&invalid).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
#[ignore = "SOP8 write-once 512 MiB repeat-cache policy comparison; requires explicit root/output/build environment"]
fn sop8_repeat_cache_policy_profile() {
    let root = PathBuf::from(
        std::env::var("SUPER_DUPER_SOP8_PROFILE_ROOT")
            .expect("SUPER_DUPER_SOP8_PROFILE_ROOT is required"),
    );
    let output = PathBuf::from(
        std::env::var("SUPER_DUPER_SOP8_PROFILE_OUTPUT")
            .expect("SUPER_DUPER_SOP8_PROFILE_OUTPUT is required"),
    );
    let software_build = std::env::var("SUPER_DUPER_SOP8_SOFTWARE_BUILD")
        .expect("SUPER_DUPER_SOP8_SOFTWARE_BUILD is required");
    assert!(root.is_absolute() && root.is_dir());
    assert!(!output.exists(), "SOP8 evidence output is write-once");
    let fixture_root = root.join(format!(
        "super-duper-sop8-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    ));
    let cache_path = fixture_root.join("repeat-cache");
    let mut sampler = WindowsSamplerPlatform::default();
    let profile_result = (|| -> io::Result<Value> {
        let paths = write_fixture(&fixture_root.join("files"), FILE_COUNT, FILE_BYTES)?;
        let device = crate::platform::storage_device_for_path(&paths[0]);
        let media_class = match device.media {
            crate::platform::StorageMediaClass::Rotational => "rotational",
            crate::platform::StorageMediaClass::SolidState => "solid_state",
            crate::platform::StorageMediaClass::Unknown => "unknown",
        };
        if device.key == crate::platform::UNKNOWN_STORAGE_DEVICE_KEY {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "SOP8 profile requires an exact local device mapping",
            ));
        }
        let descriptor = sampler
            .describe_targets(&[root.to_string_lossy().into_owned()])?
            .into_iter()
            .find(|descriptor| descriptor.device_key == device.key)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "SOP8 sampler and scheduler mappings disagree",
                )
            })?;
        let cancellation_passed = build_content_hash_map_with_progress(
            candidates(&paths[..2]),
            &AtomicBool::new(true),
            &SilentReporter,
            &ProfileSink::default(),
            &SystemHashPipelineIo::default(),
        )
        .is_err();
        if !cancellation_passed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "SOP8 cancellation preflight unexpectedly admitted work",
            ));
        }

        let first_cache = Arc::new(RepeatHashCache::open(&cache_path)?);
        let forced_seed = profile_arm(
            ARM_ORDER[0],
            RepeatCachePolicy::RevalidateContent,
            first_cache.clone(),
            &paths,
            &mut sampler,
            &descriptor,
        )?;
        let same_process = profile_arm(
            ARM_ORDER[1],
            RepeatCachePolicy::ReuseVerified,
            first_cache.clone(),
            &paths,
            &mut sampler,
            &descriptor,
        )?;
        drop(first_cache);
        let reopened_cache = Arc::new(RepeatHashCache::open(&cache_path)?);
        let reopened = profile_arm(
            ARM_ORDER[2],
            RepeatCachePolicy::ReuseVerified,
            reopened_cache.clone(),
            &paths,
            &mut sampler,
            &descriptor,
        )?;
        let forced_tail = profile_arm(
            ARM_ORDER[3],
            RepeatCachePolicy::RevalidateContent,
            reopened_cache.clone(),
            &paths,
            &mut sampler,
            &descriptor,
        )?;
        let store_stats = reopened_cache.stats()?;
        drop(reopened_cache);
        let samples = vec![forced_seed, same_process, reopened, forced_tail];
        let decision = validate_samples(&samples)?;
        let host_identity = format!(
            "{:016x}",
            super::xxhash::hash_data(
                std::env::var("COMPUTERNAME")
                    .unwrap_or_else(|_| "unavailable".to_owned())
                    .as_bytes()
            )
        );
        Ok(json!({
            "schemaVersion": 1,
            "gate": "SOP8-repeat-run-cache",
            "package": "SOP8d-repeat-policy-measurement",
            "profile": "repeat-cache-policy-v1",
            "status": "valid",
            "capturedAtUtc": chrono::Utc::now().to_rfc3339(),
            "softwareBuild": software_build.clone(),
            "inputSignature": "sop8-v1:128-files:4194304-bytes:64-exact-pairs:shared-1024-byte-prefix",
            "hostIdentityHash": host_identity,
            "processorIdentifier": std::env::var("PROCESSOR_IDENTIFIER").ok(),
            "deviceKey": device.key,
            "mediaClass": media_class,
            "volumeKey": descriptor.volume_key,
            "filesystem": descriptor.filesystem,
            "capacityBytes": descriptor.capacity_bytes,
            "freeBytesAtStart": descriptor.free_bytes_at_start,
            "hardwareSerialPersisted": false,
            "fixture": {
                "fileCount": FILE_COUNT,
                "fileBytes": FILE_BYTES,
                "totalBytes": TOTAL_FIXTURE_BYTES,
                "exactPairCount": PAIR_COUNT,
                "sharedPartialPrefixBytes": PARTIAL_BYTES,
                "collisionHeavy": true
            },
            "store": {
                "schemaVersion": STORE_SCHEMA_VERSION,
                "maximumLiveEntries": MAXIMUM_LIVE_ENTRIES,
                "pruneTargetEntries": PRUNE_TARGET_ENTRIES,
                "liveEntriesAfterArms": store_stats.live_entries,
                "encodedKeyBytesAfterArms": store_stats.encoded_key_bytes,
                "encodedValueBytesAfterArms": store_stats.encoded_value_bytes
            },
            "signatureContract": "stable physical identity + byte length + positive non-coarse nanosecond modified time + content-change token; equal before/after observations",
            "sop6ReaderPolicy": { "rotational": 1, "solidState": 4, "unknown": 1 },
            "sop7ReadPolicy": { "bucketOrder": "descending_size", "solidStateBufferBytes": 1048576, "solidStateSequentialHint": false, "rotationalUnknownBufferBytes": 65536, "rotationalUnknownSequentialHint": true, "partialPrefixReuse": false },
            "armOrder": ARM_ORDER,
            "samples": samples,
            "decision": decision,
            "cancellationPreflightPassed": cancellation_passed,
            "allSamplesRetained": true,
            "retryForFavorableSample": false
        }))
    })();

    let cleanup = remove_with_bounded_retry(&fixture_root);
    let cleanup_passed = cleanup.is_ok() && !fixture_root.exists();
    let evidence = match profile_result {
        Ok(mut evidence) if cleanup_passed => {
            evidence["fixtureRemovedAfterProfile"] = json!(true);
            evidence
        }
        Ok(mut evidence) => {
            evidence["status"] = json!("invalid_campaign");
            evidence["failure"] = json!(match cleanup {
                Err(error) => error.to_string(),
                Ok(()) => "fixture path remained after cleanup".to_owned(),
            });
            evidence["fixtureRemovedAfterProfile"] = json!(false);
            evidence
        }
        Err(error) => json!({
            "schemaVersion": 1,
            "gate": "SOP8-repeat-run-cache",
            "package": "SOP8d-repeat-policy-measurement",
            "profile": "repeat-cache-policy-v1",
            "status": "invalid_campaign",
            "capturedAtUtc": chrono::Utc::now().to_rfc3339(),
            "softwareBuild": software_build,
            "armOrder": ARM_ORDER,
            "failure": error.to_string(),
            "fixtureRemovedAfterProfile": cleanup_passed,
            "allSamplesRetained": true,
            "retryForFavorableSample": false
        }),
    };
    write_once_json(&output, &evidence).unwrap();
    assert_eq!(evidence["status"], "valid", "SOP8 campaign failed");
    println!("sop8-repeat-cache-profile={}", output.display());
}
