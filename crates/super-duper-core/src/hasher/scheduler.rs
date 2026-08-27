use crate::platform::{StorageDevice, StorageMediaClass};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

pub(crate) const ROTATIONAL_READERS: usize = 1;
pub(crate) const UNKNOWN_DEVICE_READERS: usize = 1;
pub(crate) const SOLID_STATE_READERS: usize = 4;

#[derive(Debug)]
pub(crate) struct ScheduledRead<T> {
    pub device: StorageDevice,
    pub value: T,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DeviceReadPolicy {
    global_readers: usize,
    solid_state_readers: usize,
}

impl Default for DeviceReadPolicy {
    fn default() -> Self {
        Self {
            global_readers: std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1)
                .max(1),
            solid_state_readers: SOLID_STATE_READERS,
        }
    }
}

impl DeviceReadPolicy {
    #[cfg(test)]
    pub(crate) fn for_test(global_readers: usize, solid_state_readers: usize) -> Self {
        Self {
            global_readers: global_readers.max(1),
            solid_state_readers: solid_state_readers.max(1),
        }
    }

    fn readers_for(self, media: StorageMediaClass) -> usize {
        match media {
            StorageMediaClass::Rotational => ROTATIONAL_READERS,
            StorageMediaClass::SolidState => self.solid_state_readers,
            StorageMediaClass::Unknown => UNKNOWN_DEVICE_READERS,
        }
        .min(self.global_readers)
        .max(1)
    }
}

struct DeviceQueue<T> {
    pending: VecDeque<(usize, T)>,
    active: usize,
    limit: usize,
}

struct SchedulerState<T, R> {
    queues: Vec<DeviceQueue<T>>,
    next_queue: usize,
    active: usize,
    stopped: bool,
    error: Option<io::Error>,
    results: Vec<Option<R>>,
}

