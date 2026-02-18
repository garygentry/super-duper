use crate::storage::models::ScannedFile;
use crate::storage::Database;
use rusqlite::params;
use std::fs;
use std::path::Path;
use tracing::{debug, error, info, warn};

/// Mark all files in a directory for deletion.
pub fn mark_directory_for_deletion(
    db: &Database,
    directory_path: &str,
    strategy: Option<&str>,
) -> Result<usize, crate::Error> {
    let mut stmt = db.connection().prepare(
        "SELECT id FROM scanned_file WHERE parent_dir = ?1 OR parent_dir LIKE ?2",
    )?;
    let like_pattern = format!("{}%", directory_path);
    let file_ids: Vec<i64> = stmt
        .query_map(params![directory_path, like_pattern], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    for file_id in &file_ids {
        db.mark_file_for_deletion(*file_id, strategy)?;
    }

    info!(
        "Marked {} files in directory '{}' for deletion",
        file_ids.len(),
        directory_path
    );
    Ok(file_ids.len())
}

/// Auto-mark duplicates for deletion using a strategy.
/// For each duplicate group in the given session, keep one file (the first alphabetically)
/// and mark the rest.
pub fn auto_mark_duplicates(
    db: &Database,
    session_id: i64,
    strategy: Option<&str>,
) -> Result<usize, crate::Error> {
    let groups = db.get_duplicate_groups(session_id, 0, i64::MAX)?;
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
/// When `use_trash` is true, files are moved to the system Recycle Bin / Trash
/// instead of being permanently deleted.
pub fn execute_deletion_plan(db: &Database, use_trash: bool) -> Result<(usize, usize), crate::Error> {
    let plan = db.get_deletion_plan()?;
    let mut success_count = 0;
    let mut error_count = 0;

    for entry in &plan {
        // Get file info
        let file: Option<ScannedFile> = db
            .connection()
            .query_row(
                "SELECT id, canonical_path, file_name, parent_dir, drive_letter, \
                 file_size, last_modified, partial_hash, content_hash, \
                 last_seen_session_id, marked_deleted \
                 FROM scanned_file WHERE id = ?1",
                params![entry.file_id],
                |row| {
                    Ok(ScannedFile {
                        id: row.get(0)?,
                        canonical_path: row.get(1)?,
                        file_name: row.get(2)?,
                        parent_dir: row.get(3)?,
                        drive_letter: row.get(4)?,
                        file_size: row.get(5)?,
                        last_modified: row.get(6)?,
                        partial_hash: row.get(7)?,
                        content_hash: row.get(8)?,
                        last_seen_session_id: row.get(9)?,
                        marked_deleted: row.get(10)?,
                    })
                },
            )
            .ok();

        let file = match file {
            Some(f) => f,
            None => {
                warn!("File ID {} not found in database, skipping", entry.file_id);
                error_count += 1;
                continue;
            }
        };

        let path = Path::new(&file.canonical_path);

        // Verify file still exists
        if !path.exists() {
            warn!("File '{}' no longer exists, marking as executed", file.canonical_path);
            let now = chrono::Utc::now().to_rfc3339();
            db.connection().execute(
                "UPDATE deletion_plan SET executed_at = ?1, execution_result = 'file_missing' \
                 WHERE id = ?2",
                params![now, entry.id],
            )?;
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
                let now = chrono::Utc::now().to_rfc3339();
                let result_label = if use_trash { "trashed" } else { "success" };
                db.connection().execute(
                    "UPDATE deletion_plan SET executed_at = ?1, execution_result = ?2 \
                     WHERE id = ?3",
                    params![now, result_label, entry.id],
                )?;
                db.connection().execute(
                    "UPDATE scanned_file SET marked_deleted = 1 WHERE id = ?1",
                    params![file.id],
                )?;
                success_count += 1;
                debug!("{}: {}", result_label, file.canonical_path);
            }
            Err(e) => {
                error!("Failed to remove '{}': {}", file.canonical_path, e);
                let now = chrono::Utc::now().to_rfc3339();
                db.connection().execute(
                    "UPDATE deletion_plan SET executed_at = ?1, execution_result = ?2 \
                     WHERE id = ?3",
                    params![now, e, entry.id],
                )?;
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
