use super::status::*;
use crate::utils;
use dashmap::DashMap;
use glob::Pattern;
use rayon::prelude::*;
use std::fs;
use std::fs::Metadata;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracing::*;

#[derive(Debug, Clone)]
pub struct ScanFile {
    pub path_buf: PathBuf,
    pub file_size: u64,
    pub metadata: Metadata,
}

/// Builds a map of file sizes to the corresponding file paths.
///
/// This function takes a list of root paths and a list of ignore globs as input.
/// It recursively scans the directories starting from the root paths and builds a map
/// where the keys are file sizes (in bytes) and the values are vectors of file paths
/// with the corresponding size.
///
/// # Arguments
///
/// * `root_paths` - A slice of strings representing the root paths to start scanning from.
/// * `ignore_globs` - A slice of strings representing the glob patterns to ignore during scanning.
///
/// # Returns
///
/// Returns an `io::Result` containing a `DashMap` where the keys are file sizes (in bytes)
/// and the values are vectors of `PathBuf` representing the file paths with the corresponding size.
pub fn build_size_to_files_map(
    root_paths: &[String],
    ignore_globs: &[String],
    tx_status: &Arc<dyn Fn(StatusMessage) + Send + Sync>
) -> io::Result<DashMap<u64, Vec<ScanFile>>> {
    tx_status(StatusMessage::ScanBegin);

    let map: DashMap<u64, Vec<ScanFile>> = DashMap::new();
    // make sure no root paths overlap
    let root_paths = utils::to_non_overlapping_directories(root_paths);

    // Compile the glob patterns for paths to ignore
    let ignore_patterns: Vec<Pattern> = ignore_globs
        .iter()
        .map(|glob| Pattern::new(glob).unwrap())
        .collect();

    // iterate files in root paths indexed on file size
    root_paths
        .par_iter()
        .try_for_each(|root_dir| {
            visit_dirs(Path::new(root_dir), &map, &ignore_patterns, tx_status)
        })?;

    // remove entries with only 1 file (not dupes)
    // map.retain(|_, vec_scanfile| vec_scanfile.len() > 1);
    map.retain(|&index, vec_scanfile| {
        if vec_scanfile.len() > 1 {
            tx_status(
                StatusMessage::ScanAddDupe(ScanAddDupeStatusMessage {
                    count: vec_scanfile.len(),
                    file_size: index,
                })
            );
            true // Keep the entry in the map
        } else {
            false // Remove the entry from the map
        }
    });

    tx_status(StatusMessage::ScanEnd);
    Ok(map)
}

/// Recursively visits directories and adds non-ignored files to the map.
///
/// This function is called recursively to visit directories starting from the given `dir` path.
/// It checks if the directory matches any ignore patterns and skips further processing if it does.
/// Otherwise, it reads the directory entries and processes each entry in parallel.
/// Only non-symlink files with a size greater than 0 are added to the map.
///
/// # Arguments
///
/// * `dir` - A `Path` representing the directory to visit.
/// * `map` - A reference to a `DashMap` where the file sizes and paths are stored.
/// * `ignore_patterns` - A slice of `Pattern` representing the glob patterns to ignore.
///
/// # Returns
///
/// Returns an `io::Result` indicating the success or failure of the operation.
fn visit_dirs(
    dir: &Path,
    map: &DashMap<u64, Vec<ScanFile>>,
    ignore_patterns: &[Pattern],
    tx_status: &Arc<dyn Fn(StatusMessage) + Send + Sync>
) -> io::Result<()> {
    if dir.is_dir() {
        // Check if the directory matches any ignore patterns
        if ignore_patterns.iter().any(|pattern| pattern.matches_path(dir)) {
            // Skip further processing of the directory
            return Ok(());
        }

        // Read the directory entries
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) => {
                if err.kind() == io::ErrorKind::PermissionDenied {
                    error!("Access denied error reading directory {}: {}", dir.display(), err);
                    return Ok(()); // Skip further processing of the directory
                } else {
                    return Err(
                        io::Error::new(
                            err.kind(),
                            format!("Error reading directory {}: {}", dir.display(), err)
                        )
                    );
                }
            }
        };

        // Use a parallel iterator to process each entry
        entries.par_bridge().try_for_each(|entry_result| {
            // Safely handle the Result from reading the directory entry
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(err) => {
                    return Err(
                        io::Error::new(
                            err.kind(),
                            format!("Error reading entry in directory {}: {}", dir.display(), err)
                        )
                    );
                }
            };

            // TODO: Remove this sleep after testing
            thread::sleep(Duration::from_millis(1));

            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(err) => {
                    return Err(
                        io::Error::new(
                            err.kind(),
                            format!("Error getting metadata for file {}: {}", path.display(), err)
                        )
                    );
                }
            };

            // Check if the path is a directory or a non-symlink file
            if metadata.is_dir() {
                // Recursively visit directories
                visit_dirs(&path, map, ignore_patterns, tx_status)?;
            } else if !metadata.file_type().is_symlink() && metadata.len() > 0 {
                // Only add non-symlink files to the map
                let file_size = metadata.len();

                // Check if the file matches any of the glob patterns
                if !ignore_patterns.iter().any(|pattern| pattern.matches_path(&path)) {
                    let scan_file = ScanFile {
                        path_buf: path.to_path_buf(),
                        file_size,
                        metadata,
                    };

                    tx_status(
                        StatusMessage::ScanAddRaw(ScanAddRawStatusMessage {
                            file_path: path.to_path_buf(),
                            file_size: scan_file.file_size,
                        })
                    );

                    map.entry(file_size).or_default().push(scan_file);
                }
            } else {
                // Skip symlinks and files with size 0
                // debug!("Skipping (symlink or file size 0): {}", path.display());
            }
            Ok(())
        })?;
    } else {
        // Skip non-directory paths
        error!("Skipping (not a directory): {}", dir.display());
    }
    Ok(())
}
