use dashmap::DashMap;
use glob::Pattern;
use rayon::prelude::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tracing::error;

/// Parallel directory traversal. Builds a map of file_size → Vec<PathBuf>,
/// filtering by glob ignore patterns. Skips symlinks and 0-byte files.
pub fn build_size_to_files_map(
    root_paths: &[&str],
    ignore_globs: &[&str],
) -> io::Result<DashMap<u64, Vec<PathBuf>>> {
    let map: DashMap<u64, Vec<PathBuf>> = DashMap::new();

    let ignore_patterns: Vec<Pattern> = ignore_globs
        .iter()
        .filter_map(|glob| match Pattern::new(glob) {
            Ok(p) => Some(p),
            Err(e) => {
                error!("Invalid glob pattern '{}': {}", glob, e);
                None
            }
        })
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
    if !dir.is_dir() {
        return Ok(());
    }

    if ignore_patterns
        .iter()
        .any(|pattern| pattern.matches_path(dir))
    {
        return Ok(());
    }

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            if err.kind() == io::ErrorKind::PermissionDenied {
                error!(
                    "Access denied reading directory {}: {}",
                    dir.display(),
                    err
                );
                return Ok(());
            } else {
                return Err(io::Error::new(
                    err.kind(),
                    format!("Error reading directory {}: {}", dir.display(), err),
                ));
            }
        }
    };

    entries.par_bridge().try_for_each(|entry_result| {
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
                        "Error getting metadata for {}: {}",
                        path.display(),
                        err
                    ),
                ));
            }
        };

        if path.is_dir() {
            visit_dirs(&path, map, ignore_patterns)?;
        } else if !metadata.file_type().is_symlink() && metadata.len() > 0 {
            let file_size = metadata.len();
            if !ignore_patterns
                .iter()
                .any(|pattern| pattern.matches_path(&path))
            {
                map.entry(file_size).or_default().push(path.to_path_buf());
            }
        }
        Ok(())
    })?;

    Ok(())
}
