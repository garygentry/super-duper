use dashmap::DashMap;
use glob::Pattern;
use rayon::prelude::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tracing::error;

pub fn build_size_to_files_map(
    root_paths: &Vec<&str>,
    ignore_globs: &[&str],
) -> io::Result<DashMap<u64, Vec<PathBuf>>> {
    let map: DashMap<u64, Vec<PathBuf>> = DashMap::new();

    // Compile the glob patterns
    let ignore_patterns: Vec<Pattern> = ignore_globs
        .iter()
        .map(|glob| Pattern::new(glob).unwrap())
        .collect();

    root_paths
        .par_iter()
        .try_for_each(|root_dir| visit_dirs(Path::new(root_dir), &map, &ignore_patterns))?;

    Ok(map)
}

fn visit_dirs(
    dir: &Path,
    map: &DashMap<u64, Vec<PathBuf>>,
    ignore_patterns: &[Pattern],
) -> io::Result<()> {
    if dir.is_dir() {
        // Check if the directory matches any ignore patterns
        if ignore_patterns
            .iter()
            .any(|pattern| pattern.matches_path(dir))
        {
            // Skip further processing of the directory
            // error!("Ignoring: {}", dir.display());
            return Ok(());
        }

        // Read the directory entries
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) => {
                if err.kind() == io::ErrorKind::PermissionDenied {
                    error!(
                        "Access denied error reading directory {}: {}",
                        dir.display(),
                        err
                    );
                    return Ok(()); // Skip further processing of the directory
                } else {
                    return Err(io::Error::new(
                        err.kind(),
                        format!("Error reading directory {}: {}", dir.display(), err),
                    ));
                }
            }
        };

        // Use a parallel iterator to process each entry
        entries.par_bridge().try_for_each(|entry_result| {
            // Safely handle the Result from reading the directory entry
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(err) => {
                    return Err(io::Error::new(
                        err.kind(),
                        format!(
                            "Error reading entry in directory {}: {}",
                            dir.display(),
                            err
                        ),
                    ));
                }
            };

            let path = entry.path();
            let metadata = match fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(err) => {
                    return Err(io::Error::new(
                        err.kind(),
                        format!(
                            "Error getting metadata for file {}: {}",
                            path.display(),
                            err
                        ),
                    ));
                }
            };

            // print_status(path.as_path().to_str().unwrap_or_default());

            // Check if the path is a directory or a non-symlink file
            if path.is_dir() {
                // Recursively visit directories
                visit_dirs(&path, map, ignore_patterns)?;
            } else if !metadata.file_type().is_symlink() && metadata.len() > 0 {
                // Only add non-symlink files to the map
                let file_size = metadata.len();

                // Check if the file matches any of the glob patterns
                if !ignore_patterns
                    .iter()
                    .any(|pattern| pattern.matches_path(&path))
                {
                    map.entry(file_size).or_default().push(path.to_path_buf());
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

// fn visit_dirs(
//     dir: &Path,
//     map: &DashMap<u64, Vec<PathBuf>>,
//     ignore_patterns: &Vec<Pattern>,
// ) -> io::Result<()> {
//     if dir.is_dir() {
//         // Read the directory entries
//         let entries = match fs::read_dir(dir) {
//             Ok(entries) => entries,
//             Err(err) => {
//                 if err.kind() == io::ErrorKind::PermissionDenied {
//                     error!(
//                         "Access denied error reading directory {}: {}",
//                         dir.display(),
//                         err
//                     );
//                     return Ok(()); // Skip further processing of the directory
//                 } else {
//                     return Err(io::Error::new(
//                         err.kind(),
//                         format!("Error reading directory {}: {}", dir.display(), err),
//                     ));
//                 }
//             }
//         };

//         // Use a parallel iterator to process each entry
//         entries.par_bridge().try_for_each(|entry_result| {
//             // Safely handle the Result from reading the directory entry
//             let entry = match entry_result {
//                 Ok(entry) => entry,
//                 Err(err) => {
//                     return Err(io::Error::new(
//                         err.kind(),
//                         format!(
//                             "Error reading entry in directory {}: {}",
//                             dir.display(),
//                             err
//                         ),
//                     ));
//                 }
//             };

//             let path = entry.path();
//             let metadata = match fs::metadata(&path) {
//                 Ok(metadata) => metadata,
//                 Err(err) => {
//                     return Err(io::Error::new(
//                         err.kind(),
//                         format!(
//                             "Error getting metadata for file {}: {}",
//                             path.display(),
//                             err
//                         ),
//                     ));
//                 }
//             };

//             // Call trace!() to output the name for each file or directory
//             // trace!("Processing {:?}", path.display());

//             // Check if the path is a directory or a non-symlink file
//             if path.is_dir() {
//                 // Recursively visit directories
//                 visit_dirs(&path, map, ignore_patterns)?;
//             } else if !metadata.file_type().is_symlink() && metadata.len() > 0 {
//                 // Only add non-symlink files to the map
//                 let file_size = metadata.len();

//                 // Check if the file matches any of the glob patterns
//                 if !ignore_patterns
//                     .iter()
//                     .any(|pattern| pattern.matches_path(&path))
//                 {
//                     map.entry(file_size).or_default().push(path.to_path_buf());
//                 }
//             }
//             Ok(())
//         })?;
//     }
//     Ok(())
// }
