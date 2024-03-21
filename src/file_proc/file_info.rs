use super::model::FileInfo;
use super::win::get_win_file_id;
use dashmap::DashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

pub fn build_file_info_vec(
    content_hash_map: DashMap<u64, Vec<PathBuf>>,
) -> io::Result<Vec<FileInfo>> {
    let mut file_info_vec = Vec::new();

    for entry in content_hash_map.iter() {
        let hash = *entry.key();
        let paths = entry.value();

        for path in paths {
            let metadata = fs::metadata(path)?;

            let (volume_serial_number, file_index) = get_win_file_id(path);

            let path_str = path.to_string_lossy();
            let drive_letter = path_str
                .get(0..1)
                .map(String::from)
                .unwrap_or_default()
                .to_string();
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

            file_info_vec.push(file_info);
        }
    }

    Ok(file_info_vec)
}
