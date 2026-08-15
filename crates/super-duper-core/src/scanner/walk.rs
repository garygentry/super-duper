use crate::progress::ProgressReporter;
use dashmap::DashMap;
use glob::Pattern;
use rayon::prelude::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub root_path: String,
    pub canonical_path: String,
    pub relative_path: String,
    pub file_name: String,
    pub parent_dir: String,
    pub file_size: u64,
    pub last_modified: i64,
    pub warning_message: Option<String>,
}

pub struct TraversalResult {
    pub size_to_files: DashMap<u64, Vec<PathBuf>>,
    pub files: Vec<DiscoveredFile>,
    pub files_discovered: usize,
    pub bytes_discovered: u64,
    pub warning_count: usize,
}

pub fn discover_files(
    root_paths: &[&str],
    ignore_globs: &[&str],
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> io::Result<TraversalResult> {
    let map = DashMap::new();
    let files = Mutex::new(Vec::new());
    let warnings = AtomicUsize::new(0);
    let file_count = AtomicUsize::new(0);
    let byte_count = AtomicU64::new(0);

    let ignore_patterns: Vec<Pattern> = ignore_globs
        .iter()
        .filter_map(|glob| match Pattern::new(glob) {
            Ok(pattern) => Some(pattern),
            Err(error) => {
                warn!("Invalid glob pattern '{}': {}", glob, error);
                warnings.fetch_add(1, Ordering::Relaxed);
                None
            }
        })
        .collect();

    root_paths.par_iter().try_for_each(|root| {
        let root_path = Path::new(root);
        if !root_path.is_dir() {
            warn!(
                "Scan root is not an accessible directory: {}",
                root_path.display()
            );
            warnings.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        visit_dirs(
            root_path,
            root_path,
            &map,
            &files,
            &ignore_patterns,
            cancel_token,
            progress,
            &file_count,
            &byte_count,
            &warnings,
        )
    })?;

    let files = files
        .into_inner()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "discovery result lock poisoned"))?;
    progress.on_discovery_progress(
        file_count.load(Ordering::Relaxed),
        byte_count.load(Ordering::Relaxed),
        warnings.load(Ordering::Relaxed),
        "",
    );
    Ok(TraversalResult {
        size_to_files: map,
        files,
        files_discovered: file_count.load(Ordering::Relaxed),
        bytes_discovered: byte_count.load(Ordering::Relaxed),
        warning_count: warnings.load(Ordering::Relaxed),
    })
}

/// Compatibility wrapper retained for callers that only need the size buckets.
pub fn build_size_to_files_map(
    root_paths: &[&str],
    ignore_globs: &[&str],
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> io::Result<DashMap<u64, Vec<PathBuf>>> {
    Ok(discover_files(root_paths, ignore_globs, cancel_token, progress)?.size_to_files)
}

#[allow(clippy::too_many_arguments)]
fn visit_dirs(
    root: &Path,
    dir: &Path,
    map: &DashMap<u64, Vec<PathBuf>>,
    files: &Mutex<Vec<DiscoveredFile>>,
    ignore_patterns: &[Pattern],
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
    file_count: &AtomicUsize,
    byte_count: &AtomicU64,
    warnings: &AtomicUsize,
) -> io::Result<()> {
    if cancel_token.load(Ordering::Relaxed)
        || ignore_patterns
            .iter()
            .any(|pattern| pattern.matches_path(dir))
    {
        return Ok(());
    }

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            warn!("Unable to read directory {}: {}", dir.display(), error);
            warnings.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
    };

    entries.par_bridge().try_for_each(|entry_result| {
        if cancel_token.load(Ordering::Relaxed) {
            return Ok(());
        }
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                warn!("Unable to read an entry in {}: {}", dir.display(), error);
                warnings.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                warn!("Unable to inspect {}: {}", path.display(), error);
                warnings.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        };
        if file_type.is_symlink() {
            return Ok(());
        }
        if file_type.is_dir() {
            return visit_dirs(
                root,
                &path,
                map,
                files,
                ignore_patterns,
                cancel_token,
                progress,
                file_count,
                byte_count,
                warnings,
            );
        }
        if !file_type.is_file()
            || ignore_patterns
                .iter()
                .any(|pattern| pattern.matches_path(&path))
        {
            return Ok(());
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                warn!("Unable to read metadata for {}: {}", path.display(), error);
                warnings.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        };
        if metadata.len() == 0 {
            return Ok(());
        }

        let (canonical, warning_message) = match fs::canonicalize(&path) {
            Ok(path) => (path, None),
            Err(error) => {
                warnings.fetch_add(1, Ordering::Relaxed);
                (
                    path.clone(),
                    Some(format!("Unable to canonicalize discovered path: {error}")),
                )
            }
        };
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        let canonical_path = canonical.to_string_lossy().into_owned();
        let parent_dir = canonical
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file_name = canonical
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let last_modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
            .unwrap_or(0);

        map.entry(metadata.len()).or_default().push(path);
        files
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "discovery result lock poisoned"))?
            .push(DiscoveredFile {
                root_path: root.to_string_lossy().into_owned(),
                canonical_path,
                relative_path,
                file_name,
                parent_dir,
                file_size: metadata.len(),
                last_modified,
                warning_message,
            });
        byte_count.fetch_add(metadata.len(), Ordering::Relaxed);
        let count = file_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count % 256 == 0 {
            progress.on_discovery_progress(
                count,
                byte_count.load(Ordering::Relaxed),
                warnings.load(Ordering::Relaxed),
                &canonical.to_string_lossy(),
            );
        }
        Ok(())
    })
}
