use super::model::FileInfo;
use dashmap::DashMap;
use rayon::prelude::*;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Component;
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
fn get_drive_letter(path: &Path) -> Option<OsString> {
    for component in path.components() {
        if let Component::Prefix(prefix_comp) = component {
            match prefix_comp.kind() {
                std::path::Prefix::Disk(letter) | std::path::Prefix::VerbatimDisk(letter) => {
                    // Convert the drive letter to a char, then create a String, and finally convert to OsString
                    let drive_letter = (letter as char).to_string();
                    return Some(OsString::from(drive_letter));
                }
                // Handle other prefix types if necessary
                _ => (),
            }
        }
    }
    None
}

fn get_path_without_drive_letter(path: &Path) -> PathBuf {
    let components: Vec<_> = path.components().collect();

    // Skip the first component if it's a prefix (like a drive letter on Windows).
    let without_drive = components
        .iter()
        .skip_while(|comp| matches!(comp, Component::Prefix(_)));

    // Rebuild the path from the remaining components.
    let mut result_path = PathBuf::new();
    for component in without_drive {
        result_path.push(component.as_os_str());
    }

    result_path
}

#[cfg(not(target_os = "windows"))]
fn get_drive_letter(_path: &PathBuf) -> Option<OsString> {
    // Drive letters do not exist on Unix-like operating systems
    None
}

pub fn build_file_info_vec(
    content_hash_map: DashMap<u64, Vec<PathBuf>>,
) -> io::Result<Vec<FileInfo>> {
    let entries: Vec<_> = content_hash_map.iter().collect();

    let file_info_vec: Result<Vec<_>, io::Error> = entries
        .par_iter()
        .flat_map(|entry| {
            let hash = *entry.key();
            let paths = entry.value();

            paths.par_iter().map(move |path| {
                let metadata = fs::metadata(path)?;

                // Wrap the operation that might panic in a closure and call `catch_unwind` on it
                // let result = panic::catch_unwind(AssertUnwindSafe(|| get_win_file_id(path)));

                // let (volume_serial_number, file_index) = match result {
                //     Ok(Ok(res)) => res,
                //     Ok(Err(e)) => {
                //         // Convert the io::Error into an `io::Error`
                //         return Err(io::Error::new(
                //             io::ErrorKind::Other,
                //             format!("IO error in get_win_file_id: {}", e),
                //         ));
                //     }
                //     Err(_) => {
                //         // Convert the panic into an `io::Error`
                //         return Err(io::Error::new(
                //             io::ErrorKind::Other,
                //             "Panic occurred in get_win_file_id",
                //         ));
                //     }
                // };

                // let path_str = path.to_string_lossy();
                // println!("path_str: {}", path_str);
                // let drive_letter = path_str.get(0..1).map(String::from).unwrap_or_default();
                // let drive_letter = get_drive_letter(&path)?;

                let canonical_path = fs::canonicalize(path)?;

                let drive_letter = match get_drive_letter(&canonical_path) {
                    Some(drive) => drive.to_string_lossy().into_owned(),
                    None => "No drive letter found or not applicable.".to_string(),
                };

                // println!("drive letter: {}", drive_letter);

                // let path_no_drive = path_str.get(3..).unwrap_or_default().to_string();
                let path_no_drive = get_path_without_drive_letter(&canonical_path)
                    .to_string_lossy()
                    .into_owned();

                let parent_dir = canonical_path
                    .parent()
                    .and_then(|p| p.to_str().map(String::from))
                    .expect("Failed to get parent directory or convert to string");

                let file_info = FileInfo {
                    // canonical_name: fs::canonicalize(path)?.to_string_lossy().into_owned(),
                    canonical_name: canonical_path.to_string_lossy().into_owned(),
                    file_size: metadata.len() as i64,
                    last_modified: metadata.modified()?,
                    content_hash: hash as i64,
                    // volume_serial_number,
                    // file_index,
                    drive_letter,
                    path_no_drive,
                    parent_dir,
                };

                Ok(file_info)
            })
        })
        .collect();

    file_info_vec
}
