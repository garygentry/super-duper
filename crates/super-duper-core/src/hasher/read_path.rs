use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::hash::Hasher as _;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use twox_hash::XxHash64;

const PARTIAL_PREFIX_BYTES: usize = 1024;
const CONTROL_BUFFER_BYTES: usize = 64 * 1024;
const CANDIDATE_BUFFER_BYTES: usize = 1024 * 1024;
const MAX_PROFILE_BUFFER_BYTES: usize = CANDIDATE_BUFFER_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExperimentFactor {
    PathLocality,
    BucketOrder,
    BufferSize,
    SequentialHint,
    PrefixReuse,
}

impl ExperimentFactor {
    fn parse(value: &str) -> io::Result<Self> {
        match value {
            "path_locality" => Ok(Self::PathLocality),
            "bucket_order" => Ok(Self::BucketOrder),
            "buffer_size" => Ok(Self::BufferSize),
            "sequential_hint" => Ok(Self::SequentialHint),
            "prefix_reuse" => Ok(Self::PrefixReuse),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported SOP7 factor: {value}"),
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::PathLocality => "path_locality",
            Self::BucketOrder => "bucket_order",
            Self::BufferSize => "buffer_size",
            Self::SequentialHint => "sequential_hint",
            Self::PrefixReuse => "prefix_reuse",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum ArmVariant {
    Control,
    Treatment,
}

const ARM_ORDER: [ArmVariant; 4] = [
    ArmVariant::Control,
    ArmVariant::Treatment,
    ArmVariant::Treatment,
    ArmVariant::Control,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathOrder {
    Encountered,
    ParentThenPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BucketOrder {
    AscendingSize,
    DescendingSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReadPlan {
    path_order: PathOrder,
    bucket_order: BucketOrder,
    buffer_bytes: usize,
    sequential_hint: bool,
    reuse_partial_prefix: bool,
    direct_unbuffered_full_read: bool,
}

impl ReadPlan {
    fn control() -> Self {
        Self {
            path_order: PathOrder::Encountered,
            bucket_order: BucketOrder::AscendingSize,
            buffer_bytes: CONTROL_BUFFER_BYTES,
            sequential_hint: false,
            reuse_partial_prefix: false,
            direct_unbuffered_full_read: false,
        }
    }

    fn for_arm(factor: ExperimentFactor, variant: ArmVariant) -> Self {
        let mut plan = Self::control();
        if matches!(
            factor,
            ExperimentFactor::PathLocality | ExperimentFactor::BucketOrder
        ) {
            // Ordering experiments must reach the physical device; this setting is identical in
            // both arms and is not a treatment variable.
            plan.direct_unbuffered_full_read = true;
        }
        if variant == ArmVariant::Treatment {
            match factor {
                ExperimentFactor::PathLocality => plan.path_order = PathOrder::ParentThenPath,
                ExperimentFactor::BucketOrder => plan.bucket_order = BucketOrder::DescendingSize,
                ExperimentFactor::BufferSize => plan.buffer_bytes = CANDIDATE_BUFFER_BYTES,
                ExperimentFactor::SequentialHint => plan.sequential_hint = true,
                ExperimentFactor::PrefixReuse => plan.reuse_partial_prefix = true,
            }
        }
        plan
    }

    fn validate(self) -> io::Result<()> {
        if self.buffer_bytes == 0 || self.buffer_bytes > MAX_PROFILE_BUFFER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "SOP7 profile buffer is outside the declared bound",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProfileEntry {
    stable_id: usize,
    bucket_size: u64,
    path: PathBuf,
}

fn ordered_entries(entries: &[ProfileEntry], plan: ReadPlan) -> Vec<ProfileEntry> {
    let mut buckets = BTreeMap::<u64, Vec<ProfileEntry>>::new();
    for entry in entries {
        buckets
            .entry(entry.bucket_size)
            .or_default()
            .push(entry.clone());
    }
    let mut buckets = buckets.into_iter().collect::<Vec<_>>();
    if plan.bucket_order == BucketOrder::DescendingSize {
        buckets.reverse();
    }
    buckets
        .into_iter()
        .flat_map(|(_, mut entries)| {
            if plan.path_order == PathOrder::ParentThenPath {
                entries.sort_by(|left, right| {
                    left.path
                        .parent()
                        .cmp(&right.path.parent())
                        .then_with(|| left.path.cmp(&right.path))
                        .then_with(|| left.stable_id.cmp(&right.stable_id))
                });
            }
            entries
        })
        .collect()
}

fn open_profile_file(path: &Path, sequential_hint: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "windows")]
    if sequential_hint {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;
        options.custom_flags(FILE_FLAG_SEQUENTIAL_SCAN);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = sequential_hint;
    options.open(path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReadResult {
    hash: u64,
    physical_bytes: u64,
}

fn read_for_profile(path: &Path, plan: ReadPlan, cancel: &AtomicBool) -> io::Result<ReadResult> {
    plan.validate()?;
    if cancel.load(Ordering::Acquire) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "SOP7 profile cancelled before content open",
        ));
    }

    let mut partial_file = open_profile_file(path, false)?;
    let mut prefix = vec![0_u8; PARTIAL_PREFIX_BYTES];
    let prefix_bytes = partial_file.read(&mut prefix)?;
    prefix.truncate(prefix_bytes);

    #[cfg(target_os = "windows")]
    if plan.direct_unbuffered_full_read {
        let mut result = read_full_unbuffered(path, plan.buffer_bytes, cancel)?;
        result.physical_bytes = result.physical_bytes.saturating_add(prefix_bytes as u64);
        return Ok(result);
    }

    let mut full_file = open_profile_file(path, plan.sequential_hint)?;
    let mut hasher = XxHash64::with_seed(0);
    let mut physical_bytes = prefix_bytes as u64;
    if plan.reuse_partial_prefix {
        hasher.write(&prefix);
        full_file.seek(SeekFrom::Start(prefix_bytes as u64))?;
    }
    let mut buffer = vec![0_u8; plan.buffer_bytes];
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "SOP7 profile cancelled during content read",
            ));
        }
        let count = full_file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.write(&buffer[..count]);
        physical_bytes = physical_bytes.saturating_add(count as u64);
    }
    Ok(ReadResult {
        hash: hasher.finish(),
        physical_bytes,
    })
}

#[cfg(target_os = "windows")]
fn read_full_unbuffered(
    path: &Path,
    buffer_bytes: usize,
    cancel: &AtomicBool,
) -> io::Result<ReadResult> {
    use std::alloc::{alloc, dealloc, Layout};
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
    const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;
    if buffer_bytes % 4096 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "SOP7 direct-read buffer must be 4 KiB aligned",
        ));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_NO_BUFFERING | FILE_FLAG_SEQUENTIAL_SCAN)
        .open(path)?;
    let layout = Layout::from_size_align(buffer_bytes, 4096).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid SOP7 direct-read layout: {error}"),
        )
    })?;
    let pointer = unsafe { alloc(layout) };
    if pointer.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::OutOfMemory,
            "SOP7 direct-read buffer allocation failed",
        ));
    }
    let buffer = unsafe { std::slice::from_raw_parts_mut(pointer, buffer_bytes) };
    let mut hasher = XxHash64::with_seed(0);
    let mut physical_bytes = 0_u64;
    let result = loop {
        if cancel.load(Ordering::Acquire) {
            break Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "SOP7 profile cancelled during direct content read",
            ));
        }
        match file.read(buffer) {
            Ok(0) => {
                break Ok(ReadResult {
                    hash: hasher.finish(),
                    physical_bytes,
                });
            }
            Ok(count) => {
                hasher.write(&buffer[..count]);
                physical_bytes = physical_bytes.saturating_add(count as u64);
            }
            Err(error) => break Err(error),
        }
    };
    unsafe { dealloc(pointer, layout) };
    result
}

