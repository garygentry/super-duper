use super::models::*;
use super::sqlite::Database;
use rusqlite::{params, Result};
use tracing::debug;

impl Database {
    // ── Scan Session ─────────────────────────────────────────────

    pub fn create_scan_session(&self, root_paths: &[String]) -> Result<i64> {
        let paths_json = serde_json::to_string(root_paths).unwrap_or_default();
        let now = chrono::Utc::now().to_rfc3339();
        self.connection().execute(
            "INSERT INTO scan_session (started_at, status, root_paths) VALUES (?1, 'running', ?2)",
            params![now, paths_json],
        )?;
        Ok(self.connection().last_insert_rowid())
    }

    pub fn complete_scan_session(
        &self,
        session_id: i64,
        files_scanned: i64,
        total_bytes: i64,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.connection().execute(
            "UPDATE scan_session SET completed_at = ?1, status = 'completed', \
             files_scanned = ?2, total_bytes = ?3 WHERE id = ?4",
            params![now, files_scanned, total_bytes, session_id],
        )?;
        Ok(())
    }

    // ── Scanned Files ────────────────────────────────────────────

    pub fn insert_scanned_files(&self, files: &[ScannedFile]) -> Result<usize> {
        let tx = self.connection().unchecked_transaction()?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR IGNORE INTO scanned_file \
                 (canonical_path, file_name, parent_dir, drive_letter, file_size, \
                  last_modified, partial_hash, content_hash, scan_session_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for file in files {
                count += stmt.execute(params![
                    file.canonical_path,
                    file.file_name,
                    file.parent_dir,
                    file.drive_letter,
                    file.file_size,
                    file.last_modified,
                    file.partial_hash,
                    file.content_hash,
                    file.scan_session_id,
                ])?;
            }
        }
        tx.commit()?;
        debug!("Inserted {} scanned files", count);
        Ok(count)
    }

    // ── Duplicate Groups ─────────────────────────────────────────

    /// Insert duplicate groups. Each entry is (content_hash, file_size, Vec<canonical_path>).
    pub fn insert_duplicate_groups(
        &self,
        content_hash_groups: &[(i64, i64, Vec<String>)],
    ) -> Result<usize> {
        let tx = self.connection().unchecked_transaction()?;
        let mut group_count = 0;
        {
            let mut group_stmt = tx.prepare_cached(
                "INSERT INTO duplicate_group (content_hash, file_size, file_count, wasted_bytes) \
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            let mut member_stmt = tx.prepare_cached(
                "INSERT INTO duplicate_group_member (group_id, file_id) \
                 SELECT ?1, id FROM scanned_file WHERE canonical_path = ?2",
            )?;

            for (content_hash, file_size, paths) in content_hash_groups {
                let file_count = paths.len() as i64;
                let wasted_bytes = file_size * (file_count - 1);
                group_stmt.execute(params![content_hash, file_size, file_count, wasted_bytes])?;
                let group_id = tx.last_insert_rowid();
                for path in paths {
                    member_stmt.execute(params![group_id, path])?;
                }
                group_count += 1;
            }
        }
        tx.commit()?;
        debug!("Inserted {} duplicate groups", group_count);
        Ok(group_count)
    }

    // ── Paginated Queries ────────────────────────────────────────

