use super::models::*;
use super::sqlite::Database;
use rusqlite::{params, Result};
use tracing::debug;

impl Database {
    // ── Scan Session ─────────────────────────────────────────────

    pub fn create_scan_session(&self, root_paths: &[String]) -> Result<i64> {
        let mut sorted = root_paths.to_vec();
        sorted.sort();
        let paths_json = serde_json::to_string(&sorted).unwrap_or_default();
        let now = chrono::Utc::now().to_rfc3339();
        self.connection().execute(
            "INSERT INTO scan_session (started_at, status, root_paths, root_paths_hash) \
             VALUES (?1, 'running', ?2, ?3)",
            params![now, paths_json, paths_json],
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

    /// Find an existing completed session with the same sorted root paths, or create a new one.
    /// If found, deletes its old duplicate groups (they'll be rebuilt by the current scan)
    /// and resets its status to 'running'.
    pub fn find_or_create_session(&self, root_paths: &[String]) -> Result<i64> {
        let mut sorted = root_paths.to_vec();
        sorted.sort();
        let paths_json = serde_json::to_string(&sorted).unwrap_or_default();

        match self.find_session_by_paths_hash(&paths_json)? {
            Some(session_id) => {
                self.delete_duplicate_groups_for_session(session_id)?;
                self.reset_scan_session(session_id)?;
                debug!("Reusing session {} for paths: {}", session_id, paths_json);
                Ok(session_id)
            }
            None => {
                let now = chrono::Utc::now().to_rfc3339();
                self.connection().execute(
                    "INSERT INTO scan_session (started_at, status, root_paths, root_paths_hash) \
                     VALUES (?1, 'running', ?2, ?3)",
                    params![now, paths_json, paths_json],
                )?;
                let id = self.connection().last_insert_rowid();
                debug!("Created new session {} for paths: {}", id, paths_json);
                Ok(id)
            }
        }
    }

    /// Find the id of the most recent completed session with the given paths hash.
    pub fn find_session_by_paths_hash(&self, hash: &str) -> Result<Option<i64>> {
        match self.connection().query_row(
            "SELECT id FROM scan_session \
             WHERE root_paths_hash = ?1 AND status = 'completed' \
             ORDER BY id DESC LIMIT 1",
            params![hash],
            |row| row.get(0),
        ) {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Delete all duplicate groups (and their members via CASCADE) for a session.
    pub fn delete_duplicate_groups_for_session(&self, session_id: i64) -> Result<()> {
        self.connection().execute(
            "DELETE FROM duplicate_group WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    /// Reset a session to 'running' state, clearing completion timestamps and stats.
    pub fn reset_scan_session(&self, session_id: i64) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.connection().execute(
            "UPDATE scan_session SET started_at = ?1, completed_at = NULL, \
             status = 'running', files_scanned = 0, total_bytes = 0 WHERE id = ?2",
            params![now, session_id],
        )?;
        Ok(())
    }

    /// Get the id of the most recent completed session, if any.
    /// List scan sessions ordered newest-first, with per-session duplicate group counts.
    /// Returns (sessions_with_group_count, total_session_count).
    pub fn list_sessions(&self, offset: i64, limit: i64) -> Result<(Vec<(ScanSession, i64)>, i64)> {
        let total: i64 = self
            .connection()
            .query_row("SELECT COUNT(*) FROM scan_session", [], |row| row.get(0))?;

        let mut stmt = self.connection().prepare(
            "SELECT ss.id, ss.started_at, ss.completed_at, ss.status, ss.root_paths, \
                    ss.files_scanned, ss.total_bytes, COUNT(dg.id) as group_count \
             FROM scan_session ss \
             LEFT JOIN duplicate_group dg ON dg.session_id = ss.id \
             GROUP BY ss.id \
             ORDER BY ss.id DESC \
             LIMIT ?1 OFFSET ?2",
        )?;

        let sessions = stmt
            .query_map(params![limit, offset], |row| {
                Ok((
                    ScanSession {
                        id: row.get(0)?,
                        started_at: row.get(1)?,
                        completed_at: row.get(2)?,
                        status: row.get(3)?,
                        root_paths: row.get(4)?,
                        files_scanned: row.get(5)?,
                        total_bytes: row.get(6)?,
                    },
                    row.get::<_, i64>(7)?,
                ))
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok((sessions, total))
    }

    /// Delete a session and its duplicate groups (members cascade automatically).
    /// scanned_file rows are NOT deleted — they remain in the global file index.
    pub fn delete_session(&self, session_id: i64) -> Result<()> {
        self.connection().execute(
            "DELETE FROM duplicate_group WHERE session_id = ?1",
            params![session_id],
        )?;
        self.connection().execute(
            "DELETE FROM scan_session WHERE id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    pub fn get_latest_session_id(&self) -> Result<Option<i64>> {
        match self.connection().query_row(
            "SELECT id FROM scan_session WHERE status = 'completed' ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        ) {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // ── Scanned Files ────────────────────────────────────────────

    pub fn insert_scanned_files(&self, files: &[ScannedFile]) -> Result<usize> {
        let tx = self.connection().unchecked_transaction()?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO scanned_file \
                 (canonical_path, file_name, parent_dir, drive_letter, file_size, \
                  last_modified, partial_hash, content_hash, last_seen_session_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
                 ON CONFLICT(canonical_path) DO UPDATE SET \
                     file_name = excluded.file_name, \
                     parent_dir = excluded.parent_dir, \
                     drive_letter = excluded.drive_letter, \
                     file_size = excluded.file_size, \
                     last_modified = excluded.last_modified, \
                     partial_hash = excluded.partial_hash, \
                     content_hash = excluded.content_hash, \
                     last_seen_session_id = excluded.last_seen_session_id",
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
                    file.last_seen_session_id,
                ])?;
            }
        }
        tx.commit()?;
        debug!("Upserted {} scanned files", count);
        Ok(count)
    }

    // ── Duplicate Groups ─────────────────────────────────────────

    /// Insert duplicate groups for a session. Each entry is (content_hash, file_size, Vec<canonical_path>).
    pub fn insert_duplicate_groups(
        &self,
        session_id: i64,
        content_hash_groups: &[(i64, i64, Vec<String>)],
    ) -> Result<usize> {
        let tx = self.connection().unchecked_transaction()?;
        let mut group_count = 0;
        {
            let mut group_stmt = tx.prepare_cached(
                "INSERT INTO duplicate_group \
                 (session_id, content_hash, file_size, file_count, wasted_bytes) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            let mut member_stmt = tx.prepare_cached(
                "INSERT INTO duplicate_group_member (group_id, file_id) \
                 SELECT ?1, id FROM scanned_file WHERE canonical_path = ?2",
            )?;

            for (content_hash, file_size, paths) in content_hash_groups {
                let file_count = paths.len() as i64;
                let wasted_bytes = file_size * (file_count - 1);
                group_stmt.execute(params![
                    session_id,
                    content_hash,
                    file_size,
                    file_count,
                    wasted_bytes
                ])?;
                let group_id = tx.last_insert_rowid();
                for path in paths {
                    member_stmt.execute(params![group_id, path])?;
                }
                group_count += 1;
            }
        }
        tx.commit()?;
        debug!("Inserted {} duplicate groups for session {}", group_count, session_id);
        Ok(group_count)
    }

    // ── Paginated Queries ────────────────────────────────────────

    pub fn get_duplicate_groups(
        &self,
        session_id: i64,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<DuplicateGroup>> {
        let mut stmt = self.connection().prepare(
            "SELECT id, session_id, content_hash, file_size, file_count, wasted_bytes \
             FROM duplicate_group WHERE session_id = ?1 \
             ORDER BY wasted_bytes DESC LIMIT ?2 OFFSET ?3",
        )?;
        let groups = stmt
            .query_map(params![session_id, limit, offset], |row| {
                Ok(DuplicateGroup {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    content_hash: row.get(2)?,
                    file_size: row.get(3)?,
                    file_count: row.get(4)?,
                    wasted_bytes: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(groups)
    }

    pub fn get_files_in_group(&self, group_id: i64) -> Result<Vec<ScannedFile>> {
        let mut stmt = self.connection().prepare(
            "SELECT sf.id, sf.canonical_path, sf.file_name, sf.parent_dir, sf.drive_letter, \
                    sf.file_size, sf.last_modified, sf.partial_hash, sf.content_hash, \
                    sf.last_seen_session_id, sf.marked_deleted \
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
                    last_seen_session_id: row.get(9)?,
                    marked_deleted: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(files)
    }

    pub fn get_duplicate_group_count(&self, session_id: i64) -> Result<i64> {
        self.connection().query_row(
            "SELECT COUNT(*) FROM duplicate_group WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )
    }

    pub fn get_total_wasted_bytes(&self, session_id: i64) -> Result<i64> {
        self.connection().query_row(
            "SELECT COALESCE(SUM(wasted_bytes), 0) FROM duplicate_group WHERE session_id = ?1",
            params![session_id],
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
            "SELECT ds.id, ds.dir_a_id, ds.dir_b_id, ds.similarity_score, ds.shared_bytes, \
                    ds.match_type, dn_a.path, dn_b.path \
             FROM directory_similarity ds \
             JOIN directory_node dn_a ON dn_a.id = ds.dir_a_id \
             JOIN directory_node dn_b ON dn_b.id = ds.dir_b_id \
             WHERE ds.similarity_score >= ?1 \
             ORDER BY ds.similarity_score DESC LIMIT ?2 OFFSET ?3",
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
                    dir_a_path: row.get(6)?,
                    dir_b_path: row.get(7)?,
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

    pub fn is_file_marked_for_deletion(&self, file_id: i64) -> Result<bool> {
        let count: i64 = self.connection().query_row(
            "SELECT COUNT(*) FROM deletion_plan WHERE file_id = ?1 AND executed_at IS NULL",
            params![file_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
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
