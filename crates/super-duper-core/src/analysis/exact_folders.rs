use crate::hasher::cache;
use crate::progress::ProgressReporter;
use crate::storage::models::{ExactFolderGroupInsert, ScannedFile};
use crate::storage::Database;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::hash::Hasher as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;
use twox_hash::XxHash64;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExactFolderAnalysis {
    pub visible_groups: usize,
    pub retained_groups: usize,
    pub warning_count: usize,
}

#[derive(Debug, Clone)]
struct CandidateFile {
    id: i64,
    canonical_path: String,
    relative_path: String,
    size: i64,
    last_modified: i64,
    content_hash: Option<i64>,
    file_identity: Option<String>,
}

#[derive(Debug)]
struct CandidateDirectory {
    path: String,
    files: Vec<CandidateFile>,
}

#[derive(Debug)]
struct VerifiedGroup {
    structural_fingerprint: String,
    verified_fingerprint: String,
    total_size: i64,
    file_count: i64,
    candidates: Vec<usize>,
    suppressed: bool,
}

/// Find exact duplicate folders in two phases: cheap relative-path/size candidates followed by
/// full-content verification. Every persisted group and directory member is owned by `run_id`.
pub fn analyze_exact_folders_cancellable(
    db: &Database,
    run_id: i64,
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> Result<ExactFolderAnalysis, crate::Error> {
    check_cancelled(cancel_token)?;
    let files = db.get_scanned_files(run_id)?;
    let mut candidates = build_candidates(files);
    let total_candidates = candidates.len();
    let mut structural_sets: BTreeMap<Vec<(String, i64)>, Vec<usize>> = BTreeMap::new();
    for (index, candidate) in candidates.iter_mut().enumerate() {
        check_cancelled(cancel_token)?;
        candidate.files.sort_by(|left, right| {
            left.relative_path
                .cmp(&right.relative_path)
                .then(left.id.cmp(&right.id))
        });
        let structure = candidate
            .files
            .iter()
            .map(|file| (file.relative_path.clone(), file.size))
            .collect::<Vec<_>>();
        structural_sets.entry(structure).or_default().push(index);
        progress.on_dir_analysis_progress(index + 1, total_candidates);
    }

    let mut known_hashes = HashMap::<i64, i64>::new();
    let mut warning_count = 0usize;
    let mut verified_groups = Vec::new();
    for (structure, candidate_indexes) in structural_sets
        .into_iter()
        .filter(|(_, indexes)| indexes.len() > 1)
    {
        check_cancelled(cancel_token)?;
        let structural_fingerprint = fingerprint_structure(&structure);
        let mut verified_sets: BTreeMap<Vec<(String, i64)>, Vec<usize>> = BTreeMap::new();
        for candidate_index in candidate_indexes {
            let mut verified = Vec::with_capacity(candidates[candidate_index].files.len());
            let mut valid = true;
            for file in &candidates[candidate_index].files {
                check_cancelled(cancel_token)?;
                if let Err(error) = validate_candidate_metadata(file) {
                    warning_count += 1;
                    tracing::warn!(
                        "Unable to verify exact-folder candidate file {}: {error}",
                        file.canonical_path
                    );
                    valid = false;
                    break;
                }
                let hash = if let Some(hash) = known_hashes.get(&file.id).copied() {
                    Some(hash)
                } else if let Some(hash) = file.content_hash {
                    known_hashes.insert(file.id, hash);
                    Some(hash)
                } else {
                    match hash_candidate_file(file, cancel_token) {
                        Ok((hash, cache_warning)) => {
                            warning_count += usize::from(cache_warning);
                            db.update_scanned_file_content_hash(run_id, file.id, hash)?;
                            known_hashes.insert(file.id, hash);
                            Some(hash)
                        }
                        Err(_error) if cancel_token.load(Ordering::Relaxed) => {
                            return Err(crate::Error::Cancelled)
                        }
                        Err(error) => {
                            warning_count += 1;
                            tracing::warn!(
                                "Unable to verify exact-folder candidate file {}: {error}",
                                file.canonical_path
                            );
                            None
                        }
                    }
                };
                let Some(hash) = hash else {
                    valid = false;
                    break;
                };
                verified.push((file.relative_path.clone(), hash));
            }
            if valid {
                verified_sets
                    .entry(verified)
                    .or_default()
                    .push(candidate_index);
            }
        }

        for (verified, indexes) in verified_sets.into_iter() {
            let indexes = distinct_physical_candidates(&candidates, indexes);
            if indexes.len() < 2 {
                continue;
            }
            verified_groups.push(VerifiedGroup {
                structural_fingerprint: structural_fingerprint.clone(),
                verified_fingerprint: fingerprint_verified(&verified),
                total_size: structure.iter().map(|(_, size)| size).sum(),
                file_count: structure.len() as i64,
                candidates: indexes,
                suppressed: false,
            });
        }
    }

    suppress_nested_groups(&candidates, &mut verified_groups);
    verified_groups.sort_by(|left, right| {
        left.suppressed
            .cmp(&right.suppressed)
            .then(right.total_size.cmp(&left.total_size))
            .then_with(|| {
                candidates[left.candidates[0]]
                    .path
                    .to_lowercase()
                    .cmp(&candidates[right.candidates[0]].path.to_lowercase())
            })
    });
    let inserts = verified_groups
        .iter()
        .map(|group| {
            let directory_ids = group
                .candidates
                .iter()
                .map(|index| db.get_directory_id(run_id, &candidates[*index].path))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ExactFolderGroupInsert {
                structural_fingerprint: group.structural_fingerprint.clone(),
                verified_fingerprint: group.verified_fingerprint.clone(),
                total_size: group.total_size,
                file_count: group.file_count,
                directory_ids,
                is_suppressed: group.suppressed,
            })
        })
        .collect::<Result<Vec<_>, rusqlite::Error>>()?;
    let visible_groups = db.replace_exact_folder_groups(run_id, &inserts, cancel_token)?;
    Ok(ExactFolderAnalysis {
        visible_groups,
        retained_groups: inserts.len(),
        warning_count,
    })
}