    pub fn get_duplicate_groups(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<DuplicateGroup>> {
        let mut stmt = self.connection().prepare(
            "SELECT id, content_hash, file_size, file_count, wasted_bytes \
             FROM duplicate_group ORDER BY wasted_bytes DESC LIMIT ?1 OFFSET ?2",
        )?;
        let groups = stmt
            .query_map(params![limit, offset], |row| {
                Ok(DuplicateGroup {
                    id: row.get(0)?,
                    content_hash: row.get(1)?,
                    file_size: row.get(2)?,
                    file_count: row.get(3)?,
                    wasted_bytes: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(groups)
    }

    pub fn get_files_in_group(&self, group_id: i64) -> Result<Vec<ScannedFile>> {
        let mut stmt = self.connection().prepare(
            "SELECT sf.id, sf.canonical_path, sf.file_name, sf.parent_dir, sf.drive_letter, \
                    sf.file_size, sf.last_modified, sf.partial_hash, sf.content_hash, \
                    sf.scan_session_id, sf.marked_deleted \
             FROM scanned_file sf \
             JOIN duplicate_group_member dgm ON sf.id = dgm.file_id \
             WHERE dgm.group_id = ?1",
        )?;
        let files = stmt
            .query_map(params![group_id], |row| {
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
                    scan_session_id: row.get(9)?,
                    marked_deleted: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(files)
    }

    pub fn get_duplicate_group_count(&self) -> Result<i64> {
        self.connection()
            .query_row("SELECT COUNT(*) FROM duplicate_group", [], |row| {
                row.get(0)
            })
    }

    pub fn get_total_wasted_bytes(&self) -> Result<i64> {
        self.connection().query_row(
            "SELECT COALESCE(SUM(wasted_bytes), 0) FROM duplicate_group",
            [],
            |row| row.get(0),
        )
    }

    // ── Directory Nodes ──────────────────────────────────────────

    pub fn insert_directory_node(
        &self,
        path: &str,
        name: &str,
        parent_id: Option<i64>,
        total_size: i64,
        file_count: i64,
        depth: i64,
    ) -> Result<i64> {
        self.connection().execute(
            "INSERT OR IGNORE INTO directory_node (path, name, parent_id, total_size, file_count, depth) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![path, name, parent_id, total_size, file_count, depth],
        )?;
        Ok(self.connection().last_insert_rowid())
    }

    pub fn get_directory_children(
        &self,
        parent_id: Option<i64>,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<DirectoryNode>> {
        let mut stmt = if parent_id.is_some() {
            self.connection().prepare(
                "SELECT id, path, name, parent_id, total_size, file_count, depth \
                 FROM directory_node WHERE parent_id = ?1 \
                 ORDER BY total_size DESC LIMIT ?2 OFFSET ?3",
            )?
        } else {
            self.connection().prepare(
                "SELECT id, path, name, parent_id, total_size, file_count, depth \
                 FROM directory_node WHERE parent_id IS NULL \
                 ORDER BY total_size DESC LIMIT ?2 OFFSET ?3",
            )?
        };

        let nodes = stmt
            .query_map(params![parent_id, limit, offset], |row| {
                Ok(DirectoryNode {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    parent_id: row.get(3)?,
                    total_size: row.get(4)?,
                    file_count: row.get(5)?,
                    depth: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(nodes)
    }

    // ── Directory Fingerprints & Similarity ──────────────────────

    pub fn insert_directory_fingerprint(
        &self,
        directory_id: i64,
        content_fingerprint: &str,
        file_hash_set: &str,
    ) -> Result<()> {
        self.connection().execute(
            "INSERT OR REPLACE INTO directory_fingerprint \
             (directory_id, content_fingerprint, file_hash_set) VALUES (?1, ?2, ?3)",
            params![directory_id, content_fingerprint, file_hash_set],
        )?;
        Ok(())
    }

    pub fn insert_directory_similarity(
        &self,
        dir_a_id: i64,
        dir_b_id: i64,
        similarity_score: f64,
        shared_bytes: i64,
        match_type: &str,
    ) -> Result<()> {
        let (a, b) = if dir_a_id < dir_b_id {
            (dir_a_id, dir_b_id)
        } else {
            (dir_b_id, dir_a_id)
        };
        self.connection().execute(
            "INSERT OR REPLACE INTO directory_similarity \
             (dir_a_id, dir_b_id, similarity_score, shared_bytes, match_type) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![a, b, similarity_score, shared_bytes, match_type],
        )?;
        Ok(())
    }

    pub fn get_similar_directories(
        &self,
        min_score: f64,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<DirectorySimilarity>> {
        let mut stmt = self.connection().prepare(
            "SELECT id, dir_a_id, dir_b_id, similarity_score, shared_bytes, match_type \
             FROM directory_similarity WHERE similarity_score >= ?1 \
             ORDER BY similarity_score DESC LIMIT ?2 OFFSET ?3",
        )?;
        let pairs = stmt
            .query_map(params![min_score, limit, offset], |row| {
                Ok(DirectorySimilarity {
                    id: row.get(0)?,
                    dir_a_id: row.get(1)?,
                    dir_b_id: row.get(2)?,
                    similarity_score: row.get(3)?,
                    shared_bytes: row.get(4)?,
                    match_type: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(pairs)
    }

    // ── Deletion Planning ────────────────────────────────────────

    pub fn mark_file_for_deletion(
        &self,
        file_id: i64,
        strategy: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.connection().execute(
            "INSERT OR REPLACE INTO deletion_plan (file_id, marked_at, strategy) \
             VALUES (?1, ?2, ?3)",
            params![file_id, now, strategy],
        )?;
        Ok(())
    }

    pub fn unmark_file_for_deletion(&self, file_id: i64) -> Result<()> {
        self.connection().execute(
            "DELETE FROM deletion_plan WHERE file_id = ?1",
            params![file_id],
        )?;
        Ok(())
    }

    pub fn get_deletion_plan(&self) -> Result<Vec<DeletionPlanEntry>> {
        let mut stmt = self.connection().prepare(
            "SELECT id, file_id, marked_at, strategy, executed_at, execution_result \
             FROM deletion_plan WHERE executed_at IS NULL",
        )?;
        let entries = stmt
            .query_map([], |row| {
                Ok(DeletionPlanEntry {
                    id: row.get(0)?,
                    file_id: row.get(1)?,
                    marked_at: row.get(2)?,
                    strategy: row.get(3)?,
                    executed_at: row.get(4)?,
                    execution_result: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(entries)
    }

    pub fn get_deletion_plan_summary(&self) -> Result<(i64, i64)> {
        self.connection().query_row(
            "SELECT COUNT(*), COALESCE(SUM(sf.file_size), 0) \
             FROM deletion_plan dp \
             JOIN scanned_file sf ON dp.file_id = sf.id \
             WHERE dp.executed_at IS NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
    }
}
