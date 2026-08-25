use std::io;
use std::time::Instant;

use chrono::Utc;

use super::models::{DeviceDescriptor, DeviceSample, HostSample, TelemetryPhase};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostGaugeSnapshot {
    pub process_cpu_nanos: Option<u64>,
    pub process_private_bytes: Option<u64>,
    pub process_working_set_bytes: Option<u64>,
    pub process_peak_working_set_bytes: Option<u64>,
    pub process_read_operations: Option<u64>,
    pub process_read_bytes: Option<u64>,
    pub process_write_operations: Option<u64>,
    pub process_write_bytes: Option<u64>,
    pub system_cpu_basis_points: Option<u32>,
    pub system_available_memory_bytes: Option<u64>,
    pub system_committed_memory_bytes: Option<u64>,
    pub unavailable_counter_count: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceGaugeSnapshot {
    pub device_key: String,
    pub read_bytes_per_second: Option<u64>,
    pub read_iops_millis: Option<u64>,
    pub average_read_latency_micros: Option<u64>,
    pub active_millis_per_second: Option<u32>,
    pub queue_depth_millis: Option<u64>,
    pub unavailable_counter_count: u32,
}

pub trait SamplerClock {
    fn unix_millis(&self) -> i64;
    fn monotonic_nanos(&self) -> u64;
}

pub struct SystemSamplerClock {
    origin: Instant,
}

impl Default for SystemSamplerClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl SamplerClock for SystemSamplerClock {
    fn unix_millis(&self) -> i64 {
        Utc::now().timestamp_millis()
    }

    fn monotonic_nanos(&self) -> u64 {
        self.origin.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }
}

pub trait SamplerPlatform {
    fn describe_targets(&mut self, roots: &[String]) -> io::Result<Vec<DeviceDescriptor>>;
    fn sample_host(&mut self) -> HostGaugeSnapshot;
    fn sample_devices(&mut self, devices: &[DeviceDescriptor]) -> Vec<DeviceGaugeSnapshot>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetrySampleBatch {
    pub host: HostSample,
    pub devices: Vec<DeviceSample>,
    pub samples_lost_since_previous: u64,
}

/// Bounded cadence controller around platform probes. It owns no database connection and performs
/// no background work; the worker can call it from its telemetry writer at any progress boundary.
pub struct TelemetrySampler<P, C> {
    platform: P,
    clock: C,
    devices: Vec<DeviceDescriptor>,
    interval_nanos: u64,
    maximum_samples: u64,
    emitted_samples: u64,
    last_sample_nanos: Option<u64>,
}

impl<P: SamplerPlatform, C: SamplerClock> TelemetrySampler<P, C> {
    pub fn new(
        mut platform: P,
        clock: C,
        roots: &[String],
        interval_nanos: u64,
        maximum_samples: u64,
    ) -> io::Result<Self> {
        if interval_nanos == 0 || maximum_samples == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "telemetry sampling interval and maximum sample count must be positive",
            ));
        }
        let devices = platform.describe_targets(roots)?;
        if devices.len() > super::status_db::MAX_STATUS_DEVICES_PER_RUN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "platform returned more target devices than the status contract permits",
            ));
        }
        Ok(Self {
            platform,
            clock,
            devices,
            interval_nanos,
            maximum_samples,
            emitted_samples: 0,
            last_sample_nanos: None,
        })
    }

    pub fn devices(&self) -> &[DeviceDescriptor] {
        &self.devices
    }

    pub fn try_sample(
        &mut self,
        sequence: u64,
        phase: Option<TelemetryPhase>,
    ) -> Option<TelemetrySampleBatch> {
        if self.emitted_samples >= self.maximum_samples {
            return None;
        }
        let monotonic_nanos = self.clock.monotonic_nanos();
        let samples_lost_since_previous = match self.last_sample_nanos {
            Some(previous) => {
                let elapsed = monotonic_nanos.saturating_sub(previous);
                if elapsed < self.interval_nanos {
                    return None;
                }
                elapsed
                    .checked_div(self.interval_nanos)
                    .unwrap_or(0)
                    .saturating_sub(1)
            }
            None => 0,
        };
        self.last_sample_nanos = Some(monotonic_nanos);
        self.emitted_samples = self.emitted_samples.saturating_add(1);

        let host_snapshot = self.platform.sample_host();
        let host = HostSample {
            sequence,
            observed_unix_millis: self.clock.unix_millis(),
            monotonic_nanos,
            phase,
            process_cpu_nanos: host_snapshot.process_cpu_nanos,
            process_private_bytes: host_snapshot.process_private_bytes,
            process_working_set_bytes: host_snapshot.process_working_set_bytes,
            process_peak_working_set_bytes: host_snapshot.process_peak_working_set_bytes,
            process_read_operations: host_snapshot.process_read_operations,
            process_read_bytes: host_snapshot.process_read_bytes,
            process_write_operations: host_snapshot.process_write_operations,
            process_write_bytes: host_snapshot.process_write_bytes,
            system_cpu_basis_points: host_snapshot.system_cpu_basis_points,
            system_available_memory_bytes: host_snapshot.system_available_memory_bytes,
            system_committed_memory_bytes: host_snapshot.system_committed_memory_bytes,
            unavailable_counter_count: host_snapshot.unavailable_counter_count,
        };
        let devices = self
            .platform
            .sample_devices(&self.devices)
            .into_iter()
            .map(|snapshot| DeviceSample {
                sequence,
                device_key: snapshot.device_key,
                read_bytes_per_second: snapshot.read_bytes_per_second,
                read_iops_millis: snapshot.read_iops_millis,
                average_read_latency_micros: snapshot.average_read_latency_micros,
                active_millis_per_second: snapshot.active_millis_per_second,
                queue_depth_millis: snapshot.queue_depth_millis,
                unavailable_counter_count: snapshot.unavailable_counter_count,
            })
            .collect();
        Some(TelemetrySampleBatch {
            host,
            devices,
            samples_lost_since_previous,
        })
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::collections::{HashMap, HashSet};
    use std::ffi::OsStr;
    use std::hash::Hasher;
    use std::io;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;
    use std::time::Instant;

    use twox_hash::XxHash64;
    use winapi::shared::minwindef::{DWORD, FILETIME, LPVOID};
    use winapi::shared::ntdef::ULARGE_INTEGER;
    use winapi::um::fileapi::{
        CreateFileW, GetDiskFreeSpaceExW, GetVolumeInformationW, GetVolumeNameForVolumeMountPointW,
        OPEN_EXISTING,
    };
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::ioapiset::DeviceIoControl;
    use winapi::um::processthreadsapi::{GetCurrentProcess, GetProcessTimes, GetSystemTimes};
    use winapi::um::psapi::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
    };
    use winapi::um::sysinfoapi::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use winapi::um::winbase::GetProcessIoCounters;
    use winapi::um::winioctl::{
        DISK_PERFORMANCE, IOCTL_DISK_PERFORMANCE, IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
        VOLUME_DISK_EXTENTS,
    };
    use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, IO_COUNTERS};

    use super::{DeviceGaugeSnapshot, HostGaugeSnapshot, SamplerPlatform};
    use crate::telemetry::DeviceDescriptor;

    #[derive(Clone, Copy)]
    struct SystemTimes {
        idle: u64,
        kernel: u64,
        user: u64,
    }

    #[derive(Clone, Copy)]
    struct DiskTotals {
        observed: Instant,
        bytes_read: u64,
        read_count: u64,
        read_time_100ns: u64,
    }

    #[derive(Default)]
    pub struct WindowsSamplerPlatform {
        previous_system_times: Option<SystemTimes>,
        previous_disk_totals: HashMap<String, DiskTotals>,
    }

    impl SamplerPlatform for WindowsSamplerPlatform {
        fn describe_targets(&mut self, roots: &[String]) -> io::Result<Vec<DeviceDescriptor>> {
            let mut seen = HashSet::new();
            let mut descriptors = Vec::new();
            for root in roots {
                let Some(volume_root) = volume_root(Path::new(root)) else {
                    continue;
                };
                let volume_key = volume_name(&volume_root).unwrap_or_else(|| volume_root.clone());
                let device_key = physical_disk_number(&volume_root)
                    .map(|number| format!("physical:{number}"))
                    .unwrap_or_else(|| format!("volume:{:016x}", stable_hash(&volume_key)));
                if !seen.insert((device_key.clone(), volume_key.clone())) {
                    continue;
                }
                let (filesystem, capacity_bytes, free_bytes_at_start) =
                    volume_details(&volume_root);
                descriptors.push(DeviceDescriptor {
                    device_key,
                    volume_key,
                    filesystem,
                    capacity_bytes,
                    free_bytes_at_start,
                    bus_type: None,
                    media_type: None,
                    model: None,
                });
            }
            Ok(descriptors)
        }

        fn sample_host(&mut self) -> HostGaugeSnapshot {
            let mut sample = HostGaugeSnapshot::default();
            let process = unsafe { GetCurrentProcess() };

            let mut creation: FILETIME = unsafe { zeroed() };
            let mut exit: FILETIME = unsafe { zeroed() };
            let mut kernel: FILETIME = unsafe { zeroed() };
            let mut user: FILETIME = unsafe { zeroed() };
            if unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) }
                != 0
            {
                sample.process_cpu_nanos = filetime_value(kernel)
                    .checked_add(filetime_value(user))
                    .and_then(|ticks| ticks.checked_mul(100));
            } else {
                sample.unavailable_counter_count += 1;
            }

            let mut memory: PROCESS_MEMORY_COUNTERS_EX = unsafe { zeroed() };
            memory.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as DWORD;
            if unsafe {
                GetProcessMemoryInfo(
                    process,
                    &mut memory as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
                    memory.cb,
                )
            } != 0
            {
                sample.process_private_bytes = Some(memory.PrivateUsage as u64);
                sample.process_working_set_bytes = Some(memory.WorkingSetSize as u64);
                sample.process_peak_working_set_bytes = Some(memory.PeakWorkingSetSize as u64);
            } else {
                sample.unavailable_counter_count += 3;
            }

            let mut io_counters: IO_COUNTERS = unsafe { zeroed() };
            if unsafe { GetProcessIoCounters(process, &mut io_counters) } != 0 {
                sample.process_read_operations = Some(io_counters.ReadOperationCount);
                sample.process_read_bytes = Some(io_counters.ReadTransferCount);
                sample.process_write_operations = Some(io_counters.WriteOperationCount);
                sample.process_write_bytes = Some(io_counters.WriteTransferCount);
            } else {
                sample.unavailable_counter_count += 4;
            }

            let current_system_times = system_times();
            sample.system_cpu_basis_points = current_system_times.and_then(|current| {
                let previous = self.previous_system_times.replace(current)?;
                let total = current
                    .kernel
                    .saturating_sub(previous.kernel)
                    .saturating_add(current.user.saturating_sub(previous.user));
                let idle = current.idle.saturating_sub(previous.idle);
                (total > 0).then(|| {
                    total
                        .saturating_sub(idle)
                        .saturating_mul(10_000)
                        .checked_div(total)
                        .unwrap_or(0)
                        .min(10_000) as u32
                })
            });
            if sample.system_cpu_basis_points.is_none() {
                sample.unavailable_counter_count += 1;
            }

            let mut status: MEMORYSTATUSEX = unsafe { zeroed() };
            status.dwLength = size_of::<MEMORYSTATUSEX>() as DWORD;
            if unsafe { GlobalMemoryStatusEx(&mut status) } != 0 {
                sample.system_available_memory_bytes = Some(status.ullAvailPhys);
                sample.system_committed_memory_bytes = Some(
                    status
                        .ullTotalPageFile
                        .saturating_sub(status.ullAvailPageFile),
                );
            } else {
                sample.unavailable_counter_count += 2;
            }
            sample
        }

        fn sample_devices(&mut self, devices: &[DeviceDescriptor]) -> Vec<DeviceGaugeSnapshot> {
            devices
                .iter()
                .map(|descriptor| self.sample_device(descriptor))
                .collect()
        }
    }

    impl WindowsSamplerPlatform {
        fn sample_device(&mut self, descriptor: &DeviceDescriptor) -> DeviceGaugeSnapshot {
            let mut sample = DeviceGaugeSnapshot {
                device_key: descriptor.device_key.clone(),
                ..Default::default()
            };
            let Some(number) = descriptor
                .device_key
                .strip_prefix("physical:")
                .and_then(|value| value.parse::<u32>().ok())
            else {
                sample.unavailable_counter_count = 5;
                return sample;
            };
            let Some((performance, observed)) = disk_performance(number) else {
                sample.unavailable_counter_count = 5;
                return sample;
            };
            sample.queue_depth_millis =
                Some(u64::from(performance.QueueDepth).saturating_mul(1000));
            let current = DiskTotals {
                observed,
                bytes_read: large_integer_value(performance.BytesRead),
                read_count: performance.ReadCount as u64,
                read_time_100ns: large_integer_value(performance.ReadTime),
            };
            let previous = self
                .previous_disk_totals
                .insert(descriptor.device_key.clone(), current);
            let Some(previous) = previous else {
                sample.unavailable_counter_count = 4;
                return sample;
            };
            let elapsed_nanos = current
                .observed
                .duration_since(previous.observed)
                .as_nanos() as u64;
            if elapsed_nanos == 0 {
                sample.unavailable_counter_count = 4;
                return sample;
            }
            let bytes = current.bytes_read.saturating_sub(previous.bytes_read);
            let reads = current.read_count.saturating_sub(previous.read_count);
            let read_time = current
                .read_time_100ns
                .saturating_sub(previous.read_time_100ns);
            sample.read_bytes_per_second = bytes
                .checked_mul(1_000_000_000)
                .map(|value| value / elapsed_nanos);
            sample.read_iops_millis = reads
                .checked_mul(1_000_000_000_000)
                .map(|value| value / elapsed_nanos);
            sample.average_read_latency_micros = (reads > 0).then(|| read_time / reads / 10);
            sample.active_millis_per_second = read_time
                .checked_mul(100)
                .and_then(|nanos| nanos.checked_mul(1000))
                .map(|value| (value / elapsed_nanos).min(1000) as u32);
            sample.unavailable_counter_count = [
                sample.read_bytes_per_second.is_none(),
                sample.read_iops_millis.is_none(),
                sample.average_read_latency_micros.is_none(),
                sample.active_millis_per_second.is_none(),
                sample.queue_depth_millis.is_none(),
            ]
            .into_iter()
            .filter(|missing| *missing)
            .count() as u32;
            sample
        }
    }

    fn wide_null(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    fn volume_root(path: &Path) -> Option<String> {
        let drive = crate::platform::get_drive_letter(path)?;
        Some(format!("{}:\\", drive.to_string_lossy()))
    }

    fn volume_name(volume_root: &str) -> Option<String> {
        let root = wide_null(OsStr::new(volume_root));
        let mut buffer = vec![0_u16; 128];
        let result = unsafe {
            GetVolumeNameForVolumeMountPointW(
                root.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len() as DWORD,
            )
        };
        (result != 0).then(|| wide_buffer_string(&buffer))
    }

    fn volume_details(volume_root: &str) -> (Option<String>, Option<u64>, Option<u64>) {
        let root = wide_null(OsStr::new(volume_root));
        let mut filesystem = vec![0_u16; 64];
        let filesystem_ok = unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                filesystem.as_mut_ptr(),
                filesystem.len() as DWORD,
            )
        } != 0;
        let mut free_available: ULARGE_INTEGER = unsafe { zeroed() };
        let mut total: ULARGE_INTEGER = unsafe { zeroed() };
        let mut total_free: ULARGE_INTEGER = unsafe { zeroed() };
        let space_ok = unsafe {
            GetDiskFreeSpaceExW(
                root.as_ptr(),
                &mut free_available,
                &mut total,
                &mut total_free,
            )
        } != 0;
        (
            filesystem_ok.then(|| wide_buffer_string(&filesystem)),
            space_ok.then(|| unsafe { *total.QuadPart() }),
            space_ok.then(|| unsafe { *free_available.QuadPart() }),
        )
    }

    fn physical_disk_number(volume_root: &str) -> Option<u32> {
        let device_path = format!(r"\\.\{}:", &volume_root[..1]);
        let handle = unsafe {
            CreateFileW(
                wide_null(OsStr::new(&device_path)).as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null_mut(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut extents: VOLUME_DISK_EXTENTS = unsafe { zeroed() };
        let mut returned = 0;
        let result = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
                ptr::null_mut(),
                0,
                &mut extents as *mut _ as LPVOID,
                size_of::<VOLUME_DISK_EXTENTS>() as DWORD,
                &mut returned,
                ptr::null_mut(),
            )
        };
        unsafe { CloseHandle(handle) };
        (result != 0 && extents.NumberOfDiskExtents > 0).then(|| extents.Extents[0].DiskNumber)
    }

    fn disk_performance(number: u32) -> Option<(DISK_PERFORMANCE, Instant)> {
        let path = format!(r"\\.\PhysicalDrive{number}");
        let handle = unsafe {
            CreateFileW(
                wide_null(OsStr::new(&path)).as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null_mut(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut performance: DISK_PERFORMANCE = unsafe { zeroed() };
        let mut returned = 0;
        let result = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_DISK_PERFORMANCE,
                ptr::null_mut(),
                0,
                &mut performance as *mut _ as LPVOID,
                size_of::<DISK_PERFORMANCE>() as DWORD,
                &mut returned,
                ptr::null_mut(),
            )
        };
        let observed = Instant::now();
        unsafe { CloseHandle(handle) };
        (result != 0).then_some((performance, observed))
    }

    fn system_times() -> Option<SystemTimes> {
        let mut idle: FILETIME = unsafe { zeroed() };
        let mut kernel: FILETIME = unsafe { zeroed() };
        let mut user: FILETIME = unsafe { zeroed() };
        (unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } != 0).then(|| SystemTimes {
            idle: filetime_value(idle),
            kernel: filetime_value(kernel),
            user: filetime_value(user),
        })
    }

    fn filetime_value(value: FILETIME) -> u64 {
        (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
    }

    fn large_integer_value(value: winapi::shared::ntdef::LARGE_INTEGER) -> u64 {
        unsafe { *value.QuadPart() as u64 }
    }

    fn wide_buffer_string(buffer: &[u16]) -> String {
        let length = buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..length])
    }

    fn stable_hash(value: &str) -> u64 {
        let mut hasher = XxHash64::with_seed(0);
        hasher.write(value.as_bytes());
        hasher.finish()
    }
}

