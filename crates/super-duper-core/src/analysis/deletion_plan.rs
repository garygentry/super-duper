use crate::hasher::xxhash::hash_file_streaming;
use crate::platform::{self, PathSafety};
use crate::storage::models::ScannedFile;
use crate::storage::queries::like_prefix_pattern;
use crate::storage::Database;
use rusqlite::params;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::UNIX_EPOCH;
use tracing::{debug, error, info, warn};

/// Mark all files of `run_id` located in `directory_path` or any of its subdirectories for
/// deletion. Sibling directories that merely share a name prefix (`job_1` vs `job_10`) are not
/// matched, and `%`/`_` in the directory name are treated literally.
pub fn mark_directory_for_deletion(
    db: &Database,
    run_id: i64,
    directory_path: &str,
    strategy: Option<&str>,
) -> Result<usize, crate::Error> {
    if directory_path.trim().is_empty() {
        return Err(crate::Error::Other("directory_path is empty".to_owned()));
    }
    let directory = directory_path.trim_end_matches(['\\', '/']);
    let mut stmt = db.connection().prepare(
        "SELECT id, parent_dir FROM scanned_file
         WHERE run_id = ?1 AND marked_deleted = 0
           AND (parent_dir = ?2 COLLATE NOCASE
                OR parent_dir = ?3 COLLATE NOCASE
                OR parent_dir LIKE ?4 ESCAPE '\\'
                OR parent_dir LIKE ?5 ESCAPE '\\')",
    )?;
    let candidates: Vec<(i64, String)> = stmt
        .query_map(
            params![
                run_id,
                directory,
                directory_path,
                like_prefix_pattern(&format!("{directory}\\")),
                like_prefix_pattern(&format!("{directory}/")),
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;

    // SQL LIKE folds ASCII case on every platform; re-check the boundary with path semantics.
    let file_ids: Vec<i64> = candidates
        .into_iter()
        .filter(|(_, parent_dir)| directory_contains(directory, directory_path, parent_dir))
        .map(|(id, _)| id)
        .collect();

    for file_id in &file_ids {
        db.mark_file_for_deletion(*file_id, strategy)?;
    }

    info!(
        "Marked {} files in directory '{}' (run {}) for deletion",
        file_ids.len(),
        directory_path,
        run_id
    );
    Ok(file_ids.len())
}

fn directory_contains(directory: &str, original: &str, parent_dir: &str) -> bool {
    let parent = parent_dir.as_bytes();
    if path_bytes_eq(parent, directory.as_bytes()) || path_bytes_eq(parent, original.as_bytes()) {
        return true;
    }
    let prefix = directory.as_bytes();
    parent.len() > prefix.len()
        && path_bytes_eq(&parent[..prefix.len()], prefix)
        && matches!(parent[prefix.len()], b'\\' | b'/')
}

fn path_bytes_eq(left: &[u8], right: &[u8]) -> bool {
    if cfg!(windows) {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

/// Auto-mark duplicates for deletion using a strategy.
/// For each duplicate group in the given run, keep one file (the first alphabetically)
/// and mark the rest.
pub fn auto_mark_duplicates(
    db: &Database,
    run_id: i64,
    strategy: Option<&str>,
) -> Result<usize, crate::Error> {
    let groups = db.get_duplicate_groups(run_id, 0, i64::MAX)?;
    let mut marked_count = 0;

    for group in &groups {
        let files = db.get_files_in_group(group.id)?;
        if files.len() <= 1 {
            continue;
        }

        // Keep the first file (sorted by path), mark the rest
        let mut sorted_files = files.clone();
        sorted_files.sort_by(|a, b| a.canonical_path.cmp(&b.canonical_path));

        for file in sorted_files.iter().skip(1) {
            db.mark_file_for_deletion(file.id, strategy)?;
            marked_count += 1;
        }
    }

    info!("Auto-marked {} files for deletion", marked_count);
    Ok(marked_count)
}

/// Execute the deletion plan. Returns (success_count, error_count).
///
/// Immediately before each removal the target is re-validated against its scan snapshot
/// (stable identity, size, modification time, and full content hash), and every duplicate group
/// containing it must still have at least one other member that is not planned for deletion and
/// also matches its snapshot on disk. Entries that fail are not removed; they are recorded with
/// `execution_result = 'skipped:<reason>'` and counted as errors. Files that belong to no
/// duplicate group are never removed.
///
/// When `use_trash` is true, files are moved to the system Recycle Bin / Trash
/// instead of being permanently deleted.
pub fn execute_deletion_plan(
    db: &Database,
    use_trash: bool,
) -> Result<(usize, usize), crate::Error> {
    let plan = db.get_deletion_plan()?;
    let cancel = AtomicBool::new(false);
    let mut hash_verified_survivors = HashSet::new();
    let mut success_count = 0;
    let mut error_count = 0;

    for entry in &plan {
        let file = match load_file(db, entry.file_id)? {
            Some(f) => f,
            None => {
                warn!("File ID {} not found in database, skipping", entry.file_id);
                error_count += 1;
                continue;
            }
        };

        let path = Path::new(&file.canonical_path);

        let identity = match verify_snapshot(&file, &cancel) {
            Ok(identity) => identity,
            Err("path_missing") => {
                warn!(
                    "File '{}' no longer exists, marking as executed",
                    file.canonical_path
                );
                record_result(db, entry.id, "file_missing")?;
                continue;
            }
            Err(reason) => {
                warn!(
                    "Skipping '{}': target failed revalidation ({})",
                    file.canonical_path, reason
                );
                record_result(db, entry.id, &format!("skipped:{reason}"))?;
                error_count += 1;
                continue;
            }
        };

        if let Err(reason) =
            verify_survivors(db, &file, &identity, &cancel, &mut hash_verified_survivors)
        {
            warn!(
                "Skipping '{}': no verified surviving duplicate ({})",
                file.canonical_path, reason
            );
            record_result(db, entry.id, &format!("skipped:{reason}"))?;
            error_count += 1;
            continue;
        }

        // Delete or trash the file
        let delete_result: Result<(), String> = if use_trash {
            #[cfg(windows)]
            {
                trash::delete(path).map_err(|e| format!("trash error: {}", e))
            }
            #[cfg(not(windows))]
            {
                // Trash not supported on this platform; fall back to permanent deletion
                fs::remove_file(path).map_err(|e| format!("error: {}", e))
            }
        } else {
            fs::remove_file(path).map_err(|e| format!("error: {}", e))
        };

        match delete_result {
            Ok(()) => {
                let result_label = if use_trash { "trashed" } else { "success" };
                record_result(db, entry.id, result_label)?;
                db.connection().execute(
                    "UPDATE scanned_file SET marked_deleted = 1 WHERE id = ?1",
                    params![file.id],
                )?;
                success_count += 1;
                debug!("{}: {}", result_label, file.canonical_path);
            }
            Err(e) => {
                error!("Failed to remove '{}': {}", file.canonical_path, e);
                record_result(db, entry.id, &e)?;
                error_count += 1;
            }
        }
    }

    info!(
        "Deletion plan executed: {} succeeded, {} failed",
        success_count, error_count
    );
    Ok((success_count, error_count))
}

fn load_file(db: &Database, file_id: i64) -> Result<Option<ScannedFile>, crate::Error> {
    let result = db.connection().query_row(
        "SELECT id, run_id, root_path, canonical_path, relative_path, file_name,
         parent_dir, drive_letter, file_size, last_modified, partial_hash, content_hash,
         file_identity, warning_message, marked_deleted
         FROM scanned_file WHERE id = ?1",
        params![file_id],
        |row| {
            Ok(ScannedFile {
                id: row.get(0)?,
                run_id: row.get(1)?,
                root_path: row.get(2)?,
                canonical_path: row.get(3)?,
                relative_path: row.get(4)?,
                file_name: row.get(5)?,
                parent_dir: row.get(6)?,
                drive_letter: row.get(7)?,
                file_size: row.get(8)?,
                last_modified: row.get(9)?,
                partial_hash: row.get(10)?,
                content_hash: row.get(11)?,
                file_identity: row.get(12)?,
                warning_message: row.get(13)?,
                marked_deleted: row.get(14)?,
            })
        },
    );
    match result {
        Ok(file) => Ok(Some(file)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn record_result(db: &Database, entry_id: i64, result: &str) -> Result<(), crate::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    db.connection().execute(
        "UPDATE deletion_plan SET executed_at = ?1, execution_result = ?2 WHERE id = ?3",
        params![now, result, entry_id],
    )?;
    Ok(())
}

/// Every duplicate group holding `target` must retain another member that is not planned for
/// deletion, is a different physical file, and still matches its own scan snapshot.
fn verify_survivors(
    db: &Database,
    target: &ScannedFile,
    target_identity: &str,
    cancel: &AtomicBool,
    hash_verified: &mut HashSet<i64>,
) -> Result<(), &'static str> {
    let group_ids = duplicate_group_ids(db, target.id).map_err(|_| "survivor_query_failed")?;
    if group_ids.is_empty() {
        return Err("no_duplicate_group");
    }
    for group_id in group_ids {
        let members = db
            .get_files_in_group(group_id)
            .map_err(|_| "survivor_query_failed")?;
        let mut survivor_found = false;
        for member in members {
            if member.id == target.id
                || member.marked_deleted
                || member.file_identity.as_deref() == Some(target_identity)
            {
                continue;
            }
            match db.is_file_marked_for_deletion(member.id) {
                Ok(false) => {}
                Ok(true) => continue,
                Err(_) => return Err("survivor_query_failed"),
            }
            // Metadata is re-checked every time; the full content hash is read once per
            // survivor per execution.
            let verified = if hash_verified.contains(&member.id) {
                verify_metadata(&member).is_ok()
            } else {
                verify_snapshot(&member, cancel).is_ok()
            };
            if verified {
                hash_verified.insert(member.id);
                survivor_found = true;
                break;
            }
            hash_verified.remove(&member.id);
        }
        if !survivor_found {
            return Err("survivor_missing");
        }
    }
    Ok(())
}

fn duplicate_group_ids(db: &Database, file_id: i64) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = db.connection().prepare(
        "SELECT dgm.group_id
         FROM duplicate_group_member dgm
         JOIN duplicate_group dg ON dg.id = dgm.group_id
         JOIN scanned_file sf ON sf.id = dgm.file_id AND sf.run_id = dg.run_id
         WHERE dgm.file_id = ?1
         ORDER BY dgm.group_id",
    )?;
    let ids = stmt.query_map(params![file_id], |row| row.get(0))?;
    ids.collect()
}

/// Confirms the file on disk is still the scanned file: same identity, size, modification time,
/// and content hash, with metadata re-read after hashing to catch concurrent writes. Returns the
/// verified identity; `Err("path_missing")` means the path no longer exists.
fn verify_snapshot(file: &ScannedFile, cancel: &AtomicBool) -> Result<String, &'static str> {
    let identity = verify_metadata(file)?;
    let expected_hash = file.content_hash.ok_or("snapshot_hash_missing")?;
    let hash = match hash_file_streaming(Path::new(&file.canonical_path), cancel) {
        Ok(hash) => hash as i64,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err("path_missing"),
        Err(_) => return Err("hash_unavailable"),
    };
    verify_metadata(file).map_err(|reason| match reason {
        "path_missing" => "path_missing",
        _ => "changed_during_validation",
    })?;
    if hash != expected_hash {
        return Err("content_hash_changed");
    }
    Ok(identity)
}

fn verify_metadata(file: &ScannedFile) -> Result<String, &'static str> {
    let path = Path::new(&file.canonical_path);
    match platform::classify_path_without_open(path) {
        Ok(PathSafety::File) => {}
        Ok(PathSafety::Missing) => return Err("path_missing"),
        Ok(PathSafety::CloudPlaceholder) => return Err("cloud_placeholder"),
        Ok(PathSafety::ReparsePoint) => return Err("reparse_point"),
        Ok(PathSafety::Directory) => return Err("wrong_type_directory"),
        Ok(PathSafety::Other) => return Err("wrong_type_other"),
        Err(_) => return Err("metadata_unavailable"),
    }
    let expected_identity = match file.file_identity.as_deref() {
        Some(identity) if !identity.is_empty() => identity,
        _ => return Err("snapshot_identity_missing"),
    };
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err("path_missing"),
        Err(_) => return Err("metadata_unavailable"),
    };
    let identity = match platform::file_identity(path) {
        Ok(Some(identity)) => identity,
        Ok(None) | Err(_) => return Err("identity_unavailable"),
    };
    if identity != expected_identity {
        return Err("identity_changed");
    }
    if metadata.len().min(i64::MAX as u64) as i64 != file.file_size {
        return Err("size_changed");
    }
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .ok_or("timestamp_unavailable")?;
    if modified != file.last_modified {
        return Err("timestamp_changed");
    }
    Ok(identity)
}