fn build_candidates(files: Vec<ScannedFile>) -> Vec<CandidateDirectory> {
    let mut by_path = BTreeMap::<String, CandidateDirectory>::new();
    for file in files {
        if file.root_path.is_empty() {
            continue;
        }
        let components = normalized_components(&file.relative_path);
        if components.is_empty() {
            continue;
        }
        for prefix_length in 0..components.len() {
            let mut directory_path = PathBuf::from(&file.root_path);
            for component in &components[..prefix_length] {
                directory_path.push(component);
            }
            let display_path = directory_path.to_string_lossy().into_owned();
            let key = normalize_absolute(&display_path);
            by_path
                .entry(key)
                .or_insert_with(|| CandidateDirectory {
                    path: display_path,
                    files: Vec::new(),
                })
                .files
                .push(CandidateFile {
                    id: file.id,
                    canonical_path: file.canonical_path.clone(),
                    relative_path: components[prefix_length..].join("/").to_lowercase(),
                    size: file.file_size,
                    last_modified: file.last_modified,
                    content_hash: file.content_hash,
                    file_identity: file.file_identity.clone(),
                });
        }
    }
    by_path.into_values().collect()
}

fn distinct_physical_candidates(
    candidates: &[CandidateDirectory],
    mut indexes: Vec<usize>,
) -> Vec<usize> {
    indexes.sort_by(|left, right| {
        candidates[*left]
            .path
            .to_lowercase()
            .cmp(&candidates[*right].path.to_lowercase())
    });
    let mut seen = HashSet::new();
    indexes
        .into_iter()
        .filter(|index| {
            let identities = candidates[*index]
                .files
                .iter()
                .filter_map(|file| file.file_identity.as_ref())
                .collect::<Vec<_>>();
            if identities.iter().any(|identity| seen.contains(*identity)) {
                return false;
            }
            seen.extend(identities.into_iter().cloned());
            true
        })
        .collect()
}

