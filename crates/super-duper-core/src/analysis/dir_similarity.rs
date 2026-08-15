use crate::progress::{ProgressReporter, SilentReporter};
use crate::storage::Database;
use ahash::{AHashMap, AHashSet};
use rayon::prelude::*;
use rusqlite::params;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::info;

/// Compute directory similarity using Jaccard index.
///
/// 1. Build inverted index: content_hash → Vec<directory_id>
/// 2. Identify candidate pairs (directories sharing at least one hash)
/// 3. Skip hashes appearing in >50 directories (noise)
/// 4. Compute Jaccard = |intersection| / |union| for each candidate pair
/// 5. Store pairs above threshold
pub fn compute_directory_similarity(
    db: &Database,
    run_id: i64,
    threshold: f64,
) -> Result<usize, crate::Error> {
    compute_directory_similarity_cancellable(
        db,
        run_id,
        threshold,
        &AtomicBool::new(false),
        &SilentReporter,
    )
}

pub fn compute_directory_similarity_cancellable(
    db: &Database,
    run_id: i64,
    threshold: f64,
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
) -> Result<usize, crate::Error> {
    check_cancelled(cancel_token)?;
    info!(
        "Computing directory similarity (threshold={:.2})...",
        threshold
    );

    // Load all directory fingerprints
    let mut stmt = db.connection().prepare(
        "SELECT df.directory_id, df.file_hash_set FROM directory_fingerprint df
         JOIN directory_node dn ON dn.id = df.directory_id WHERE dn.run_id = ?1",
    )?;

    let fingerprints: Vec<(i64, Vec<i64>)> = stmt
        .query_map(params![run_id], |row| {
            let dir_id: i64 = row.get(0)?;
            let hash_set_json: String = row.get(1)?;
            Ok((dir_id, hash_set_json))
        })?
        .filter_map(|r| r.ok())
        .filter_map(|(dir_id, json)| {
            serde_json::from_str::<Vec<i64>>(&json)
                .ok()
                .map(|hashes| (dir_id, hashes))
        })
        .collect();

    if fingerprints.is_empty() {
        info!("No directory fingerprints found");
        return Ok(0);
    }

    // Build content_hash → file_size map for accurate shared-bytes computation.
    // Duplicate files share the same content_hash and file_size, so one row per hash suffices.
    let hash_to_size: AHashMap<i64, i64> = {
        let mut stmt = db.connection().prepare(
            "SELECT content_hash, file_size FROM scanned_file \
             WHERE run_id = ?1 AND content_hash IS NOT NULL \
             GROUP BY content_hash",
        )?;
        let map: AHashMap<i64, i64> = stmt
            .query_map(params![run_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        map
    };

    // Build inverted index: hash → Vec<dir_id>
    let mut inverted_index: AHashMap<i64, Vec<i64>> = AHashMap::new();
    let mut dir_hash_sets: AHashMap<i64, AHashSet<i64>> = AHashMap::new();

    for (dir_id, hashes) in &fingerprints {
        check_cancelled(cancel_token)?;
        let hash_set: AHashSet<i64> = hashes.iter().copied().collect();
        for &hash in &hash_set {
            inverted_index.entry(hash).or_default().push(*dir_id);
        }
        dir_hash_sets.insert(*dir_id, hash_set);
    }

    // Find candidate pairs (share at least one hash)
    // Skip hashes appearing in >50 directories (noise: common files like README, .gitkeep)
    let max_dir_frequency = 50;
    let mut candidate_pairs: AHashSet<(i64, i64)> = AHashSet::new();

    for (_hash, dir_ids) in &inverted_index {
        check_cancelled(cancel_token)?;
        if dir_ids.len() > max_dir_frequency {
            continue;
        }
        for i in 0..dir_ids.len() {
            for j in (i + 1)..dir_ids.len() {
                let (a, b) = if dir_ids[i] < dir_ids[j] {
                    (dir_ids[i], dir_ids[j])
                } else {
                    (dir_ids[j], dir_ids[i])
                };
                candidate_pairs.insert((a, b));
            }
        }
    }

    info!("Found {} candidate directory pairs", candidate_pairs.len());

    // Compute Jaccard similarity for each candidate pair
    let pairs_vec: Vec<(i64, i64)> = candidate_pairs.into_iter().collect();
    let results: Vec<(i64, i64, f64, i64, &str)> = pairs_vec
        .par_iter()
        .filter_map(|&(dir_a, dir_b)| {
            if cancel_token.load(Ordering::Relaxed) {
                return None;
            }
            let set_a = dir_hash_sets.get(&dir_a)?;
            let set_b = dir_hash_sets.get(&dir_b)?;

            let intersection_size = set_a.intersection(set_b).count();
            let union_size = set_a.union(set_b).count();

            if union_size == 0 {
                return None;
            }

            let jaccard = intersection_size as f64 / union_size as f64;
            if jaccard < threshold {
                return None;
            }

            // Determine match type
            let match_type = if jaccard >= 1.0 {
                "exact"
            } else if set_a.is_subset(set_b) || set_b.is_subset(set_a) {
                "subset"
            } else {
                "threshold"
            };

            // Sum actual file sizes for shared hashes
            let shared_bytes: i64 = set_a
                .intersection(set_b)
                .map(|h| hash_to_size.get(h).copied().unwrap_or(0))
                .sum();

            Some((dir_a, dir_b, jaccard, shared_bytes, match_type))
        })
        .collect();
    check_cancelled(cancel_token)?;

    // Write results to database
    let mut similarity_count = 0;
    for (dir_a, dir_b, score, shared_bytes, match_type) in &results {
        check_cancelled(cancel_token)?;
        db.insert_directory_similarity(run_id, *dir_a, *dir_b, *score, *shared_bytes, match_type)?;
        similarity_count += 1;
        progress.on_dir_analysis_progress(similarity_count, results.len());
    }

    // Also find exact matches via content_fingerprint
    let exact_count = find_exact_matches(db, run_id, cancel_token, progress, similarity_count)?;

    info!(
        "Computed {} similarity pairs ({} from Jaccard, {} exact fingerprint matches)",
        similarity_count + exact_count,
        similarity_count,
        exact_count,
    );

    Ok(similarity_count + exact_count)
}

/// Find exact directory duplicates via matching content_fingerprint.
fn find_exact_matches(
    db: &Database,
    run_id: i64,
    cancel_token: &AtomicBool,
    progress: &dyn ProgressReporter,
    progress_offset: usize,
) -> Result<usize, crate::Error> {
    check_cancelled(cancel_token)?;
    // Find fingerprints that appear more than once
    let mut stmt = db.connection().prepare(
        "SELECT df1.directory_id, df2.directory_id \
         FROM directory_fingerprint df1 \
         JOIN directory_fingerprint df2 ON df1.content_fingerprint = df2.content_fingerprint \
         JOIN directory_node dn1 ON dn1.id = df1.directory_id
         JOIN directory_node dn2 ON dn2.id = df2.directory_id
         WHERE dn1.run_id = ?1 AND dn2.run_id = ?1 AND df1.directory_id < df2.directory_id",
    )?;

    let pairs: Vec<(i64, i64)> = stmt
        .query_map(params![run_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut count = 0;
    for (dir_a, dir_b) in &pairs {
        check_cancelled(cancel_token)?;
        // Only insert if not already present
        let existing: i64 = db.connection().query_row(
            "SELECT COUNT(*) FROM directory_similarity
             WHERE run_id = ?1 AND dir_a_id = ?2 AND dir_b_id = ?3",
            params![run_id, dir_a, dir_b],
            |row| row.get(0),
        )?;

        if existing == 0 {
            // Get shared bytes from directory total_size
            let shared_bytes: i64 = db.connection().query_row(
                "SELECT COALESCE(total_size, 0) FROM directory_node WHERE id = ?1",
                params![dir_a],
                |row| row.get(0),
            )?;

            db.insert_directory_similarity(run_id, *dir_a, *dir_b, 1.0, shared_bytes, "exact")?;
            count += 1;
            progress
                .on_dir_analysis_progress(progress_offset + count, progress_offset + pairs.len());
        }
    }

    Ok(count)
}

fn check_cancelled(cancel_token: &AtomicBool) -> Result<(), crate::Error> {
    if cancel_token.load(Ordering::Relaxed) {
        Err(crate::Error::Cancelled)
    } else {
        Ok(())
    }
}