#[cfg(target_os = "windows")]
pub use windows::WindowsSamplerPlatform;

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    #[derive(Clone)]
    struct FakeClock {
        monotonic: Rc<Cell<u64>>,
        unix: Rc<Cell<i64>>,
    }

    impl SamplerClock for FakeClock {
        fn unix_millis(&self) -> i64 {
            self.unix.get()
        }

        fn monotonic_nanos(&self) -> u64 {
            self.monotonic.get()
        }
    }

    struct FakePlatform {
        device_count: usize,
    }

    impl SamplerPlatform for FakePlatform {
        fn describe_targets(&mut self, _roots: &[String]) -> io::Result<Vec<DeviceDescriptor>> {
            Ok((0..self.device_count)
                .map(|index| DeviceDescriptor {
                    device_key: format!("physical:{index}"),
                    volume_key: format!("volume:{index}"),
                    ..Default::default()
                })
                .collect())
        }

        fn sample_host(&mut self) -> HostGaugeSnapshot {
            HostGaugeSnapshot {
                process_cpu_nanos: Some(10),
                process_private_bytes: None,
                unavailable_counter_count: 1,
                ..Default::default()
            }
        }

        fn sample_devices(&mut self, devices: &[DeviceDescriptor]) -> Vec<DeviceGaugeSnapshot> {
            devices
                .iter()
                .map(|device| DeviceGaugeSnapshot {
                    device_key: device.device_key.clone(),
                    read_bytes_per_second: Some(25_000_000),
                    queue_depth_millis: None,
                    unavailable_counter_count: 1,
                    ..Default::default()
                })
                .collect()
        }
    }

    #[test]
    fn sampler_enforces_cadence_bounds_and_reports_missed_intervals() {
        let monotonic = Rc::new(Cell::new(0));
        let unix = Rc::new(Cell::new(1_700_000_000_000));
        let clock = FakeClock {
            monotonic: monotonic.clone(),
            unix: unix.clone(),
        };
        let mut sampler = TelemetrySampler::new(
            FakePlatform { device_count: 1 },
            clock,
            &["ignored-by-fake".to_owned()],
            5_000_000_000,
            2,
        )
        .unwrap();
        assert_eq!(sampler.devices().len(), 1);

        let first = sampler
            .try_sample(7, Some(TelemetryPhase::Discovering))
            .unwrap();
        assert_eq!(first.host.sequence, 7);
        assert_eq!(first.host.process_cpu_nanos, Some(10));
        assert_eq!(first.host.process_private_bytes, None);
        assert_eq!(first.host.unavailable_counter_count, 1);
        assert_eq!(first.devices[0].read_bytes_per_second, Some(25_000_000));
        assert_eq!(first.devices[0].queue_depth_millis, None);
        assert_eq!(first.samples_lost_since_previous, 0);

        monotonic.set(2_000_000_000);
        assert!(sampler.try_sample(8, None).is_none());
        monotonic.set(15_000_000_000);
        unix.set(1_700_000_015_000);
        let delayed = sampler
            .try_sample(9, Some(TelemetryPhase::CandidateScreening))
            .unwrap();
        assert_eq!(delayed.samples_lost_since_previous, 2);
        assert_eq!(delayed.host.observed_unix_millis, 1_700_000_015_000);

        monotonic.set(20_000_000_000);
        assert!(sampler.try_sample(10, None).is_none());
    }

    #[test]
    fn sampler_rejects_invalid_or_over_cardinality_contracts() {
        let clock = FakeClock {
            monotonic: Rc::new(Cell::new(0)),
            unix: Rc::new(Cell::new(0)),
        };
        assert!(
            TelemetrySampler::new(FakePlatform { device_count: 1 }, clock.clone(), &[], 0, 1,)
                .is_err()
        );
        assert!(
            TelemetrySampler::new(FakePlatform { device_count: 65 }, clock, &[], 1, 1,).is_err()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_sampler_describes_local_volume_without_serials_and_samples_explicitly() {
        let temp = tempfile::tempdir().unwrap();
        let mut platform = WindowsSamplerPlatform::default();
        let descriptors = platform
            .describe_targets(&[temp.path().to_string_lossy().into_owned()])
            .unwrap();
        assert_eq!(descriptors.len(), 1);
        let descriptor = &descriptors[0];
        assert!(descriptor.device_key.starts_with("physical:"));
        assert!(!descriptor.volume_key.is_empty());
        assert!(descriptor.capacity_bytes.is_some());
        assert!(descriptor.free_bytes_at_start.is_some());
        assert!(descriptor.model.is_none());

        let host = platform.sample_host();
        assert!(host.process_cpu_nanos.is_some());
        assert!(host.process_working_set_bytes.is_some());
        let devices = platform.sample_devices(&descriptors);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_key, descriptor.device_key);
    }
}