fn normalized_components(path: &str) -> Vec<String> {
    path.replace('\\', "/")
        .split('/')
        .filter(|component| !component.is_empty() && *component != ".")
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_absolute(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

fn hash_candidate_file(
    file: &CandidateFile,
    cancel_token: &AtomicBool,
) -> std::io::Result<(i64, bool)> {
    validate_candidate_metadata(file)?;
    let outcome =
        cache::get_content_hash_cancellable(Path::new(&file.canonical_path), cancel_token)?;
    Ok((outcome.hash as i64, outcome.warning.is_some()))
}

fn validate_candidate_metadata(file: &CandidateFile) -> std::io::Result<()> {
    let metadata = fs::metadata(&file.canonical_path)?;
    if metadata.len() != file.size as u64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file size changed after discovery",
        ));
    }
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?
        .as_nanos()
        .min(i64::MAX as u128) as i64;
    if file.last_modified != 0 && modified != file.last_modified {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file modified time changed after discovery",
        ));
    }
    Ok(())
}

fn fingerprint_structure(values: &[(String, i64)]) -> String {
    let mut hasher = XxHash64::with_seed(0x5354_5255_4354);
    for (path, size) in values {
        hasher.write_usize(path.len());
        hasher.write(path.as_bytes());
        hasher.write_i64(*size);
    }
    format!("{:016x}", hasher.finish())
}

fn fingerprint_verified(values: &[(String, i64)]) -> String {
    let mut hasher = XxHash64::with_seed(0x5645_5249_4649);
    for (path, hash) in values {
        hasher.write_usize(path.len());
        hasher.write(path.as_bytes());
        hasher.write_i64(*hash);
    }
    format!("{:016x}", hasher.finish())
}

fn suppress_nested_groups(candidates: &[CandidateDirectory], groups: &mut [VerifiedGroup]) {
    let mut order = (0..groups.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| {
        groups[*index]
            .candidates
            .iter()
            .map(|candidate| normalized_components(&candidates[*candidate].path).len())
            .min()
            .unwrap_or(usize::MAX)
    });
    for (position, child_index) in order.iter().copied().enumerate() {
        for parent_index in order[..position].iter().copied() {
            if groups[parent_index].suppressed
                || groups[parent_index].candidates.len() != groups[child_index].candidates.len()
            {
                continue;
            }
            if group_covers(candidates, &groups[parent_index], &groups[child_index]) {
                groups[child_index].suppressed = true;
                break;
            }
        }
    }
}

fn group_covers(
    candidates: &[CandidateDirectory],
    parent: &VerifiedGroup,
    child: &VerifiedGroup,
) -> bool {
    let child_paths = child
        .candidates
        .iter()
        .map(|index| normalize_absolute(&candidates[*index].path))
        .collect::<HashSet<_>>();
    for child_index in &child.candidates {
        for parent_index in &parent.candidates {
            let child_path = Path::new(&candidates[*child_index].path);
            let parent_path = Path::new(&candidates[*parent_index].path);
            let Ok(suffix) = child_path.strip_prefix(parent_path) else {
                continue;
            };
            if suffix.as_os_str().is_empty() {
                continue;
            }
            if parent.candidates.iter().all(|index| {
                child_paths.contains(&normalize_absolute(
                    &Path::new(&candidates[*index].path)
                        .join(suffix)
                        .to_string_lossy(),
                ))
            }) {
                return true;
            }
        }
    }
    false
}

fn check_cancelled(cancel_token: &AtomicBool) -> Result<(), crate::Error> {
    if cancel_token.load(Ordering::Relaxed) {
        Err(crate::Error::Cancelled)
    } else {
        Ok(())
    }
}
