use super::model::FileInfo;
use super::win::get_win_file_id;
use dashmap::DashMap;
use rayon::prelude::*;
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;

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
                let result = panic::catch_unwind(AssertUnwindSafe(|| get_win_file_id(path)));

                let (volume_serial_number, file_index) = match result {
                    Ok(Ok(res)) => res,
                    Ok(Err(e)) => {
                        // Convert the io::Error into an `io::Error`
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            format!("IO error in get_win_file_id: {}", e),
                        ));
                    }
                    Err(_) => {
                        // Convert the panic into an `io::Error`
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            "Panic occurred in get_win_file_id",
                        ));
                    }
                };

                let path_str = path.to_string_lossy();
                let drive_letter = path_str.get(0..1).map(String::from).unwrap_or_default();
                let path_no_drive = path_str.get(3..).unwrap_or_default().to_string();

                let parent_dir = path
                    .parent()
                    .and_then(|p| p.to_str().map(String::from))
                    .expect("Failed to get parent directory or convert to string");

                let file_info = FileInfo {
                    canonical_name: fs::canonicalize(path)?.to_string_lossy().into_owned(),
                    file_size: metadata.len() as i64,
                    last_modified: metadata.modified()?,
                    content_hash: hash as i64,
                    volume_serial_number,
                    file_index,
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
