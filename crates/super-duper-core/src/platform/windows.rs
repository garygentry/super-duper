use std::ffi::OsString;
use std::io;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::path::{Component, Path};
use std::ptr;
use winapi::shared::minwindef::DWORD;
use winapi::um::fileapi::{
    CreateFileW, GetFileAttributesW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    INVALID_FILE_ATTRIBUTES, OPEN_EXISTING,
};
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::ioapiset::DeviceIoControl;
use winapi::um::winbase::FILE_FLAG_BACKUP_SEMANTICS;
use winapi::um::winioctl::{IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, VOLUME_DISK_EXTENTS};
use winapi::um::winnt::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
};

use super::{PathSafety, StorageDevice, StorageMediaClass};

const FILE_ATTRIBUTE_RECALL_ON_OPEN: DWORD = 0x0004_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: DWORD = 0x0040_0000;
const IOCTL_STORAGE_QUERY_PROPERTY: DWORD = 0x002d_1400;
const STORAGE_DEVICE_SEEK_PENALTY_PROPERTY: DWORD = 7;
const PROPERTY_STANDARD_QUERY: DWORD = 0;

#[repr(C)]
struct StoragePropertyQuery {
    property_id: DWORD,
    query_type: DWORD,
    additional_parameters: [u8; 1],
}

#[repr(C)]
struct DeviceSeekPenaltyDescriptor {
    version: DWORD,
    size: DWORD,
    incurs_seek_penalty: u8,
    padding: [u8; 3],
}

trait StorageDeviceProbe {
    fn disk_numbers(&self, volume_root: &str) -> io::Result<Vec<u32>>;
    fn incurs_seek_penalty(&self, disk_number: u32) -> io::Result<bool>;
}

struct WindowsStorageDeviceProbe;

/// Classify attributes without opening the path. Cloud placeholder and reparse results must be
/// handled before any metadata, identity, canonicalization, or content operation.
pub fn classify_path_without_open(path: &Path) -> io::Result<PathSafety> {
    let mut wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
    wide.push(0);
    let attributes = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attributes == INVALID_FILE_ATTRIBUTES {
        let error = io::Error::last_os_error();
        return if error.kind() == io::ErrorKind::NotFound {
            Ok(PathSafety::Missing)
        } else {
            Err(error)
        };
    }
    if attributes
        & (FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_OPEN
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
        != 0
    {
        return Ok(PathSafety::CloudPlaceholder);
    }
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Ok(PathSafety::ReparsePoint);
    }
    if attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
        Ok(PathSafety::Directory)
    } else {
        Ok(PathSafety::File)
    }
}

pub fn get_drive_letter(path: &Path) -> Option<OsString> {
    for component in path.components() {
        if let Component::Prefix(prefix_comp) = component {
            match prefix_comp.kind() {
                std::path::Prefix::Disk(letter) | std::path::Prefix::VerbatimDisk(letter) => {
                    let drive_letter = (letter as char).to_string();
                    return Some(OsString::from(drive_letter));
                }
                _ => (),
            }
        }
    }
    None
}

pub(crate) fn storage_device_for_path(path: &Path) -> StorageDevice {
    storage_device_for_path_with_probe(path, &WindowsStorageDeviceProbe)
}

fn storage_device_for_path_with_probe(
    path: &Path,
    probe: &dyn StorageDeviceProbe,
) -> StorageDevice {
    let Some(drive) = get_drive_letter(path) else {
        return StorageDevice::mapping_unavailable();
    };
    let volume_root = format!("{}:\\", drive.to_string_lossy().to_ascii_uppercase());
    let Ok(disks) = probe.disk_numbers(&volume_root) else {
        return StorageDevice::mapping_unavailable();
    };
    let [disk_number] = disks.as_slice() else {
        return StorageDevice::mapping_unavailable();
    };
    let media = match probe.incurs_seek_penalty(*disk_number) {
        Ok(true) => StorageMediaClass::Rotational,
        Ok(false) => StorageMediaClass::SolidState,
        Err(_) => StorageMediaClass::Unknown,
    };
    StorageDevice {
        key: format!("physical:{disk_number}"),
        media,
    }
}

