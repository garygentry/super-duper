use crate::platform;
use crate::progress::ProgressReporter;
use dashmap::{DashMap, DashSet};
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
    pub file_identity: Option<String>,
    pub warning_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocationExclusion {
    pub path: PathBuf,
    pub reason_code: String,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcludedSubtree {
    pub path: String,
    pub reason_code: String,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
}

pub struct TraversalResult {
    pub size_to_files: DashMap<u64, Vec<PathBuf>>,
    pub files: Vec<DiscoveredFile>,
    pub files_discovered: usize,
    pub bytes_discovered: u64,
    /// Logical zero-byte files observed but intentionally excluded from product results.
    pub zero_byte_files: usize,
    pub warning_count: usize,
    pub excluded_subtrees: Vec<ExcludedSubtree>,
}

pub fn discover_files(
    root_paths: &[&str],
    ignore_globs: &[&str],
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> io::Result<TraversalResult> {
    discover_files_with_exclusions(root_paths, ignore_globs, &[], cancel_token, progress)
}

pub fn discover_files_with_exclusions(
    root_paths: &[&str],
    ignore_globs: &[&str],
    location_exclusions: &[LocationExclusion],
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> io::Result<TraversalResult> {
    let map = DashMap::new();
    let seen_file_identities = DashSet::new();
    let files = Mutex::new(Vec::new());
    let warnings = AtomicUsize::new(0);
    let file_count = AtomicUsize::new(0);
    let byte_count = AtomicU64::new(0);
    let zero_byte_count = AtomicUsize::new(0);
    let excluded_subtrees = DashMap::new();

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
        if cancel_token.load(Ordering::Relaxed) {
            return Ok(());
        }
        if let Some(exclusion) = matching_exclusion(root_path, location_exclusions) {
            record_exclusion(&excluded_subtrees, root_path, exclusion);
            return Ok(());
        }
        if !root_path.is_dir() {
            warn!(
                "Scan root is not an accessible directory: {}",
                root_path.display()
            );
            warnings.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        match is_link_or_reparse(root_path) {
            Ok(true) => {
                warn!(
                    "Skipping linked or reparse-point scan root: {}",
                    root_path.display()
                );
                warnings.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
            Ok(false) => {}
            Err(error) => {
                warn!(
                    "Unable to inspect scan root {}: {}",
                    root_path.display(),
                    error
                );
                warnings.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        }
        let canonical_root = match fs::canonicalize(root_path) {
            Ok(path) => path,
            Err(error) => {
                warn!(
                    "Unable to canonicalize scan root {}: {}",
                    root_path.display(),
                    error
                );
                warnings.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        };
        visit_dirs(
            &canonical_root,
            &canonical_root,
            &map,
            &seen_file_identities,
            &files,
            &ignore_patterns,
            cancel_token,
            progress,
            &file_count,
            &byte_count,
            &zero_byte_count,
            &warnings,
            location_exclusions,
            &excluded_subtrees,
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
    let mut excluded_subtrees = excluded_subtrees
        .into_iter()
        .map(|(_, exclusion)| exclusion)
        .collect::<Vec<_>>();
    excluded_subtrees.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(TraversalResult {
        size_to_files: map,
        files,
        files_discovered: file_count.load(Ordering::Relaxed),
        bytes_discovered: byte_count.load(Ordering::Relaxed),
        zero_byte_files: zero_byte_count.load(Ordering::Relaxed),
        warning_count: warnings.load(Ordering::Relaxed),
        excluded_subtrees,
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
    seen_file_identities: &DashSet<String>,
    files: &Mutex<Vec<DiscoveredFile>>,
    ignore_patterns: &[Pattern],
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
    file_count: &AtomicUsize,
    byte_count: &AtomicU64,
    zero_byte_count: &AtomicUsize,
    warnings: &AtomicUsize,
    location_exclusions: &[LocationExclusion],
    excluded_subtrees: &DashMap<String, ExcludedSubtree>,
) -> io::Result<()> {
    if cancel_token.load(Ordering::Relaxed) {
        return Ok(());
    }
    if let Some(exclusion) = matching_exclusion(dir, location_exclusions) {
        record_exclusion(excluded_subtrees, dir, exclusion);
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
        if let Some(exclusion) = matching_exclusion(&path, location_exclusions) {
            record_exclusion(excluded_subtrees, &path, exclusion);
            return Ok(());
        }
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
        match is_link_or_reparse(&path) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(error) => {
                warn!(
                    "Unable to inspect {} for reparse metadata: {}",
                    path.display(),
                    error
                );
                warnings.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        }
        if file_type.is_dir() {
            return visit_dirs(
                root,
                &path,
                map,
                seen_file_identities,
                files,
                ignore_patterns,
                cancel_token,
                progress,
                file_count,
                byte_count,
                zero_byte_count,
                warnings,
                location_exclusions,
                excluded_subtrees,
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
            zero_byte_count.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }

        let mut warning_messages = Vec::new();
        let canonical = match fs::canonicalize(&path) {
            Ok(path) => path,
            Err(error) => {
                warn!(
                    "Unable to canonicalize discovered path {}: {}",
                    path.display(),
                    error
                );
                warnings.fetch_add(1, Ordering::Relaxed);
                warning_messages.push(format!("Unable to canonicalize discovered path: {error}"));
                path.clone()
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
        let last_modified = match metadata.modified().and_then(|time| {
            time.duration_since(UNIX_EPOCH)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        }) {
            Ok(duration) => duration.as_nanos().min(i64::MAX as u128) as i64,
            Err(error) => {
                warn!(
                    "Unable to read modified time for {}: {}",
                    path.display(),
                    error
                );
                warnings.fetch_add(1, Ordering::Relaxed);
                warning_messages.push(format!("Unable to read modified time: {error}"));
                0
            }
        };
        let file_identity = match platform::file_identity(&canonical) {
            Ok(identity) => identity,
            Err(error) => {
                warn!(
                    "Unable to read file identity for {}: {}",
                    path.display(),
                    error
                );
                warnings.fetch_add(1, Ordering::Relaxed);
                warning_messages.push(format!("Unable to read stable file identity: {error}"));
                None
            }
        };
        let first_physical_file = file_identity.as_ref().map_or(true, |identity| {
            seen_file_identities.insert(identity.clone())
        });

        if first_physical_file {
            map.entry(metadata.len()).or_default().push(path);
        }
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
                file_identity,
                warning_message: (!warning_messages.is_empty())
                    .then(|| warning_messages.join("; ")),
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

fn matching_exclusion<'a>(
    path: &Path,
    exclusions: &'a [LocationExclusion],
) -> Option<&'a LocationExclusion> {
    exclusions
        .iter()
        .filter(|exclusion| path_is_within(path, &exclusion.path))
        .max_by_key(|exclusion| exclusion.path.components().count())
}

fn record_exclusion(
    exclusions: &DashMap<String, ExcludedSubtree>,
    path: &Path,
    exclusion: &LocationExclusion,
) {
    let path = path.to_string_lossy().into_owned();
    exclusions
        .entry(path_key(Path::new(&path)))
        .or_insert_with(|| ExcludedSubtree {
            path,
            reason_code: exclusion.reason_code.clone(),
            provider_id: exclusion.provider_id.clone(),
            provider_name: exclusion.provider_name.clone(),
        });
}

#[cfg(windows)]
fn path_is_within(path: &Path, ancestor: &Path) -> bool {
    let path = path_key(path);
    let ancestor = path_key(ancestor);
    path == ancestor
        || path
            .strip_prefix(&ancestor)
            .is_some_and(|suffix| suffix.starts_with('\\'))
}

#[cfg(not(windows))]
fn path_is_within(path: &Path, ancestor: &Path) -> bool {
    path.starts_with(ancestor)
}

#[cfg(windows)]
fn path_key(path: &Path) -> String {
    let value = path.to_string_lossy().replace('/', "\\");
    let value = if let Some(unc) = value.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{unc}")
    } else if let Some(dos) = value.strip_prefix("\\\\?\\") {
        dos.to_owned()
    } else {
        value
    };
    value.trim_end_matches('\\').to_lowercase()
}

#[cfg(not(windows))]
fn path_key(path: &Path) -> String {
    path.to_string_lossy().trim_end_matches('/').to_owned()
}

pub fn is_link_or_reparse(path: &Path) -> io::Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Ok(true);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return Ok(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0);
    }
    #[cfg(not(windows))]
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::SilentReporter;
    use std::sync::atomic::AtomicBool;
    use tempfile::tempdir;

    #[test]
    fn broad_root_prunes_excluded_subtree_before_discovery() {
        let temp = tempdir().unwrap();
        let local = temp.path().join("local");
        let cloud = temp.path().join("cloud");
        fs::create_dir_all(&local).unwrap();
        fs::create_dir_all(&cloud).unwrap();
        fs::write(local.join("kept.bin"), b"local").unwrap();
        fs::write(cloud.join("placeholder.bin"), b"cloud").unwrap();
        let root = temp.path().to_string_lossy().into_owned();
        let result = discover_files_with_exclusions(
            &[&root],
            &[],
            &[LocationExclusion {
                path: cloud.clone(),
                reason_code: "registered_cloud_root_excluded".to_owned(),
                provider_id: Some("provider".to_owned()),
                provider_name: Some("Test cloud".to_owned()),
            }],
            &AtomicBool::new(false),
            &SilentReporter,
        )
        .unwrap();

        assert_eq!(result.files_discovered, 1);
        assert!(result.files[0].canonical_path.contains("kept.bin"));
        assert_eq!(result.excluded_subtrees.len(), 1);
        assert!(paths_equal_for_test(
            Path::new(&result.excluded_subtrees[0].path),
            &cloud
        ));
    }

    #[test]
    fn root_inside_exclusion_is_classified_without_touching_filesystem() {
        let temp = tempdir().unwrap();
        let cloud = temp.path().join("unavailable-cloud");
        let selected = cloud.join("selected-root");
        let selected_text = selected.to_string_lossy().into_owned();
        let result = discover_files_with_exclusions(
            &[&selected_text],
            &[],
            &[LocationExclusion {
                path: cloud,
                reason_code: "registered_cloud_root_excluded".to_owned(),
                provider_id: None,
                provider_name: None,
            }],
            &AtomicBool::new(false),
            &SilentReporter,
        )
        .unwrap();

        assert_eq!(result.files_discovered, 0);
        assert_eq!(result.warning_count, 0);
        assert_eq!(result.excluded_subtrees.len(), 1);
        assert!(paths_equal_for_test(
            Path::new(&result.excluded_subtrees[0].path),
            &selected
        ));
    }

    #[cfg(windows)]
    fn paths_equal_for_test(left: &Path, right: &Path) -> bool {
        path_key(left) == path_key(right)
    }

    #[cfg(not(windows))]
    fn paths_equal_for_test(left: &Path, right: &Path) -> bool {
        left == right
    }
}
