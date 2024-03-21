extern crate winapi;

// use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
// use std::path::PathBuf;
use std::ptr;
use winapi::um::fileapi::OPEN_EXISTING;
use winapi::um::fileapi::{CreateFileW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION};
use winapi::um::handleapi::CloseHandle;

use winapi::um::winnt::{FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE};

pub(crate) fn get_win_file_id(file_path: &Path) -> (i32, i64) {
    let file_path_wide: Vec<u16> = file_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let handle = CreateFileW(
            file_path_wide.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        );

        if handle.is_null() {
            panic!("Failed to open file: {:?}", file_path);
        }

        let mut file_info: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
        if GetFileInformationByHandle(handle, &mut file_info) == 0 {
            CloseHandle(handle);
            panic!("Failed to get file information: {:?}", file_path);
        }

        CloseHandle(handle);

        // Convert file index to u64 and then to i64
        let file_index =
            (((file_info.nFileIndexHigh as u64) << 32) | file_info.nFileIndexLow as u64) as i64;

        // Return the volume serial number (converted to i32) and file index (as i64)
        (file_info.dwVolumeSerialNumber as i32, file_index)
    }
}