impl StorageDeviceProbe for WindowsStorageDeviceProbe {
    fn disk_numbers(&self, volume_root: &str) -> io::Result<Vec<u32>> {
        let device_path = format!(r"\\.\{}:", &volume_root[..1]);
        let handle = unsafe {
            CreateFileW(
                wide_null(device_path.as_ref()).as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null_mut(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let mut extents: VOLUME_DISK_EXTENTS = unsafe { zeroed() };
        let mut returned = 0;
        let result = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
                ptr::null_mut(),
                0,
                &mut extents as *mut _ as *mut _,
                size_of::<VOLUME_DISK_EXTENTS>() as DWORD,
                &mut returned,
                ptr::null_mut(),
            )
        };
        let error = (result == 0).then(io::Error::last_os_error);
        unsafe { CloseHandle(handle) };
        if let Some(error) = error {
            return Err(error);
        }
        if extents.NumberOfDiskExtents != 1 {
            return Ok(Vec::new());
        }
        Ok(vec![extents.Extents[0].DiskNumber])
    }

    fn incurs_seek_penalty(&self, disk_number: u32) -> io::Result<bool> {
        let device_path = format!(r"\\.\PhysicalDrive{disk_number}");
        let handle = unsafe {
            CreateFileW(
                wide_null(device_path.as_ref()).as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null_mut(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let query = StoragePropertyQuery {
            property_id: STORAGE_DEVICE_SEEK_PENALTY_PROPERTY,
            query_type: PROPERTY_STANDARD_QUERY,
            additional_parameters: [0],
        };
        let mut descriptor = DeviceSeekPenaltyDescriptor {
            version: size_of::<DeviceSeekPenaltyDescriptor>() as DWORD,
            size: size_of::<DeviceSeekPenaltyDescriptor>() as DWORD,
            incurs_seek_penalty: 0,
            padding: [0; 3],
        };
        let mut returned = 0;
        let result = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                &query as *const _ as *mut _,
                size_of::<StoragePropertyQuery>() as DWORD,
                &mut descriptor as *mut _ as *mut _,
                size_of::<DeviceSeekPenaltyDescriptor>() as DWORD,
                &mut returned,
                ptr::null_mut(),
            )
        };
        let error = (result == 0).then(io::Error::last_os_error);
        unsafe { CloseHandle(handle) };
        if let Some(error) = error {
            return Err(error);
        }
        if returned < size_of::<DeviceSeekPenaltyDescriptor>() as DWORD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "seek-penalty query returned a truncated descriptor",
            ));
        }
        Ok(descriptor.incurs_seek_penalty != 0)
    }
}

fn wide_null(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

/// Return the stable volume/file-index pair used by Windows to identify one physical file.
/// Multiple directory entries for a hard-linked file have the same value.
pub fn file_identity(path: &Path) -> io::Result<Option<String>> {
    let mut wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
    wide.push(0);
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null_mut(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }

    let mut information = unsafe { std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>() };
    let succeeded = unsafe { GetFileInformationByHandle(handle, &mut information) };
    let close_result = unsafe { CloseHandle(handle) };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    if close_result == 0 {
        return Err(io::Error::last_os_error());
    }

    let file_index =
        ((information.nFileIndexHigh as u64) << DWORD::BITS) | information.nFileIndexLow as u64;
    Ok(Some(format!(
        "{:08x}:{file_index:016x}",
        information.dwVolumeSerialNumber
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FakeProbe {
        disks: io::Result<Vec<u32>>,
        seek_penalties: HashMap<u32, io::Result<bool>>,
    }

    impl StorageDeviceProbe for FakeProbe {
        fn disk_numbers(&self, _volume_root: &str) -> io::Result<Vec<u32>> {
            match &self.disks {
                Ok(disks) => Ok(disks.clone()),
                Err(error) => Err(io::Error::new(error.kind(), error.to_string())),
            }
        }

        fn incurs_seek_penalty(&self, disk_number: u32) -> io::Result<bool> {
            match self.seek_penalties.get(&disk_number) {
                Some(Ok(value)) => Ok(*value),
                Some(Err(error)) => Err(io::Error::new(error.kind(), error.to_string())),
                None => Err(io::Error::new(io::ErrorKind::NotFound, "missing fake disk")),
            }
        }
    }

    fn probe(disks: io::Result<Vec<u32>>, penalty: io::Result<bool>) -> FakeProbe {
        FakeProbe {
            disks,
            seek_penalties: HashMap::from([(7, penalty)]),
        }
    }

    #[test]
    fn maps_drive_aliases_to_one_physical_rotational_device() {
        for path in [Path::new(r"D:\a.bin"), Path::new(r"\\?\D:\b.bin")] {
            assert_eq!(
                storage_device_for_path_with_probe(path, &probe(Ok(vec![7]), Ok(true))),
                StorageDevice {
                    key: "physical:7".to_owned(),
                    media: StorageMediaClass::Rotational,
                }
            );
        }
    }

    #[test]
    fn classifies_no_seek_penalty_as_solid_state() {
        assert_eq!(
            storage_device_for_path_with_probe(
                Path::new(r"C:\a.bin"),
                &probe(Ok(vec![7]), Ok(false)),
            )
            .media,
            StorageMediaClass::SolidState
        );
    }

    #[test]
    fn keeps_exact_device_identity_when_only_media_query_is_unavailable() {
        assert_eq!(
            storage_device_for_path_with_probe(
                Path::new(r"C:\a.bin"),
                &probe(
                    Ok(vec![7]),
                    Err(io::Error::new(io::ErrorKind::PermissionDenied, "blocked")),
                ),
            ),
            StorageDevice {
                key: "physical:7".to_owned(),
                media: StorageMediaClass::Unknown,
            }
        );
    }

    #[test]
    fn ambiguous_remote_and_unavailable_mapping_share_conservative_fallback() {
        for device in [
            storage_device_for_path_with_probe(
                Path::new(r"C:\a.bin"),
                &probe(Ok(vec![7, 8]), Ok(false)),
            ),
            storage_device_for_path_with_probe(
                Path::new(r"\\server\share\a.bin"),
                &probe(Ok(vec![7]), Ok(false)),
            ),
            storage_device_for_path_with_probe(
                Path::new(r"C:\a.bin"),
                &probe(
                    Err(io::Error::new(io::ErrorKind::PermissionDenied, "blocked")),
                    Ok(false),
                ),
            ),
        ] {
            assert_eq!(device, StorageDevice::mapping_unavailable());
        }
    }
}