fn validate_factor_isolation(factor: ExperimentFactor) -> io::Result<()> {
    let control = ReadPlan::for_arm(factor, ArmVariant::Control);
    let treatment = ReadPlan::for_arm(factor, ArmVariant::Treatment);
    control.validate()?;
    treatment.validate()?;
    let differences = [
        control.path_order != treatment.path_order,
        control.bucket_order != treatment.bucket_order,
        control.buffer_bytes != treatment.buffer_bytes,
        control.sequential_hint != treatment.sequential_hint,
        control.reuse_partial_prefix != treatment.reuse_partial_prefix,
    ]
    .into_iter()
    .filter(|changed| *changed)
    .count();
    if differences != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "SOP7 arm changes more than one factor",
        ));
    }
    Ok(())
}

fn require_write_once_output(path: &Path) -> io::Result<()> {
    if path.exists() {
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "SOP7 profile output is write-once",
        ))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod windows_profile {
    use super::*;
    use crate::hasher::scheduler::{execute_device_reads, DeviceReadPolicy, ScheduledRead};
    use crate::platform::StorageDevice;
    use crate::telemetry::{SamplerPlatform, WindowsSamplerPlatform};
    use std::fs;
    use std::io::Write;
    use std::time::Instant;

    struct Fixture {
        root: PathBuf,
        entries_by_arm: Vec<Vec<ProfileEntry>>,
    }

    fn write_fixture(root: &Path, file_count: usize, file_bytes: u64) -> io::Result<Fixture> {
        if !root.is_absolute() || !root.is_dir() || file_count < 4 || file_bytes < 4096 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "SOP7 fixture requires an absolute existing root, at least four files, and files of at least 4 KiB",
            ));
        }
        let fixture_root = root.join(format!(
            "super-duper-sop7-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        fs::create_dir(&fixture_root)?;
        let mut entries_by_arm = Vec::new();
        let mut buffer = vec![0_u8; 1024 * 1024];
        for arm in 0..ARM_ORDER.len() {
            let mut entries = Vec::new();
            let parent_count = file_count.min(8);
            // Create each parent's files together so the locality arm has a declared
            // allocation-locality workload. Control order is restored to stable ID below.
            for parent in 0..parent_count {
                for index in (parent..file_count).step_by(parent_count) {
                    let directory = fixture_root
                        .join(format!("arm-{arm}"))
                        .join(format!("parent-{parent:02}"));
                    fs::create_dir_all(&directory)?;
                    let bucket_size = file_bytes + ((index % 4) as u64 * 4096);
                    let path = directory.join(format!("read-{index:05}.bin"));
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)?;
                    let mut remaining = bucket_size;
                    let mut state = 0x9e37_79b9_7f4a_7c15_u64 ^ index as u64;
                    while remaining > 0 {
                        let count = remaining.min(buffer.len() as u64) as usize;
                        for chunk in buffer[..count].chunks_mut(8) {
                            state ^= state << 13;
                            state ^= state >> 7;
                            state ^= state << 17;
                            chunk.copy_from_slice(&state.to_le_bytes()[..chunk.len()]);
                        }
                        file.write_all(&buffer[..count])?;
                        remaining -= count as u64;
                    }
                    file.sync_all()?;
                    entries.push(ProfileEntry {
                        stable_id: index,
                        bucket_size,
                        path,
                    });
                }
            }
            entries.sort_by_key(|entry| entry.stable_id);
            entries.rotate_left(arm % file_count);
            entries_by_arm.push(entries);
        }
        Ok(Fixture {
            root: fixture_root,
            entries_by_arm,
        })
    }

    fn profile_arm(
        entries: &[ProfileEntry],
        plan: ReadPlan,
        device: &StorageDevice,
        sampler: &mut WindowsSamplerPlatform,
        descriptor: &crate::telemetry::DeviceDescriptor,
    ) -> io::Result<serde_json::Value> {
        let ordered = ordered_entries(entries, plan);
        let host_before = sampler.sample_host();
        let _ = sampler.sample_devices(std::slice::from_ref(descriptor));
        let started = Instant::now();
        let cancel = AtomicBool::new(false);
        let mut results = execute_device_reads(
            ordered
                .into_iter()
                .map(|entry| ScheduledRead {
                    device: device.clone(),
                    value: entry,
                })
                .collect(),
            &cancel,
            DeviceReadPolicy::default(),
            |entry| {
                read_for_profile(&entry.path, plan, &cancel).map(|result| (entry.stable_id, result))
            },
        )?;
        let elapsed = started.elapsed();
        let device_after = sampler
            .sample_devices(std::slice::from_ref(descriptor))
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "SOP7 device sample missing"))?;
        let host_after = sampler.sample_host();
        results.sort_by_key(|(stable_id, _)| *stable_id);
        let physical_bytes = results
            .iter()
            .map(|(_, result)| result.physical_bytes)
            .sum::<u64>();
        Ok(serde_json::json!({
            "wallNanos": elapsed.as_nanos().min(u128::from(u64::MAX)) as u64,
            "throughputBytesPerSecond": (u128::from(physical_bytes) * 1_000_000_000 / elapsed.as_nanos().max(1)) as u64,
            "physicalBytesRead": physical_bytes,
            "checksums": results.iter().map(|(_, result)| format!("{:016x}", result.hash)).collect::<Vec<_>>(),
            "cancelled": false,
            "directUnbufferedFullReads": plan.direct_unbuffered_full_read,
            "selectedReaderCeiling": match device.media {
                crate::platform::StorageMediaClass::SolidState => crate::hasher::scheduler::SOLID_STATE_READERS,
                crate::platform::StorageMediaClass::Rotational => crate::hasher::scheduler::ROTATIONAL_READERS,
                crate::platform::StorageMediaClass::Unknown => crate::hasher::scheduler::UNKNOWN_DEVICE_READERS,
            },
            "processCpuNanos": host_after.process_cpu_nanos.zip(host_before.process_cpu_nanos).map(|(after, before)| after.saturating_sub(before)),
            "processReadOperations": host_after.process_read_operations.zip(host_before.process_read_operations).map(|(after, before)| after.saturating_sub(before)),
            "processReadBytes": host_after.process_read_bytes.zip(host_before.process_read_bytes).map(|(after, before)| after.saturating_sub(before)),
            "privateBytesBefore": host_before.process_private_bytes,
            "privateBytesAfter": host_after.process_private_bytes,
            "workingSetBytesBefore": host_before.process_working_set_bytes,
            "workingSetBytesAfter": host_after.process_working_set_bytes,
            "deviceReadBytesPerSecond": device_after.read_bytes_per_second,
            "deviceReadIopsMillis": device_after.read_iops_millis,
            "deviceAverageReadLatencyMicros": device_after.average_read_latency_micros,
            "deviceActiveMillisPerSecond": device_after.active_millis_per_second,
            "deviceQueueDepthMillis": device_after.queue_depth_millis,
            "deviceUnavailableCounterCount": device_after.unavailable_counter_count,
        }))
    }

    fn validate_evidence(value: &serde_json::Value) -> io::Result<()> {
        let samples = value["samples"].as_array().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "SOP7 evidence samples missing")
        })?;
        if samples.len() != ARM_ORDER.len() || value["armOrder"] != serde_json::json!(ARM_ORDER) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "SOP7 evidence arm order is incomplete or changed",
            ));
        }
        let first_checksums = &samples[0]["checksums"];
        if samples.iter().any(|sample| {
            sample["checksums"] != *first_checksums
                || sample["wallNanos"].as_u64().unwrap_or_default() == 0
                || sample["physicalBytesRead"].as_u64().unwrap_or_default() == 0
                || sample["cancelled"] != false
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "SOP7 evidence failed checksum, byte, timing, or cancellation reconciliation",
            ));
        }
        Ok(())
    }

    fn remove_fixture_with_bounded_retry(path: &Path) -> io::Result<()> {
        let mut last_error = None;
        for attempt in 0..10 {
            match fs::remove_dir_all(path) {
                Ok(()) => return Ok(()),
                Err(error) => last_error = Some(error),
            }
            if attempt < 9 {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
        Err(last_error
            .unwrap_or_else(|| io::Error::new(io::ErrorKind::Other, "SOP7 fixture cleanup failed")))
    }

    #[test]
    #[ignore = "SOP7 one-factor physical read-path comparison; requires explicit root/output environment"]
    fn sop7_physical_read_path_profile() {
        let factor = ExperimentFactor::parse(
            &std::env::var("SUPER_DUPER_SOP7_FACTOR").expect("SUPER_DUPER_SOP7_FACTOR is required"),
        )
        .unwrap();
        validate_factor_isolation(factor).unwrap();
        let root = PathBuf::from(
            std::env::var("SUPER_DUPER_SOP7_PROFILE_ROOT")
                .expect("SUPER_DUPER_SOP7_PROFILE_ROOT is required"),
        );
        let output = PathBuf::from(
            std::env::var("SUPER_DUPER_SOP7_PROFILE_OUTPUT")
                .expect("SUPER_DUPER_SOP7_PROFILE_OUTPUT is required"),
        );
        require_write_once_output(&output).unwrap();
        let expected_media = std::env::var("SUPER_DUPER_SOP7_EXPECT_MEDIA")
            .expect("SUPER_DUPER_SOP7_EXPECT_MEDIA is required");
        let build = std::env::var("SUPER_DUPER_SOP7_SOFTWARE_BUILD")
            .expect("SUPER_DUPER_SOP7_SOFTWARE_BUILD is required");
        let file_count = std::env::var("SUPER_DUPER_SOP7_FILE_COUNT")
            .unwrap_or_else(|_| "64".to_owned())
            .parse::<usize>()
            .unwrap();
        let file_bytes = std::env::var("SUPER_DUPER_SOP7_FILE_BYTES")
            .unwrap_or_else(|_| (16_u64 * 1024 * 1024).to_string())
            .parse::<u64>()
            .unwrap();
        let fixture = write_fixture(&root, file_count, file_bytes).unwrap();
        let device = crate::platform::storage_device_for_path(&fixture.entries_by_arm[0][0].path);
        let actual_media = match device.media {
            crate::platform::StorageMediaClass::Rotational => "rotational",
            crate::platform::StorageMediaClass::SolidState => "solid_state",
            crate::platform::StorageMediaClass::Unknown => "unknown",
        };
        assert_eq!(actual_media, expected_media);
        assert_ne!(device.key, crate::platform::UNKNOWN_STORAGE_DEVICE_KEY);
        let mut sampler = WindowsSamplerPlatform::default();
        let descriptor = sampler
            .describe_targets(&[root.to_string_lossy().into_owned()])
            .unwrap()
            .into_iter()
            .find(|descriptor| descriptor.device_key == device.key)
            .expect("SOP7 sampler and scheduler mapping must agree");
        let mut samples = Vec::new();
        for (arm_index, variant) in ARM_ORDER.into_iter().enumerate() {
            let mut sample = profile_arm(
                &fixture.entries_by_arm[arm_index],
                ReadPlan::for_arm(factor, variant),
                &device,
                &mut sampler,
                &descriptor,
            )
            .unwrap();
            sample["variant"] = serde_json::json!(variant);
            samples.push(sample);
        }
        let fixture_path = fixture.root.clone();
        let input_signature = format!(
            "sop7-v1:{factor_name}:{file_count}:{file_bytes}:4",
            factor_name = factor.as_str()
        );
        remove_fixture_with_bounded_retry(&fixture_path).unwrap();
        let evidence = serde_json::json!({
            "schemaVersion": 1,
            "gate": "SOP7-hash-read-path",
            "profile": "one-factor-read-path-comparison-v1",
            "factor": factor.as_str(),
            "capturedAtUtc": chrono::Utc::now().to_rfc3339(),
            "softwareBuild": build,
            "inputSignature": input_signature,
            "deviceKey": device.key,
            "mediaClass": actual_media,
            "volumeKey": descriptor.volume_key,
            "filesystem": descriptor.filesystem,
            "capacityBytes": descriptor.capacity_bytes,
            "freeBytesAtStart": descriptor.free_bytes_at_start,
            "hardwareSerialPersisted": false,
            "fileCountPerArm": file_count,
            "baseFileBytes": file_bytes,
            "armOrder": ARM_ORDER,
            "samples": samples,
            "fixtureRemovedAfterProfile": !fixture_path.exists(),
        });
        validate_evidence(&evidence).unwrap();
        fs::write(&output, serde_json::to_vec_pretty(&evidence).unwrap()).unwrap();
        println!("sop7-read-path-profile={}", output.display());
    }

    #[test]
    fn invalid_evidence_is_rejected_before_write() {
        let evidence = serde_json::json!({
            "armOrder": ARM_ORDER,
            "samples": [{
                "checksums": ["same"],
                "wallNanos": 1,
                "physicalBytesRead": 1,
                "cancelled": false
            }]
        });
        assert_eq!(
            validate_evidence(&evidence).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn generated_fixture_cleanup_is_bounded_and_complete() {
        let directory = tempfile::tempdir().unwrap();
        let fixture = directory.path().join("super-duper-sop7-cleanup-test");
        fs::create_dir(&fixture).unwrap();
        fs::write(fixture.join("sample.bin"), b"sample").unwrap();
        remove_fixture_with_bounded_retry(&fixture).unwrap();
        assert!(!fixture.exists());
    }
}

#[test]
fn every_factor_changes_exactly_one_control_variable() {
    for factor in [
        ExperimentFactor::PathLocality,
        ExperimentFactor::BucketOrder,
        ExperimentFactor::BufferSize,
        ExperimentFactor::SequentialHint,
        ExperimentFactor::PrefixReuse,
    ] {
        validate_factor_isolation(factor).unwrap();
    }
    assert!(ExperimentFactor::parse("reader_count").is_err());
}

#[test]
fn ordering_is_deterministic_and_preserves_stable_task_identity() {
    let entries = vec![
        ProfileEntry {
            stable_id: 0,
            bucket_size: 8,
            path: PathBuf::from("z/b"),
        },
        ProfileEntry {
            stable_id: 1,
            bucket_size: 4,
            path: PathBuf::from("a/c"),
        },
        ProfileEntry {
            stable_id: 2,
            bucket_size: 8,
            path: PathBuf::from("a/a"),
        },
        ProfileEntry {
            stable_id: 3,
            bucket_size: 4,
            path: PathBuf::from("z/d"),
        },
    ];
    let control = ordered_entries(&entries, ReadPlan::control());
    assert_eq!(
        control
            .iter()
            .map(|entry| entry.stable_id)
            .collect::<Vec<_>>(),
        vec![1, 3, 0, 2]
    );
    let treatment = ordered_entries(
        &entries,
        ReadPlan::for_arm(ExperimentFactor::PathLocality, ArmVariant::Treatment),
    );
    assert_eq!(
        treatment
            .iter()
            .map(|entry| entry.stable_id)
            .collect::<Vec<_>>(),
        vec![1, 3, 2, 0]
    );
    let mut identities = treatment
        .iter()
        .map(|entry| entry.stable_id)
        .collect::<Vec<_>>();
    identities.sort_unstable();
    assert_eq!(identities, vec![0, 1, 2, 3]);
}

#[test]
fn read_plans_preserve_hashes_and_prefix_reuse_reconciles_saved_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.bin");
    let data = (0..8193)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    std::fs::write(&path, &data).unwrap();
    let control = read_for_profile(&path, ReadPlan::control(), &AtomicBool::new(false)).unwrap();
    let reuse = read_for_profile(
        &path,
        ReadPlan::for_arm(ExperimentFactor::PrefixReuse, ArmVariant::Treatment),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(control.hash, reuse.hash);
    assert_eq!(control.hash, super::xxhash::hash_data(&data));
    assert_eq!(
        control.physical_bytes,
        data.len() as u64 + PARTIAL_PREFIX_BYTES as u64
    );
    assert_eq!(reuse.physical_bytes, data.len() as u64);
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        read_for_profile(&path, ReadPlan::control(), &cancelled)
            .unwrap_err()
            .kind(),
        io::ErrorKind::Interrupted
    );
}

#[test]
fn profile_output_is_write_once_and_memory_bounds_are_fixed() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("evidence.json");
    require_write_once_output(&output).unwrap();
    std::fs::write(&output, b"occupied").unwrap();
    assert_eq!(
        require_write_once_output(&output).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(PARTIAL_PREFIX_BYTES, 1024);
    assert_eq!(MAX_PROFILE_BUFFER_BYTES, 1024 * 1024);
}
