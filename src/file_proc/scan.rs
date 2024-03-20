use dashmap::DashMap;
use rayon::prelude::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn build_size_to_files_map(root_paths: &Vec<&str>) -> io::Result<DashMap<u64, Vec<PathBuf>>> {
    let map: DashMap<u64, Vec<PathBuf>> = DashMap::new();

    root_paths
        .par_iter()
        .try_for_each(|root_dir| visit_dirs(Path::new(root_dir), &map))?;

    Ok(map)
}

fn visit_dirs(dir: &Path, map: &DashMap<u64, Vec<PathBuf>>) -> io::Result<()> {
    if dir.is_dir() {
        // Read the directory entries
        let entries = fs::read_dir(dir)?;

        // Use a parallel iterator to process each entry
        entries.par_bridge().try_for_each(|entry_result| {
            // Safely handle the Result from reading the directory entry
            let entry = entry_result?;
            let path = entry.path();
            let metadata = fs::metadata(&path)?;

            // Check if the path is a directory or a non-symlink file
            if path.is_dir() {
                // Recursively visit directories
                visit_dirs(&path, map)?;
            } else if !metadata.file_type().is_symlink() && metadata.len() > 0 {
                // Only add non-symlink files to the map
                let file_size = metadata.len();
                map.entry(file_size)
                    .or_insert_with(Vec::new)
                    .push(path.to_path_buf());
            }
            Ok::<_, io::Error>(())
        })?;
    }
    Ok(())
}