pub(crate) fn execute_device_reads<T, R, F>(
    tasks: Vec<ScheduledRead<T>>,
    cancel: &AtomicBool,
    policy: DeviceReadPolicy,
    work: F,
) -> io::Result<Vec<R>>
where
    T: Send,
    R: Send,
    F: Fn(T) -> io::Result<R> + Sync,
{
    if tasks.is_empty() {
        return Ok(Vec::new());
    }

    let task_count = tasks.len();
    let mut queue_indexes = HashMap::<String, usize>::new();
    let mut queues = Vec::<DeviceQueue<T>>::new();
    for (task_index, task) in tasks.into_iter().enumerate() {
        let requested_limit = policy.readers_for(task.device.media);
        let queue_index = match queue_indexes.get(&task.device.key) {
            Some(index) => {
                // Conflicting media observations for one physical key fail conservatively.
                queues[*index].limit = queues[*index].limit.min(requested_limit);
                *index
            }
            None => {
                let index = queues.len();
                queue_indexes.insert(task.device.key, index);
                queues.push(DeviceQueue {
                    pending: VecDeque::new(),
                    active: 0,
                    limit: requested_limit,
                });
                index
            }
        };
        queues[queue_index]
            .pending
            .push_back((task_index, task.value));
    }

    let shared = (
        Mutex::new(SchedulerState {
            queues,
            next_queue: 0,
            active: 0,
            stopped: false,
            error: None,
            results: std::iter::repeat_with(|| None).take(task_count).collect(),
        }),
        Condvar::new(),
    );
    let worker_count = policy.global_readers.min(task_count).max(1);

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let shared = &shared;
            let work = &work;
            scope.spawn(move || loop {
                let task = {
                    let (lock, ready) = shared;
                    let mut state = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    loop {
                        if cancel.load(Ordering::Acquire) && !state.stopped {
                            state.stopped = true;
                            state.error = Some(io::Error::new(
                                io::ErrorKind::Interrupted,
                                "hash scheduling cancelled",
                            ));
                            for queue in &mut state.queues {
                                queue.pending.clear();
                            }
                        }
                        if state.stopped {
                            if state.active == 0 {
                                return;
                            }
                            state = ready
                                .wait(state)
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            continue;
                        }

                        let queue_count = state.queues.len();
                        let mut selected = None;
                        for offset in 0..queue_count {
                            let index = (state.next_queue + offset) % queue_count;
                            let queue = &state.queues[index];
                            if queue.active < queue.limit && !queue.pending.is_empty() {
                                selected = Some(index);
                                break;
                            }
                        }
                        if let Some(index) = selected {
                            state.next_queue = (index + 1) % queue_count;
                            let task = state.queues[index]
                                .pending
                                .pop_front()
                                .expect("selected queue must contain a task");
                            state.queues[index].active += 1;
                            state.active += 1;
                            break (index, task);
                        }
                        if state.active == 0 {
                            return;
                        }
                        state = ready
                            .wait(state)
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                    }
                };

                let (queue_index, (task_index, value)) = task;
                let result = if cancel.load(Ordering::Acquire) {
                    Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "hash scheduling cancelled",
                    ))
                } else {
                    work(value)
                };

                let (lock, ready) = shared;
                let mut state = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                state.queues[queue_index].active -= 1;
                state.active -= 1;
                match result {
                    Ok(value) if !state.stopped => state.results[task_index] = Some(value),
                    Ok(_) => {}
                    Err(error) if !state.stopped => {
                        state.stopped = true;
                        state.error = Some(error);
                        for queue in &mut state.queues {
                            queue.pending.clear();
                        }
                    }
                    Err(_) => {}
                }
                ready.notify_all();
            });
        }
    });

    let mut state = shared
        .0
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(error) = state.error.take() {
        return Err(error);
    }
    state
        .results
        .into_iter()
        .map(|result| {
            result.ok_or_else(|| {
                io::Error::new(io::ErrorKind::Other, "device scheduler omitted a result")
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "windows")]
    use crate::telemetry::{SamplerPlatform, WindowsSamplerPlatform};
    #[cfg(target_os = "windows")]
    use std::alloc::{alloc, dealloc, Layout};
    #[cfg(target_os = "windows")]
    use std::fs::{self, OpenOptions};
    #[cfg(target_os = "windows")]
    use std::hash::Hasher;
    #[cfg(target_os = "windows")]
    use std::io::{Read, Write};
    #[cfg(target_os = "windows")]
    use std::os::windows::fs::OpenOptionsExt;
    #[cfg(target_os = "windows")]
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Barrier};
    use std::time::Duration;

    fn task(device: &str, media: StorageMediaClass, value: usize) -> ScheduledRead<usize> {
        ScheduledRead {
            device: StorageDevice {
                key: device.to_owned(),
                media,
            },
            value,
        }
    }

    fn observe_max(maximum: &AtomicUsize, value: usize) {
        let mut current = maximum.load(Ordering::Relaxed);
        while value > current {
            match maximum.compare_exchange_weak(
                current,
                value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
    }

    #[test]
    fn rotational_and_unknown_devices_are_serialized() {
        for media in [StorageMediaClass::Rotational, StorageMediaClass::Unknown] {
            let active = AtomicUsize::new(0);
            let maximum = AtomicUsize::new(0);
            let values = execute_device_reads(
                (0..12).map(|value| task("same", media, value)).collect(),
                &AtomicBool::new(false),
                DeviceReadPolicy::for_test(8, 4),
                |value| {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    observe_max(&maximum, now);
                    std::thread::sleep(Duration::from_millis(1));
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(value)
                },
            )
            .unwrap();
            assert_eq!(values, (0..12).collect::<Vec<_>>());
            assert_eq!(maximum.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn solid_state_device_uses_its_bounded_reader_ceiling() {
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);
        let first_readers = Barrier::new(4);
        execute_device_reads(
            (0..4)
                .map(|value| task("ssd", StorageMediaClass::SolidState, value))
                .collect(),
            &AtomicBool::new(false),
            DeviceReadPolicy::for_test(8, 4),
            |value| {
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                observe_max(&maximum, now);
                first_readers.wait();
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(value)
            },
        )
        .unwrap();
        assert_eq!(maximum.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn separate_rotational_devices_progress_independently() {
        let barrier = Arc::new(Barrier::new(3));
        let worker_barrier = barrier.clone();
        let join = std::thread::spawn(move || {
            execute_device_reads(
                vec![
                    task("disk-a", StorageMediaClass::Rotational, 1),
                    task("disk-b", StorageMediaClass::Rotational, 2),
                ],
                &AtomicBool::new(false),
                DeviceReadPolicy::for_test(2, 2),
                |value| {
                    worker_barrier.wait();
                    Ok(value)
                },
            )
        });
        barrier.wait();
        assert_eq!(join.join().unwrap().unwrap(), vec![1, 2]);
    }

    #[test]
    fn cancellation_prevents_queued_content_work_from_starting() {
        let cancel = Arc::new(AtomicBool::new(false));
        let opens = Arc::new(AtomicUsize::new(0));
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_cancel = cancel.clone();
        let worker_opens = opens.clone();
        let worker_release = release.clone();
        let join = std::thread::spawn(move || {
            execute_device_reads(
                (0..8)
                    .map(|value| task("hdd", StorageMediaClass::Rotational, value))
                    .collect(),
                &worker_cancel,
                DeviceReadPolicy::for_test(8, 4),
                |value| {
                    worker_opens.fetch_add(1, Ordering::SeqCst);
                    if value == 0 {
                        started_tx.send(()).unwrap();
                        let (lock, ready) = &*worker_release;
                        let mut released =
                            lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                        while !*released {
                            released = ready
                                .wait(released)
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                        }
                    }
                    Ok(value)
                },
            )
        });
        started_rx.recv().unwrap();
        cancel.store(true, Ordering::Release);
        let (lock, ready) = &*release;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        ready.notify_all();
        let error = join.join().unwrap().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(opens.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn conflicting_media_for_one_key_uses_the_more_conservative_limit() {
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);
        execute_device_reads(
            vec![
                task("disk", StorageMediaClass::SolidState, 1),
                task("disk", StorageMediaClass::Rotational, 2),
                task("disk", StorageMediaClass::SolidState, 3),
            ],
            &AtomicBool::new(false),
            DeviceReadPolicy::for_test(4, 4),
            |value| {
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                observe_max(&maximum, now);
                std::thread::sleep(Duration::from_millis(1));
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(value)
            },
        )
        .unwrap();
        assert_eq!(maximum.load(Ordering::SeqCst), 1);
    }

    #[cfg(target_os = "windows")]
    struct ProfileFixture(PathBuf);

    #[cfg(target_os = "windows")]
    impl Drop for ProfileFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(target_os = "windows")]
    fn write_profile_fixture(root: &Path, file_count: usize, file_bytes: u64) -> ProfileFixture {
        assert!(root.is_absolute(), "profile root must be absolute");
        assert!(root.is_dir(), "profile root must already exist");
        assert_eq!(file_bytes % 4096, 0, "profile bytes must be 4 KiB aligned");
        let fixture = root.join(format!(
            "super-duper-sop6-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        fs::create_dir(&fixture).unwrap();
        let mut buffer = vec![0_u8; 1024 * 1024];
        for index in 0..file_count {
            let path = fixture.join(format!("reader-{index:04}.bin"));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap();
            let mut remaining = file_bytes;
            let mut state = 0x9e37_79b9_7f4a_7c15_u64 ^ index as u64;
            while remaining > 0 {
                let count = remaining.min(buffer.len() as u64) as usize;
                for chunk in buffer[..count].chunks_mut(8) {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    let bytes = state.to_le_bytes();
                    chunk.copy_from_slice(&bytes[..chunk.len()]);
                }
                file.write_all(&buffer[..count]).unwrap();
                remaining -= count as u64;
            }
            file.sync_all().unwrap();
        }
        ProfileFixture(fixture)
    }

    #[cfg(target_os = "windows")]
    fn read_unbuffered(path: &Path) -> io::Result<(u64, u64)> {
        const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
        const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;
        const BUFFER_BYTES: usize = 1024 * 1024;
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_NO_BUFFERING | FILE_FLAG_SEQUENTIAL_SCAN)
            .open(path)?;
        let layout = Layout::from_size_align(BUFFER_BYTES, 4096).unwrap();
        let pointer = unsafe { alloc(layout) };
        if pointer.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "aligned profile buffer allocation failed",
            ));
        }
        let buffer = unsafe { std::slice::from_raw_parts_mut(pointer, BUFFER_BYTES) };
        let mut bytes_read = 0_u64;
        let mut hasher = twox_hash::XxHash64::with_seed(0);
        let result = loop {
            match file.read(buffer) {
                Ok(0) => break Ok((bytes_read, hasher.finish())),
                Ok(count) => {
                    bytes_read += count as u64;
                    hasher.write(&buffer[..count]);
                }
                Err(error) => break Err(error),
            }
        };
        unsafe { dealloc(pointer, layout) };
        result
    }

    #[cfg(target_os = "windows")]
    fn profile_arm(
        paths: &[PathBuf],
        device: &StorageDevice,
        readers: usize,
        sampler: &mut WindowsSamplerPlatform,
        descriptor: &crate::telemetry::DeviceDescriptor,
    ) -> serde_json::Value {
        let host_before = sampler.sample_host();
        let _ = sampler.sample_devices(std::slice::from_ref(descriptor));
        let started = std::time::Instant::now();
        let results = execute_device_reads(
            paths
                .iter()
                .cloned()
                .map(|path| ScheduledRead {
                    device: StorageDevice {
                        key: device.key.clone(),
                        media: StorageMediaClass::SolidState,
                    },
                    value: path,
                })
                .collect(),
            &AtomicBool::new(false),
            DeviceReadPolicy::for_test(readers, readers),
            |path| read_unbuffered(&path),
        )
        .unwrap();
        let elapsed = started.elapsed();
        let device_after = sampler
            .sample_devices(std::slice::from_ref(descriptor))
            .into_iter()
            .next()
            .unwrap();
        let host_after = sampler.sample_host();
        let bytes = results.iter().map(|(bytes, _)| *bytes).sum::<u64>();
        serde_json::json!({
            "readers": readers,
            "wallNanos": elapsed.as_nanos().min(u128::from(u64::MAX)) as u64,
            "physicalBytesRequested": bytes,
            "throughputBytesPerSecond": (u128::from(bytes) * 1_000_000_000 / elapsed.as_nanos().max(1)) as u64,
            "checksums": results.iter().map(|(_, hash)| format!("{hash:016x}")).collect::<Vec<_>>(),
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
        })
    }

    #[test]
    #[ignore = "SOP6 physical-device reader comparison; requires explicit root/output environment"]
    #[cfg(target_os = "windows")]
    fn sop6_physical_device_reader_profile() {
        let root = PathBuf::from(
            std::env::var("SUPER_DUPER_SOP6_PROFILE_ROOT")
                .expect("SUPER_DUPER_SOP6_PROFILE_ROOT is required"),
        );
        let output = PathBuf::from(
            std::env::var("SUPER_DUPER_SOP6_PROFILE_OUTPUT")
                .expect("SUPER_DUPER_SOP6_PROFILE_OUTPUT is required"),
        );
        assert!(!output.exists(), "profile output is write-once");
        let expected_media = std::env::var("SUPER_DUPER_SOP6_EXPECT_MEDIA")
            .expect("SUPER_DUPER_SOP6_EXPECT_MEDIA is required");
        let candidate_readers = std::env::var("SUPER_DUPER_SOP6_CANDIDATE_READERS")
            .expect("SUPER_DUPER_SOP6_CANDIDATE_READERS is required")
            .parse::<usize>()
            .unwrap();
        assert!(candidate_readers > 1);
        let file_count = std::env::var("SUPER_DUPER_SOP6_FILE_COUNT")
            .unwrap_or_else(|_| "16".to_owned())
            .parse::<usize>()
            .unwrap();
        let file_bytes = std::env::var("SUPER_DUPER_SOP6_FILE_BYTES")
            .unwrap_or_else(|_| (64_u64 * 1024 * 1024).to_string())
            .parse::<u64>()
            .unwrap();
        let fixture = write_profile_fixture(&root, file_count, file_bytes);
        let mut paths = fs::read_dir(&fixture.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        let device = crate::platform::storage_device_for_path(&paths[0]);
        let actual_media = match device.media {
            StorageMediaClass::Rotational => "rotational",
            StorageMediaClass::SolidState => "solid_state",
            StorageMediaClass::Unknown => "unknown",
        };
        assert_eq!(actual_media, expected_media);
        assert_ne!(device.key, crate::platform::UNKNOWN_STORAGE_DEVICE_KEY);

        let mut sampler = WindowsSamplerPlatform::default();
        let descriptors = sampler
            .describe_targets(&[root.to_string_lossy().into_owned()])
            .unwrap();
        let descriptor = descriptors
            .iter()
            .find(|descriptor| descriptor.device_key == device.key)
            .expect("sampler and scheduler must agree on the physical device")
            .clone();
        let order = [1, candidate_readers, candidate_readers, 1];
        let mut samples = Vec::new();
        for readers in order {
            samples.push(profile_arm(
                &paths,
                &device,
                readers,
                &mut sampler,
                &descriptor,
            ));
        }
        let checksums = samples[0]["checksums"].clone();
        assert!(samples
            .iter()
            .all(|sample| sample["checksums"] == checksums));
        let evidence = serde_json::json!({
            "schemaVersion": 1,
            "gate": "SOP6-device-aware-scheduler",
            "profile": "physical-device-reader-comparison-v1",
            "capturedAtUtc": chrono::Utc::now().to_rfc3339(),
            "deviceKey": device.key,
            "mediaClass": actual_media,
            "volumeKey": descriptor.volume_key,
            "filesystem": descriptor.filesystem,
            "capacityBytes": descriptor.capacity_bytes,
            "freeBytesAtStart": descriptor.free_bytes_at_start,
            "hardwareSerialPersisted": false,
            "directUnbufferedReads": true,
            "fileCount": file_count,
            "fileBytes": file_bytes,
            "bytesPerArm": (file_count as u64).saturating_mul(file_bytes),
            "order": order,
            "samples": samples,
            "fixtureRemovedAfterProfile": true,
        });
        fs::write(&output, serde_json::to_vec_pretty(&evidence).unwrap()).unwrap();
        println!("sop6-device-profile={}", output.display());
    }
}
