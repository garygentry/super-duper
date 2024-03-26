#![allow(dead_code)]
use dashmap::DashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn build_directory_sizes_map(root_paths: &[&str]) -> io::Result<DashMap<u64, Vec<PathBuf>>> {
    let map: DashMap<u64, Vec<PathBuf>> = DashMap::new();

    root_paths
        .iter()
        .try_for_each(|root_dir| visit_dirs(Path::new(root_dir), &map))?;

    Ok(map)
}

fn visit_dirs(dir: &Path, map: &DashMap<u64, Vec<PathBuf>>) -> io::Result<()> {
    if dir.is_dir() {
        let mut total_size = 0;

        // Read the directory entries
        let entries = fs::read_dir(dir)?;

        // Collect directories and their sizes
        let mut sub_dirs: Vec<PathBuf> = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::metadata(&path)?;

            if metadata.is_dir() {
                sub_dirs.push(path);
            } else {
                total_size += metadata.len();
            }
        }

        // Add the directory to the map if it contains any files
        if total_size > 0 {
            map.entry(total_size).or_default().push(dir.to_path_buf());
        }

        // Recursively visit subdirectories
        for sub_dir in sub_dirs {
            visit_dirs(&sub_dir, map)?;
        }
    }
    Ok(())
}
