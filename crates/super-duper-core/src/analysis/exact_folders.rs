use crate::hasher::cache;
use crate::progress::ProgressReporter;
use crate::storage::models::{ExactFolderGroupInsert, ScannedFile};
use crate::storage::Database;
use rusqlite::params;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::hash::Hasher as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;
use twox_hash::XxHash64;

const PERSIST_BATCH_SIZE: usize = 1_024;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExactFolderAnalysis {
    pub visible_groups: usize,
    pub retained_groups: usize,
    pub warning_count: usize,
    pub directory_fingerprints: usize,
    pub scanned_file_passes: usize,
    pub largest_persistence_batch: usize,
}

#[derive(Debug, Clone)]
struct CandidateFile {
    id: i64,
    canonical_path: String,
    name: String,
    size: i64,
    last_modified: i64,
    content_hash: Option<i64>,
    file_identity: Option<String>,
}

#[derive(Debug)]
struct DirectoryState {
    path: String,
    normalized_path: String,
    name: String,
    parent: Option<usize>,
    children: Vec<usize>,
    direct_files: Vec<CandidateFile>,
    total_size: i64,
    file_count: i64,
    depth: i64,
    structural_class: u64,
    structural_fingerprint: String,
    verified_class: Option<u64>,
    verified_fingerprint: Option<String>,
    database_id: Option<i64>,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum StructuralAtom {
    File(String, i64),
    Directory(String, u64, String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum VerifiedAtom {
    File(String, i64, i64),
    Directory(String, u64, String),
}

/// Find exact duplicate folders from one ordered file stream and one bottom-up directory tree.
/// Each file snapshot is retained only by its direct parent; ancestors carry constant-size Merkle
/// state rather than cloned file records or descendant hash sets.
pub fn analyze_exact_folders_cancellable(
    db: &Database,
    run_id: i64,
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> Result<ExactFolderAnalysis, crate::Error> {
    check_cancelled(cancel_token)?;
    let total_files: usize = db.connection().query_row(
        "SELECT COUNT(*) FROM scanned_file WHERE run_id = ?1",
        params![run_id],
        |row| row.get(0),
    )?;
    let mut directories = Vec::<DirectoryState>::new();
    let mut by_path = HashMap::<String, usize>::new();
    let mut streamed_files = 0usize;
    let visited = db.visit_scanned_files_ordered(run_id, |file| {
        check_cancelled(cancel_token)?;
        add_file_to_tree(&mut directories, &mut by_path, file);
        streamed_files += 1;
        progress.on_dir_analysis_progress(streamed_files, total_files);
        Ok(())
    })?;

    let mut structural_classes = BTreeMap::<Vec<StructuralAtom>, u64>::new();
    let mut next_structural_class = 1u64;
    for index in (0..directories.len()).rev() {
        check_cancelled(cancel_token)?;
        directories[index]
            .direct_files
            .sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        let mut children = std::mem::take(&mut directories[index].children);
        children.sort_by(|left, right| {
            directories[*left]
                .name
                .cmp(&directories[*right].name)
                .then(left.cmp(right))
        });
        directories[index].children = children;
        let mut key = directories[index]
            .direct_files
            .iter()
            .map(|file| StructuralAtom::File(file.name.clone(), file.size))
            .collect::<Vec<_>>();
        let children = directories[index].children.clone();
        for child in children {
            key.push(StructuralAtom::Directory(
                directories[child].name.clone(),
                directories[child].structural_class,
                directories[child].structural_fingerprint.clone(),
            ));
            directories[index].total_size += directories[child].total_size;
            directories[index].file_count += directories[child].file_count;
        }
        directories[index].total_size += directories[index]
            .direct_files
            .iter()
            .map(|file| file.size)
            .sum::<i64>();
        directories[index].file_count += directories[index].direct_files.len() as i64;
        let class = *structural_classes.entry(key.clone()).or_insert_with(|| {
            let class = next_structural_class;
            next_structural_class += 1;
            class
        });
        directories[index].structural_class = class;
        directories[index].structural_fingerprint = fingerprint_structure(&key);
    }

    let mut structural_counts = HashMap::<u64, usize>::new();
    for directory in &directories {
        if directory.file_count > 0 {
            *structural_counts
                .entry(directory.structural_class)
                .or_default() += 1;
        }
    }

    let mut verified_classes = BTreeMap::<(u64, Vec<VerifiedAtom>), u64>::new();
    let mut next_verified_class = 1u64;
    let mut warning_count = 0usize;
    for index in (0..directories.len()).rev() {
        check_cancelled(cancel_token)?;
        if structural_counts
            .get(&directories[index].structural_class)
            .copied()
            .unwrap_or_default()
            < 2
        {
            continue;
        }
        let mut key = Vec::new();
        let mut valid = true;
        for file in &directories[index].direct_files {
            match verified_file_hash(db, run_id, file, cancel_token) {
                Ok((hash, cache_warning)) => {
                    warning_count += usize::from(cache_warning);
                    key.push(VerifiedAtom::File(file.name.clone(), file.size, hash));
                }
                Err(crate::Error::Cancelled) => return Err(crate::Error::Cancelled),
                Err(error) => {
                    warning_count += 1;
                    tracing::warn!(
                        "Unable to verify exact-folder candidate file {}: {error}",
                        file.canonical_path
                    );
                    valid = false;
                    break;
                }
            }
        }
        if valid {
            for child in directories[index].children.iter().copied() {
                let Some(child_class) = directories[child].verified_class else {
                    valid = false;
                    break;
                };
                key.push(VerifiedAtom::Directory(
                    directories[child].name.clone(),
                    child_class,
                    directories[child]
                        .verified_fingerprint
                        .clone()
                        .unwrap_or_default(),
                ));
            }
        }
        if valid {
            let map_key = (directories[index].structural_class, key.clone());
            let class = *verified_classes.entry(map_key).or_insert_with(|| {
                let class = next_verified_class;
                next_verified_class += 1;
                class
            });
            directories[index].verified_class = Some(class);
            directories[index].verified_fingerprint = Some(fingerprint_verified(&key));
        }
        progress.on_dir_analysis_progress(
            visited + directories.len() - index,
            visited + directories.len(),
        );
    }

    let mut by_verified = BTreeMap::<(u64, u64), Vec<usize>>::new();
    for (index, directory) in directories.iter().enumerate() {
        if let Some(verified_class) = directory.verified_class {
            by_verified
                .entry((directory.structural_class, verified_class))
                .or_default()
                .push(index);
        }
    }
    let mut groups = by_verified
        .into_values()
        .filter_map(|indexes| {
            let indexes = distinct_physical_candidates(&directories, indexes);
            if indexes.len() < 2 {
                return None;
            }
            let first = &directories[indexes[0]];
            Some(VerifiedGroup {
                structural_fingerprint: first.structural_fingerprint.clone(),
                verified_fingerprint: first.verified_fingerprint.clone().unwrap_or_default(),
                total_size: first.total_size,
                file_count: first.file_count,
                candidates: indexes,
                suppressed: false,
            })
        })
        .collect::<Vec<_>>();
    suppress_nested_groups(&directories, &mut groups);
    groups.sort_by(|left, right| {
        left.suppressed
            .cmp(&right.suppressed)
            .then(right.total_size.cmp(&left.total_size))
            .then_with(|| {
                directories[left.candidates[0]]
                    .normalized_path
                    .cmp(&directories[right.candidates[0]].normalized_path)
            })
    });

    persist_directory_nodes(db, run_id, &mut directories, cancel_token)?;
    persist_directory_fingerprints(db, &directories, cancel_token)?;
    let inserts = groups
        .iter()
        .map(|group| ExactFolderGroupInsert {
            structural_fingerprint: group.structural_fingerprint.clone(),
            verified_fingerprint: group.verified_fingerprint.clone(),
            total_size: group.total_size,
            file_count: group.file_count,
            directory_ids: group
                .candidates
                .iter()
                .map(|index| {
                    directories[*index]
                        .database_id
                        .expect("persisted directory")
                })
                .collect(),
            is_suppressed: group.suppressed,
        })
        .collect::<Vec<_>>();
    let visible_groups = db.replace_exact_folder_groups(run_id, &inserts, cancel_token)?;
    Ok(ExactFolderAnalysis {
        visible_groups,
        retained_groups: inserts.len(),
        warning_count,
        directory_fingerprints: directories.len(),
        scanned_file_passes: 1,
        largest_persistence_batch: directories.len().min(PERSIST_BATCH_SIZE),
    })
}

fn add_file_to_tree(
    directories: &mut Vec<DirectoryState>,
    by_path: &mut HashMap<String, usize>,
    file: ScannedFile,
) {
    if file.root_path.is_empty() {
        return;
    }
    let components = normalized_components(&file.relative_path);
    if components.is_empty() {
        return;
    }
    let root = ensure_directory(directories, by_path, None, PathBuf::from(&file.root_path));
    let mut parent = root;
    let mut path = PathBuf::from(&file.root_path);
    for component in &components[..components.len() - 1] {
        path.push(component);
        parent = ensure_directory(directories, by_path, Some(parent), path.clone());
    }
    directories[parent].direct_files.push(CandidateFile {
        id: file.id,
        canonical_path: file.canonical_path,
        name: components.last().unwrap().to_lowercase(),
        size: file.file_size,
        last_modified: file.last_modified,
        content_hash: file.content_hash,
        file_identity: file.file_identity,
    });
}

fn ensure_directory(
    directories: &mut Vec<DirectoryState>,
    by_path: &mut HashMap<String, usize>,
    parent: Option<usize>,
    path: PathBuf,
) -> usize {
    let display_path = path.to_string_lossy().into_owned();
    let normalized_path = normalize_absolute(&display_path);
    if let Some(index) = by_path.get(&normalized_path).copied() {
        return index;
    }
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| normalized_path.clone());
    let index = directories.len();
    directories.push(DirectoryState {
        path: display_path,
        normalized_path: normalized_path.clone(),
        name,
        parent,
        children: Vec::new(),
        direct_files: Vec::new(),
        total_size: 0,
        file_count: 0,
        depth: path.components().count() as i64,
        structural_class: 0,
        structural_fingerprint: String::new(),
        verified_class: None,
        verified_fingerprint: None,
        database_id: None,
    });
    by_path.insert(normalized_path, index);
    if let Some(parent) = parent {
        directories[parent].children.push(index);
    }
    index
}

fn verified_file_hash(
    db: &Database,
    run_id: i64,
    file: &CandidateFile,
    cancel_token: &AtomicBool,
) -> Result<(i64, bool), crate::Error> {
    validate_candidate_metadata(file)?;
    if let Some(hash) = file.content_hash {
        return Ok((hash, false));
    }
    let outcome =
        cache::get_content_hash_cancellable(Path::new(&file.canonical_path), cancel_token)?;
    db.update_scanned_file_content_hash(run_id, file.id, outcome.hash as i64)?;
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

fn persist_directory_nodes(
    db: &Database,
    run_id: i64,
    directories: &mut [DirectoryState],
    cancel_token: &AtomicBool,
) -> Result<(), crate::Error> {
    db.connection().execute(
        "DELETE FROM directory_node WHERE run_id = ?1",
        params![run_id],
    )?;
    for start in (0..directories.len()).step_by(PERSIST_BATCH_SIZE) {
        check_cancelled(cancel_token)?;
        let end = (start + PERSIST_BATCH_SIZE).min(directories.len());
        let tx = db.connection().unchecked_transaction()?;
        {
            let mut statement = tx.prepare_cached(
                "INSERT INTO directory_node
                    (run_id, path, name, parent_id, total_size, file_count, depth)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for index in start..end {
                check_cancelled(cancel_token)?;
                let parent_id = directories[index]
                    .parent
                    .and_then(|parent| directories[parent].database_id);
                statement.execute(params![
                    run_id,
                    directories[index].path,
                    directories[index].name,
                    parent_id,
                    directories[index].total_size,
                    directories[index].file_count,
                    directories[index].depth,
                ])?;
                directories[index].database_id = Some(tx.last_insert_rowid());
            }
        }
        tx.commit()?;
    }
    Ok(())
}

fn persist_directory_fingerprints(
    db: &Database,
    directories: &[DirectoryState],
    cancel_token: &AtomicBool,
) -> Result<(), crate::Error> {
    for chunk in directories.chunks(PERSIST_BATCH_SIZE) {
        check_cancelled(cancel_token)?;
        let tx = db.connection().unchecked_transaction()?;
        {
            let mut statement = tx.prepare_cached(
                "INSERT INTO directory_fingerprint
                    (directory_id, content_fingerprint, file_hash_set) VALUES (?1, ?2, '[]')",
            )?;
            for directory in chunk {
                check_cancelled(cancel_token)?;
                let fingerprint = directory
                    .verified_fingerprint
                    .as_ref()
                    .unwrap_or(&directory.structural_fingerprint);
                statement.execute(params![directory.database_id, fingerprint])?;
            }
        }
        tx.commit()?;
    }
    Ok(())
}

fn fingerprint_structure(values: &[StructuralAtom]) -> String {
    let mut hasher = XxHash64::with_seed(0x5354_5255_4354);
    for value in values {
        match value {
            StructuralAtom::File(name, size) => {
                hasher.write_u8(0);
                hasher.write_usize(name.len());
                hasher.write(name.as_bytes());
                hasher.write_i64(*size);
            }
            StructuralAtom::Directory(name, _class, fingerprint) => {
                hasher.write_u8(1);
                hasher.write_usize(name.len());
                hasher.write(name.as_bytes());
                hasher.write_usize(fingerprint.len());
                hasher.write(fingerprint.as_bytes());
            }
        }
    }
    format!("{:016x}", hasher.finish())
}

fn fingerprint_verified(values: &[VerifiedAtom]) -> String {
    let mut hasher = XxHash64::with_seed(0x5645_5249_4649);
    for value in values {
        match value {
            VerifiedAtom::File(name, size, hash) => {
                hasher.write_u8(0);
                hasher.write_usize(name.len());
                hasher.write(name.as_bytes());
                hasher.write_i64(*size);
                hasher.write_i64(*hash);
            }
            VerifiedAtom::Directory(name, _class, fingerprint) => {
                hasher.write_u8(1);
                hasher.write_usize(name.len());
                hasher.write(name.as_bytes());
                hasher.write_usize(fingerprint.len());
                hasher.write(fingerprint.as_bytes());
            }
        }
    }
    format!("{:016x}", hasher.finish())
}

fn distinct_physical_candidates(
    directories: &[DirectoryState],
    mut candidates: Vec<usize>,
) -> Vec<usize> {
    candidates.sort_by(|left, right| {
        directories[*left]
            .normalized_path
            .cmp(&directories[*right].normalized_path)
    });
    let mut seen = HashSet::<String>::new();
    candidates
        .into_iter()
        .filter(|candidate| {
            let mut identities = Vec::new();
            collect_identities(directories, *candidate, &mut identities);
            if identities.iter().any(|identity| seen.contains(identity)) {
                return false;
            }
            seen.extend(identities);
            true
        })
        .collect()
}

fn collect_identities(directories: &[DirectoryState], index: usize, output: &mut Vec<String>) {
    output.extend(
        directories[index]
            .direct_files
            .iter()
            .filter_map(|file| file.file_identity.clone()),
    );
    for child in directories[index].children.iter().copied() {
        collect_identities(directories, child, output);
    }
}

fn suppress_nested_groups(directories: &[DirectoryState], groups: &mut [VerifiedGroup]) {
    let mut order = (0..groups.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| {
        groups[*index]
            .candidates
            .iter()
            .map(|candidate| directories[*candidate].depth)
            .min()
            .unwrap_or(i64::MAX)
    });
    for (position, child_index) in order.iter().copied().enumerate() {
        for parent_index in order[..position].iter().copied() {
            if groups[parent_index].suppressed
                || groups[parent_index].candidates.len() != groups[child_index].candidates.len()
            {
                continue;
            }
            if group_covers(directories, &groups[parent_index], &groups[child_index]) {
                groups[child_index].suppressed = true;
                break;
            }
        }
    }
}

fn group_covers(
    directories: &[DirectoryState],
    parent: &VerifiedGroup,
    child: &VerifiedGroup,
) -> bool {
    let child_paths = child
        .candidates
        .iter()
        .map(|index| directories[*index].normalized_path.clone())
        .collect::<HashSet<_>>();
    for child_index in &child.candidates {
        for parent_index in &parent.candidates {
            let child_path = Path::new(&directories[*child_index].path);
            let parent_path = Path::new(&directories[*parent_index].path);
            let Ok(suffix) = child_path.strip_prefix(parent_path) else {
                continue;
            };
            if suffix.as_os_str().is_empty() {
                continue;
            }
            if parent.candidates.iter().all(|index| {
                child_paths.contains(&normalize_absolute(
                    &Path::new(&directories[*index].path)
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

fn check_cancelled(cancel_token: &AtomicBool) -> Result<(), crate::Error> {
    if cancel_token.load(Ordering::Relaxed) {
        Err(crate::Error::Cancelled)
    } else {
        Ok(())
    }
}
