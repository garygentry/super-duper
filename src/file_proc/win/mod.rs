use std::fs::File;
use std::io::Error;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use winapi::um::fileapi::GetFileInformationByHandle;
use winapi::um::fileapi::BY_HANDLE_FILE_INFORMATION;

pub fn get_win_file_id(file_path: &Path) -> Result<(i32, i64), Error> {
    // Open the file
    let file = File::open(file_path)?;

    // Prepare the BY_HANDLE_FILE_INFORMATION struct
    let mut file_info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };

    // Call GetFileInformationByHandle
    let result = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut file_info) };

    // Check the result
    if result == 0 {
        // The function failed, get the last error code
        Err(Error::last_os_error())
    } else {
        // The function succeeded, return the volume serial number and the file index
        Ok((
            file_info.dwVolumeSerialNumber as i32,
            file_info.nFileIndexHigh as i64,
        ))
    }
}
