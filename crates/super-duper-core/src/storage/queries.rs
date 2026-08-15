use super::models::*;
use super::sqlite::Database;
use chrono::Utc;
use rusqlite::{params, Error, Result};
use std::sync::atomic::{AtomicBool, Ordering};

impl Database {
    // -- Session definitions -------------------------------------------------

    pub fn create_session(
        &self,
        name: &str,
        roots: &[String],
        ignore_patterns: &[String],
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        let roots_json = json(roots)?;
        let ignores_json = json(ignore_patterns)?;
        self.connection().execute(
            "INSERT INTO scan_session
                (name, roots_json, ignore_patterns_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![name.trim(), roots_json, ignores_json, now],
        )?;
        Ok(self.connection().last_insert_rowid())
    }

    pub fn update_session(
        &self,
        session_id: i64,
        name: &str,
        roots: &[String],
        ignore_patterns: &[String],
    ) -> Result<()> {
        let changed = self.connection().execute(
            "UPDATE scan_session SET name = ?1, roots_json = ?2,
                    ignore_patterns_json = ?3, updated_at = ?4
             WHERE id = ?5",
            params![
                name.trim(),
                json(roots)?,
                json(ignore_patterns)?,
                Utc::now().to_rfc3339(),
                session_id
            ],
        )?;
        changed_one(changed)
    }

    pub fn get_session(&self, session_id: i64) -> Result<ScanSession> {
        self.connection().query_row(
            "SELECT id, name, roots_json, ignore_patterns_json, created_at, updated_at
             FROM scan_session WHERE id = ?1",
            params![session_id],
            map_session,
        )
    }

    pub fn list_sessions(&self, offset: i64, limit: i64) -> Result<(Vec<ScanSession>, i64)> {
        let total =
            self.connection()
                .query_row("SELECT COUNT(*) FROM scan_session", [], |row| row.get(0))?;
        let mut stmt = self.connection().prepare(
            "SELECT id, name, roots_json, ignore_patterns_json, created_at, updated_at
             FROM scan_session ORDER BY name COLLATE NOCASE, id LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt
            .query_map(params![limit, offset], map_session)?
            .collect::<Result<Vec<_>>>()?;
        Ok((rows, total))
    }

    pub fn ensure_default_session(
        &self,
        roots: &[String],
        ignore_patterns: &[String],
    ) -> Result<i64> {
        let existing = self.connection().query_row(
            "SELECT id FROM scan_session WHERE name = 'Default' COLLATE NOCASE",
            [],
            |row| row.get(0),
        );
        match existing {
            Ok(id) => {
                self.update_session(id, "Default", roots, ignore_patterns)?;
                Ok(id)
            }
            Err(Error::QueryReturnedNoRows) => {
                self.create_session("Default", roots, ignore_patterns)
            }
            Err(error) => Err(error),
        }
    }

    pub fn delete_session(&self, session_id: i64) -> Result<()> {
        changed_one(self.connection().execute(
            "DELETE FROM scan_session WHERE id = ?1",
            params![session_id],
        )?)
    }

    // -- Immutable scan runs and lifecycle ----------------------------------

    pub fn create_scan_run(
        &self,
        session_id: i64,
        parameters: &RunParameters,
        engine_version: &str,
    ) -> Result<i64> {
        self.connection().execute(
            "INSERT INTO scan_run
                (session_id, parameters_json, status, created_at, engine_version)
             VALUES (?1, ?2, 'pending', ?3, ?4)",
            params![
                session_id,
                serde_json::to_string(parameters).map_err(json_error)?,
                Utc::now().to_rfc3339(),
                engine_version
            ],
        )?;
        Ok(self.connection().last_insert_rowid())
    }

    pub fn start_scan_run(&self, run_id: i64) -> Result<()> {
        changed_one(self.connection().execute(
            "UPDATE scan_run SET status = 'running', phase = 'discovering', started_at = ?1
             WHERE id = ?2 AND status = 'pending'",
            params![Utc::now().to_rfc3339(), run_id],
        )?)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_run_progress(
        &self,
        run_id: i64,
        phase: &str,
        files_discovered: i64,
        bytes_discovered: i64,
        files_hashed: i64,
        warning_count: i64,
    ) -> Result<()> {
        changed_one(self.connection().execute(
            "UPDATE scan_run SET phase = ?1, files_discovered = ?2,
                    bytes_discovered = ?3, files_hashed = ?4, warning_count = ?5
             WHERE id = ?6 AND status IN ('running', 'cancelling')",
            params![
                phase,
                files_discovered,
                bytes_discovered,
                files_hashed,
                warning_count,
                run_id
            ],
        )?)
    }

    pub fn mark_run_cancelling(&self, run_id: i64) -> Result<()> {
        let changed = self.connection().execute(
            "UPDATE scan_run SET status = 'cancelling'
             WHERE id = ?1 AND status = 'running'",
            params![run_id],
        )?;
        if changed == 0 {
            let status: String = self.connection().query_row(
                "SELECT status FROM scan_run WHERE id = ?1",
                params![run_id],
                |row| row.get(0),
            )?;
            if status != "cancelling" {
                return Err(Error::InvalidQuery);
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn complete_scan_run(
        &self,
        run_id: i64,
        files_discovered: i64,
        bytes_discovered: i64,
        files_hashed: i64,
        duplicate_file_groups: i64,
        duplicate_folder_groups: i64,
        wasted_bytes: i64,
        warning_count: i64,
    ) -> Result<()> {
        changed_one(self.connection().execute(
            "UPDATE scan_run SET status = 'completed', phase = 'finalizing', completed_at = ?1,
                    files_discovered = ?2, bytes_discovered = ?3, files_hashed = ?4,
                    duplicate_file_groups = ?5, duplicate_folder_groups = ?6,
                    wasted_bytes = ?7, warning_count = ?8, error_message = NULL
             WHERE id = ?9 AND status = 'running'",
            params![
                Utc::now().to_rfc3339(),
                files_discovered,
                bytes_discovered,
                files_hashed,
                duplicate_file_groups,
                duplicate_folder_groups,
                wasted_bytes,
                warning_count,
                run_id
            ],
        )?)
    }

    pub fn cancel_scan_run(&self, run_id: i64) -> Result<()> {
        terminal_run(self, run_id, "cancelled", None, &["running", "cancelling"])
    }

    pub fn fail_scan_run(&self, run_id: i64, message: &str) -> Result<()> {
        terminal_run(
            self,
            run_id,
            "failed",
            Some(message),
            &["pending", "running", "cancelling"],
        )
    }

    pub fn interrupt_scan_run(&self, run_id: i64, message: &str) -> Result<()> {
        terminal_run(
            self,
            run_id,
            "interrupted",
            Some(message),
            &["running", "cancelling"],
        )
    }

    pub fn get_scan_run(&self, run_id: i64) -> Result<ScanRun> {
        let sql = RUN_SELECT.to_owned() + " WHERE id = ?1";
        self.connection().query_row(&sql, params![run_id], map_run)
    }

    pub fn list_runs(&self, offset: i64, limit: i64) -> Result<(Vec<ScanRun>, i64)> {
        let total = self
            .connection()
            .query_row("SELECT COUNT(*) FROM scan_run", [], |row| row.get(0))?;
        let sql = RUN_SELECT.to_owned() + " ORDER BY id DESC LIMIT ?1 OFFSET ?2";
        let mut stmt = self.connection().prepare(&sql)?;
        let runs = stmt
            .query_map(params![limit, offset], map_run)?
            .collect::<Result<Vec<_>>>()?;
        Ok((runs, total))
    }

    pub fn list_session_runs(
        &self,
        session_id: i64,
        offset: i64,
        limit: i64,
    ) -> Result<(Vec<ScanRun>, i64)> {
        let total = self.connection().query_row(
            "SELECT COUNT(*) FROM scan_run WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )?;
        let sql =
            RUN_SELECT.to_owned() + " WHERE session_id = ?1 ORDER BY id DESC LIMIT ?2 OFFSET ?3";
        let mut stmt = self.connection().prepare(&sql)?;
        let runs = stmt
            .query_map(params![session_id, limit, offset], map_run)?
            .collect::<Result<Vec<_>>>()?;
        Ok((runs, total))
    }

    pub fn get_latest_completed_run_id(&self) -> Result<Option<i64>> {
        optional_id(self.connection().query_row(
            "SELECT id FROM scan_run WHERE status = 'completed' ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        ))
    }

    pub fn delete_run(&self, run_id: i64) -> Result<()> {
        changed_one(self.connection().execute(
            "DELETE FROM scan_run WHERE id = ?1 AND status NOT IN ('running', 'cancelling')",
            params![run_id],
        )?)
    }

    // -- Run-scoped file and duplicate results -------------------------------

    pub fn insert_scanned_files(&self, files: &[ScannedFile]) -> Result<usize> {
        let tx = self.connection().unchecked_transaction()?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO scanned_file
                    (run_id, root_path, canonical_path, relative_path, file_name, parent_dir,
                     drive_letter, file_size, last_modified, partial_hash, content_hash,
                     file_identity, warning_message, marked_deleted)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )?;
            for file in files {
                count += stmt.execute(params![
                    file.run_id,
                    file.root_path,
                    file.canonical_path,
                    file.relative_path,
                    file.file_name,
                    file.parent_dir,
                    file.drive_letter,
                    file.file_size,
                    file.last_modified,
                    file.partial_hash,
                    file.content_hash,
                    file.file_identity,
                    file.warning_message,
                    file.marked_deleted,
                ])?;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    pub fn insert_duplicate_groups(
        &self,
        run_id: i64,
        content_hash_groups: &[(i64, i64, Vec<String>)],
    ) -> Result<usize> {
        self.insert_duplicate_groups_cancellable(
            run_id,
            content_hash_groups,
            &AtomicBool::new(false),
        )
        .map_err(|error| match error {
            crate::Error::Database(error) => error,
            _ => Error::InvalidQuery,
        })
    }

    pub fn insert_duplicate_groups_cancellable(
        &self,
        run_id: i64,
        content_hash_groups: &[(i64, i64, Vec<String>)],
        cancel_token: &AtomicBool,
    ) -> std::result::Result<usize, crate::Error> {
        let tx = self.connection().unchecked_transaction()?;
        let mut group_count = 0;
        {
            let mut group_stmt = tx.prepare_cached(
                "INSERT INTO duplicate_group
                    (run_id, content_hash, file_size, file_count, wasted_bytes)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            let mut member_stmt = tx.prepare_cached(
                "INSERT INTO duplicate_group_member (group_id, file_id)
                 SELECT ?1, id FROM scanned_file WHERE run_id = ?2 AND canonical_path = ?3",
            )?;
            for (content_hash, file_size, paths) in content_hash_groups {
                if cancel_token.load(Ordering::Relaxed) {
                    return Err(crate::Error::Cancelled);
                }
                let file_count = paths.len() as i64;
                group_stmt.execute(params![
                    run_id,
                    content_hash,
                    file_size,
                    file_count,
                    file_size * (file_count - 1)
                ])?;
                let group_id = tx.last_insert_rowid();
                for path in paths {
                    if cancel_token.load(Ordering::Relaxed) {
                        return Err(crate::Error::Cancelled);
                    }
                    if member_stmt.execute(params![group_id, run_id, path])? != 1 {
                        return Err(crate::Error::Database(Error::InvalidQuery));
                    }
                }
                group_count += 1;
            }
        }
        tx.commit()?;
        Ok(group_count)
    }

    pub fn get_duplicate_groups(
        &self,
        run_id: i64,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<DuplicateGroup>> {
        let mut stmt = self.connection().prepare(
            "SELECT id, run_id, content_hash, file_size, file_count, wasted_bytes
             FROM duplicate_group WHERE run_id = ?1
             ORDER BY wasted_bytes DESC, id LIMIT ?2 OFFSET ?3",
        )?;
        let mapped = stmt.query_map(params![run_id, limit, offset], |row| {
            Ok(DuplicateGroup {
                id: row.get(0)?,
                run_id: row.get(1)?,
                content_hash: row.get(2)?,
                file_size: row.get(3)?,
                file_count: row.get(4)?,
                wasted_bytes: row.get(5)?,
            })
        })?;
        mapped.collect()
    }

    pub fn get_files_in_group(&self, group_id: i64) -> Result<Vec<ScannedFile>> {
        let mut stmt = self.connection().prepare(
            "SELECT sf.id, sf.run_id, sf.root_path, sf.canonical_path, sf.relative_path,
                    sf.file_name, sf.parent_dir, sf.drive_letter, sf.file_size,
                    sf.last_modified, sf.partial_hash, sf.content_hash, sf.file_identity,
                    sf.warning_message, sf.marked_deleted
             FROM scanned_file sf
             JOIN duplicate_group_member dgm ON sf.id = dgm.file_id
             JOIN duplicate_group dg ON dg.id = dgm.group_id AND dg.run_id = sf.run_id
             WHERE dgm.group_id = ?1 ORDER BY sf.canonical_path",
        )?;
        let mapped = stmt.query_map(params![group_id], map_file)?;
        mapped.collect()
    }

    pub fn get_duplicate_group_count(&self, run_id: i64) -> Result<i64> {
        self.connection().query_row(
            "SELECT COUNT(*) FROM duplicate_group WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )
    }

    pub fn get_total_wasted_bytes(&self, run_id: i64) -> Result<i64> {
        self.connection().query_row(
            "SELECT COALESCE(SUM(wasted_bytes), 0) FROM duplicate_group WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )
    }

    // -- Run-scoped directory preparation -----------------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn insert_directory_node(
        &self,
        run_id: i64,
        path: &str,
        name: &str,
        parent_id: Option<i64>,
        total_size: i64,
        file_count: i64,
        depth: i64,
    ) -> Result<i64> {
        self.connection().execute(
            "INSERT OR IGNORE INTO directory_node
                (run_id, path, name, parent_id, total_size, file_count, depth)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![run_id, path, name, parent_id, total_size, file_count, depth],
        )?;
        self.connection().query_row(
            "SELECT id FROM directory_node WHERE run_id = ?1 AND path = ?2",
            params![run_id, path],
            |row| row.get(0),
        )
    }

    pub fn get_directory_children(
        &self,
        run_id: i64,
        parent_id: Option<i64>,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<DirectoryNode>> {
        let predicate = if parent_id.is_some() {
            "parent_id = ?2"
        } else {
            "parent_id IS ?2"
        };
        let sql = format!(
            "SELECT id, run_id, path, name, parent_id, total_size, file_count, depth
             FROM directory_node WHERE run_id = ?1 AND {predicate}
             ORDER BY total_size DESC, id LIMIT ?3 OFFSET ?4"
        );
        let mut stmt = self.connection().prepare(&sql)?;
        let mapped = stmt.query_map(params![run_id, parent_id, limit, offset], |row| {
            Ok(DirectoryNode {
                id: row.get(0)?,
                run_id: row.get(1)?,
                path: row.get(2)?,
                name: row.get(3)?,
                parent_id: row.get(4)?,
                total_size: row.get(5)?,
                file_count: row.get(6)?,
                depth: row.get(7)?,
            })
        })?;
        mapped.collect()
    }

    pub fn insert_directory_fingerprint(
        &self,
        directory_id: i64,
        content_fingerprint: &str,
        file_hash_set: &str,
    ) -> Result<()> {
        self.connection().execute(
            "INSERT OR REPLACE INTO directory_fingerprint
                (directory_id, content_fingerprint, file_hash_set) VALUES (?1, ?2, ?3)",
            params![directory_id, content_fingerprint, file_hash_set],
        )?;
        Ok(())
    }

    pub fn insert_directory_similarity(
        &self,
        run_id: i64,
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
            "INSERT OR REPLACE INTO directory_similarity
                (run_id, dir_a_id, dir_b_id, similarity_score, shared_bytes, match_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![run_id, a, b, similarity_score, shared_bytes, match_type],
        )?;
        Ok(())
    }

    pub fn get_similar_directories(
        &self,
        run_id: i64,
        min_score: f64,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<DirectorySimilarity>> {
        let mut stmt = self.connection().prepare(
            "SELECT ds.id, ds.run_id, ds.dir_a_id, ds.dir_b_id, ds.similarity_score,
                    ds.shared_bytes, ds.match_type, dn_a.path, dn_b.path
             FROM directory_similarity ds
             JOIN directory_node dn_a ON dn_a.id = ds.dir_a_id AND dn_a.run_id = ds.run_id
             JOIN directory_node dn_b ON dn_b.id = ds.dir_b_id AND dn_b.run_id = ds.run_id
             WHERE ds.run_id = ?1 AND ds.similarity_score >= ?2
             ORDER BY ds.similarity_score DESC, ds.id LIMIT ?3 OFFSET ?4",
        )?;
        let mapped = stmt.query_map(params![run_id, min_score, limit, offset], |row| {
            Ok(DirectorySimilarity {
                id: row.get(0)?,
                run_id: row.get(1)?,
                dir_a_id: row.get(2)?,
                dir_b_id: row.get(3)?,
                similarity_score: row.get(4)?,
                shared_bytes: row.get(5)?,
                match_type: row.get(6)?,
                dir_a_path: row.get(7)?,
                dir_b_path: row.get(8)?,
            })
        })?;
        mapped.collect()
    }

    // -- Deletion planning (legacy core functionality; still run-file based) -

    pub fn mark_file_for_deletion(&self, file_id: i64, strategy: Option<&str>) -> Result<()> {
        self.connection().execute(
            "INSERT OR REPLACE INTO deletion_plan (file_id, marked_at, strategy)
             VALUES (?1, ?2, ?3)",
            params![file_id, Utc::now().to_rfc3339(), strategy],
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
            "SELECT id, file_id, marked_at, strategy, executed_at, execution_result
             FROM deletion_plan WHERE executed_at IS NULL",
        )?;
        let mapped = stmt.query_map([], |row| {
            Ok(DeletionPlanEntry {
                id: row.get(0)?,
                file_id: row.get(1)?,
                marked_at: row.get(2)?,
                strategy: row.get(3)?,
                executed_at: row.get(4)?,
                execution_result: row.get(5)?,
            })
        })?;
        mapped.collect()
    }

    pub fn is_file_marked_for_deletion(&self, file_id: i64) -> Result<bool> {
        Ok(self.connection().query_row(
            "SELECT COUNT(*) FROM deletion_plan WHERE file_id = ?1 AND executed_at IS NULL",
            params![file_id],
            |row| row.get::<_, i64>(0),
        )? > 0)
    }

    pub fn get_deletion_plan_summary(&self) -> Result<(i64, i64)> {
        self.connection().query_row(
            "SELECT COUNT(*), COALESCE(SUM(sf.file_size), 0)
             FROM deletion_plan dp JOIN scanned_file sf ON dp.file_id = sf.id
             WHERE dp.executed_at IS NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
    }
}

const RUN_SELECT: &str =
    "SELECT id, session_id, parameters_json, status, phase, created_at, started_at,
            completed_at, files_discovered, bytes_discovered, files_hashed,
            duplicate_file_groups, duplicate_folder_groups, wasted_bytes, warning_count,
            error_message, engine_version FROM scan_run";

fn map_session(row: &rusqlite::Row<'_>) -> Result<ScanSession> {
    Ok(ScanSession {
        id: row.get(0)?,
        name: row.get(1)?,
        roots_json: row.get(2)?,
        ignore_patterns_json: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn map_run(row: &rusqlite::Row<'_>) -> Result<ScanRun> {
    Ok(ScanRun {
        id: row.get(0)?,
        session_id: row.get(1)?,
        parameters_json: row.get(2)?,
        status: row.get(3)?,
        phase: row.get(4)?,
        created_at: row.get(5)?,
        started_at: row.get(6)?,
        completed_at: row.get(7)?,
        files_discovered: row.get(8)?,
        bytes_discovered: row.get(9)?,
        files_hashed: row.get(10)?,
        duplicate_file_groups: row.get(11)?,
        duplicate_folder_groups: row.get(12)?,
        wasted_bytes: row.get(13)?,
        warning_count: row.get(14)?,
        error_message: row.get(15)?,
        engine_version: row.get(16)?,
    })
}

fn map_file(row: &rusqlite::Row<'_>) -> Result<ScannedFile> {
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
}

fn terminal_run(
    db: &Database,
    run_id: i64,
    status: &str,
    message: Option<&str>,
    allowed: &[&str],
) -> Result<()> {
    let current: String = db.connection().query_row(
        "SELECT status FROM scan_run WHERE id = ?1",
        params![run_id],
        |row| row.get(0),
    )?;
    if !allowed.contains(&current.as_str()) {
        return Err(Error::InvalidQuery);
    }
    changed_one(db.connection().execute(
        "UPDATE scan_run SET status = ?1, phase = 'finalizing', completed_at = ?2,
                error_message = ?3 WHERE id = ?4 AND status = ?5",
        params![status, Utc::now().to_rfc3339(), message, run_id, current],
    )?)
}

fn optional_id(result: Result<i64>) -> Result<Option<i64>> {
    match result {
        Ok(id) => Ok(Some(id)),
        Err(Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error),
    }
}

fn changed_one(changed: usize) -> Result<()> {
    if changed == 1 {
        Ok(())
    } else {
        Err(Error::InvalidQuery)
    }
}

fn json<T: serde::Serialize + ?Sized>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(json_error)
}

fn json_error(error: serde_json::Error) -> Error {
    Error::ToSqlConversionFailure(Box::new(error))
}
