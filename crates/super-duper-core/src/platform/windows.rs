use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Component, Path};
use std::ptr;
use winapi::shared::minwindef::DWORD;
use winapi::um::fileapi::{
    CreateFileW, GetFileAttributesW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    INVALID_FILE_ATTRIBUTES, OPEN_EXISTING,
};
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::winbase::FILE_FLAG_BACKUP_SEMANTICS;
use winapi::um::winnt::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
};

use super::PathSafety;

const FILE_ATTRIBUTE_RECALL_ON_OPEN: DWORD = 0x0004_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: DWORD = 0x0040_0000;

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
