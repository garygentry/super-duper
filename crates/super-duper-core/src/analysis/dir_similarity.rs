use crate::storage::Database;
use ahash::{AHashMap, AHashSet};
use rayon::prelude::*;
use rusqlite::params;
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
    threshold: f64,
) -> Result<usize, crate::Error> {
    info!("Computing directory similarity (threshold={:.2})...", threshold);

    // Load all directory fingerprints
    let mut stmt = db.connection().prepare(
        "SELECT directory_id, file_hash_set FROM directory_fingerprint",
    )?;

    let fingerprints: Vec<(i64, Vec<i64>)> = stmt
        .query_map([], |row| {
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

    // Build inverted index: hash → Vec<dir_id>
    let mut inverted_index: AHashMap<i64, Vec<i64>> = AHashMap::new();
    let mut dir_hash_sets: AHashMap<i64, AHashSet<i64>> = AHashMap::new();

    for (dir_id, hashes) in &fingerprints {
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

            // Estimate shared bytes (sum of file sizes for shared hashes)
            // This is an approximation; exact value would need file size lookup
            let shared_bytes = intersection_size as i64;

            Some((dir_a, dir_b, jaccard, shared_bytes, match_type))
        })
        .collect();

    // Write results to database
    let mut similarity_count = 0;
    for (dir_a, dir_b, score, shared_bytes, match_type) in &results {
        db.insert_directory_similarity(*dir_a, *dir_b, *score, *shared_bytes, match_type)?;
        similarity_count += 1;
    }

    // Also find exact matches via content_fingerprint
    let exact_count = find_exact_matches(db)?;

    info!(
        "Computed {} similarity pairs ({} from Jaccard, {} exact fingerprint matches)",
        similarity_count + exact_count,
        similarity_count,
        exact_count,
    );

    Ok(similarity_count + exact_count)
}

/// Find exact directory duplicates via matching content_fingerprint.
fn find_exact_matches(db: &Database) -> Result<usize, crate::Error> {
    // Find fingerprints that appear more than once
    let mut stmt = db.connection().prepare(
        "SELECT df1.directory_id, df2.directory_id \
         FROM directory_fingerprint df1 \
         JOIN directory_fingerprint df2 ON df1.content_fingerprint = df2.content_fingerprint \
         WHERE df1.directory_id < df2.directory_id",
    )?;

    let pairs: Vec<(i64, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut count = 0;
    for (dir_a, dir_b) in &pairs {
        // Only insert if not already present
        let existing: i64 = db.connection().query_row(
            "SELECT COUNT(*) FROM directory_similarity WHERE dir_a_id = ?1 AND dir_b_id = ?2",
            params![dir_a, dir_b],
            |row| row.get(0),
        )?;

        if existing == 0 {
            // Get shared bytes from directory total_size
            let shared_bytes: i64 = db.connection().query_row(
                "SELECT COALESCE(total_size, 0) FROM directory_node WHERE id = ?1",
                params![dir_a],
                |row| row.get(0),
            )?;

            db.insert_directory_similarity(*dir_a, *dir_b, 1.0, shared_bytes, "exact")?;
            count += 1;
        }
    }

    Ok(count)
}
