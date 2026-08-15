use crate::progress::{ProgressReporter, SilentReporter};
use crate::storage::Database;
use ahash::AHashMap;
use rusqlite::params;
use std::hash::Hasher as _;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::info;
use twox_hash::XxHash64;

/// Build directory hierarchy from scanned files, compute fingerprints bottom-up.
///
/// Algorithm:
/// 1. Build directory_node tree from scanned_file.parent_dir
/// 2. Process directories bottom-up by depth:
///    - Collect content hashes of direct child files
///    - Union with child directories' hash sets (already computed)
///    - content_fingerprint = XxHash64 of sorted hash list
///    - Store file_hash_set as JSON for Jaccard computation
pub fn build_directory_fingerprints(db: &Database, run_id: i64) -> Result<usize, crate::Error> {
    build_directory_fingerprints_cancellable(db, run_id, &AtomicBool::new(false), &SilentReporter)
}

pub fn build_directory_fingerprints_cancellable(
    db: &Database,
    run_id: i64,
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> Result<usize, crate::Error> {
    check_cancelled(cancel_token)?;
    info!("Building directory hierarchy...");
    db.connection().execute(
        "DELETE FROM directory_node WHERE run_id = ?1",
        params![run_id],
    )?;

    // Step 1: Collect all unique parent directories from scanned files
    let mut stmt = db.connection().prepare(
        "SELECT DISTINCT parent_dir FROM scanned_file WHERE run_id = ?1 ORDER BY parent_dir",
    )?;

    let parent_dirs: Vec<String> = stmt
        .query_map(params![run_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    // Step 2: Build directory_node entries
    let mut dir_id_map: AHashMap<String, i64> = AHashMap::new();

    for dir_path in &parent_dirs {
        check_cancelled(cancel_token)?;
        insert_directory_hierarchy(db, run_id, dir_path, &mut dir_id_map)?;
        progress.on_dir_analysis_progress(dir_id_map.len(), parent_dirs.len());
    }

    info!("Built {} directory nodes", dir_id_map.len());

    // Step 3: Compute file counts and sizes for each directory
    db.connection().execute(
        "UPDATE directory_node SET
            file_count = (SELECT COUNT(*) FROM scanned_file WHERE run_id = directory_node.run_id AND parent_dir = directory_node.path),
            total_size = (SELECT COALESCE(SUM(file_size), 0) FROM scanned_file WHERE run_id = directory_node.run_id AND parent_dir = directory_node.path)
         WHERE run_id = ?1",
        params![run_id],
    )?;

    // Step 4: Propagate sizes up the tree (from deepest to shallowest)
    let max_depth: i64 = db.connection().query_row(
        "SELECT COALESCE(MAX(depth), 0) FROM directory_node WHERE run_id = ?1",
        params![run_id],
        |row| row.get(0),
    )?;

    for depth in (0..max_depth).rev() {
        check_cancelled(cancel_token)?;
        db.connection().execute(
            "UPDATE directory_node SET total_size = total_size + COALESCE(
                (SELECT SUM(dn2.total_size) FROM directory_node dn2 WHERE dn2.parent_id = directory_node.id),
                0
            ), file_count = file_count + COALESCE(
                (SELECT SUM(dn2.file_count) FROM directory_node dn2 WHERE dn2.parent_id = directory_node.id),
                0
            ) WHERE run_id = ?1 AND depth = ?2",
            params![run_id, depth],
        )?;
    }

    // Step 5: Compute fingerprints bottom-up
    info!("Computing directory fingerprints...");
    let mut fingerprint_count = 0;

    for depth in (0..=max_depth).rev() {
        check_cancelled(cancel_token)?;
        let mut dir_stmt = db
            .connection()
            .prepare("SELECT id, path FROM directory_node WHERE run_id = ?1 AND depth = ?2")?;

        let dirs: Vec<(i64, String)> = dir_stmt
            .query_map(params![run_id, depth], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        for (dir_id, dir_path) in &dirs {
            check_cancelled(cancel_token)?;
            // Collect content hashes of direct child files
            let mut hash_stmt = db.connection().prepare(
                "SELECT content_hash FROM scanned_file
                 WHERE run_id = ?1 AND parent_dir = ?2 AND content_hash IS NOT NULL",
            )?;
            let mut hashes: Vec<i64> = hash_stmt
                .query_map(params![run_id, dir_path], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;

            // Union with child directories' hash sets
            let mut child_stmt = db.connection().prepare(
                "SELECT df.file_hash_set FROM directory_fingerprint df \
                 JOIN directory_node dn ON df.directory_id = dn.id \
                 WHERE dn.run_id = ?1 AND dn.parent_id = ?2",
            )?;
            let child_hash_sets: Vec<String> = child_stmt
                .query_map(params![run_id, dir_id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;

            for hash_set_json in &child_hash_sets {
                if let Ok(child_hashes) = serde_json::from_str::<Vec<i64>>(hash_set_json) {
                    hashes.extend(child_hashes);
                }
            }

            if hashes.is_empty() {
                continue;
            }

            // Sort for deterministic fingerprint
            hashes.sort();
            hashes.dedup();

            // Compute content fingerprint = XxHash64 of sorted hash list
            let mut hasher = XxHash64::with_seed(0);
            for h in &hashes {
                hasher.write_i64(*h);
            }
            let fingerprint = format!("{:016x}", hasher.finish());

            // Store hash set as JSON
            let hash_set_json = serde_json::to_string(&hashes).unwrap_or_default();

            db.insert_directory_fingerprint(*dir_id, &fingerprint, &hash_set_json)?;
            fingerprint_count += 1;
            progress.on_dir_analysis_progress(fingerprint_count, dir_id_map.len());
        }
    }

    info!("Computed {} directory fingerprints", fingerprint_count);
    Ok(fingerprint_count)
}

fn check_cancelled(cancel_token: &AtomicBool) -> Result<(), crate::Error> {
    if cancel_token.load(Ordering::Relaxed) {
        Err(crate::Error::Cancelled)
    } else {
        Ok(())
    }
}

fn insert_directory_hierarchy(
    db: &Database,
    run_id: i64,
    dir_path: &str,
    dir_id_map: &mut AHashMap<String, i64>,
) -> Result<i64, crate::Error> {
    if let Some(&id) = dir_id_map.get(dir_path) {
        return Ok(id);
    }

    let path = std::path::Path::new(dir_path);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir_path.to_string());

    let depth = path.components().count() as i64;

    let parent_id = if let Some(parent) = path.parent() {
        let parent_str = parent.to_string_lossy().into_owned();
        if parent_str != dir_path && !parent_str.is_empty() {
            Some(insert_directory_hierarchy(
                db,
                run_id,
                &parent_str,
                dir_id_map,
            )?)
        } else {
            None
        }
    } else {
        None
    };

    let id = db.insert_directory_node(run_id, dir_path, &name, parent_id, 0, 0, depth)?;
    dir_id_map.insert(dir_path.to_string(), id);
    Ok(id)
}
