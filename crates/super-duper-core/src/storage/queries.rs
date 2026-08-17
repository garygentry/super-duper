use super::models::*;
use super::sqlite::Database;
use chrono::Utc;
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Error, Result};
use std::sync::atomic::{AtomicBool, Ordering};

impl Database {
    // -- Session definitions -------------------------------------------------

    pub fn create_session(
        &self,
        name: &str,
        roots: &[String],
        ignore_patterns: &[String],
    ) -> Result<i64> {
        self.create_session_with_cloud_settings(
            name,
            roots,
            ignore_patterns,
            CloudPolicy::ExcludeRegisteredRoots,
            &[],
            &[],
            CloudDetectionStatus::Unavailable,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_session_with_cloud_settings(
        &self,
        name: &str,
        roots: &[String],
        ignore_patterns: &[String],
        cloud_policy: CloudPolicy,
        manual_location_exclusions: &[String],
        registered_cloud_locations: &[RegisteredCloudLocation],
        cloud_detection_status: CloudDetectionStatus,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        let roots_json = json(roots)?;
        let ignores_json = json(ignore_patterns)?;
        self.connection().execute(
            "INSERT INTO scan_session
                (name, roots_json, ignore_patterns_json, cloud_policy,
                 manual_location_exclusions_json, registered_cloud_locations_json,
                 cloud_detection_status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                name.trim(),
                roots_json,
                ignores_json,
                cloud_policy.as_str(),
                json(manual_location_exclusions)?,
                json(registered_cloud_locations)?,
                cloud_detection_status.as_str(),
                now
            ],
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
        self.update_session_with_cloud_settings(
            session_id,
            name,
            roots,
            ignore_patterns,
            CloudPolicy::ExcludeRegisteredRoots,
            &[],
            &[],
            CloudDetectionStatus::Unavailable,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_session_with_cloud_settings(
        &self,
        session_id: i64,
        name: &str,
        roots: &[String],
        ignore_patterns: &[String],
        cloud_policy: CloudPolicy,
        manual_location_exclusions: &[String],
        registered_cloud_locations: &[RegisteredCloudLocation],
        cloud_detection_status: CloudDetectionStatus,
    ) -> Result<()> {
        let changed = self.connection().execute(
            "UPDATE scan_session SET name = ?1, roots_json = ?2,
                    ignore_patterns_json = ?3, cloud_policy = ?4,
                    manual_location_exclusions_json = ?5,
                    registered_cloud_locations_json = ?6,
                    cloud_detection_status = ?7, updated_at = ?8
             WHERE id = ?9",
            params![
                name.trim(),
                json(roots)?,
                json(ignore_patterns)?,
                cloud_policy.as_str(),
                json(manual_location_exclusions)?,
                json(registered_cloud_locations)?,
                cloud_detection_status.as_str(),
                Utc::now().to_rfc3339(),
                session_id
            ],
        )?;
        changed_one(changed)
    }

    pub fn get_session(&self, session_id: i64) -> Result<ScanSession> {
        self.connection().query_row(
            "SELECT id, name, roots_json, ignore_patterns_json, cloud_policy,
                    manual_location_exclusions_json, registered_cloud_locations_json,
                    cloud_detection_status, created_at, updated_at
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
            "SELECT id, name, roots_json, ignore_patterns_json, cloud_policy,
                    manual_location_exclusions_json, registered_cloud_locations_json,
                    cloud_detection_status, created_at, updated_at
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

    pub fn get_scanned_files(&self, run_id: i64) -> Result<Vec<ScannedFile>> {
        let mut statement = self.connection().prepare(
            "SELECT id, run_id, root_path, canonical_path, relative_path, file_name,
                    parent_dir, drive_letter, file_size, last_modified, partial_hash,
                    content_hash, file_identity, warning_message, marked_deleted
             FROM scanned_file WHERE run_id = ?1 ORDER BY id",
        )?;
        let files = statement
            .query_map(params![run_id], map_file)?
            .collect::<Result<Vec<_>>>()?;
        Ok(files)
    }

    pub fn update_scanned_file_content_hash(
        &self,
        run_id: i64,
        file_id: i64,
        hash: i64,
    ) -> Result<()> {
        changed_one(self.connection().execute(
            "UPDATE scanned_file SET content_hash = ?1 WHERE run_id = ?2 AND id = ?3",
            params![hash, run_id, file_id],
        )?)
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

    pub fn page_duplicate_file_groups(
        &self,
        query: &DuplicateFileGroupPageQuery,
    ) -> Result<DuplicateFileGroupPage> {
        let mut predicates = vec!["dg.run_id = ?".to_owned(), "dg.file_count > 1".to_owned()];
        let mut base_parameters = vec![SqlValue::Integer(query.run_id)];
        predicates.push("dg.file_size >= ?".to_owned());
        base_parameters.push(SqlValue::Integer(query.filter.minimum_size));
        if let Some(search) = query
            .filter
            .search
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            predicates.push(
                "EXISTS (
                    SELECT 1 FROM duplicate_group_member search_member
                    JOIN scanned_file search_file ON search_file.id = search_member.file_id
                    WHERE search_member.group_id = dg.id
                      AND search_file.run_id = dg.run_id
                      AND search_file.canonical_path LIKE ? ESCAPE '\\' COLLATE NOCASE
                )"
                .to_owned(),
            );
            base_parameters.push(SqlValue::Text(like_pattern(search)));
        }
        let where_sql = predicates.join(" AND ");
        let summary = self.connection().query_row(
            &format!(
                "SELECT COUNT(*), COALESCE(SUM(dg.file_count), 0),
                        COALESCE(SUM(dg.wasted_bytes), 0), COALESCE(MAX(dg.wasted_bytes), 0)
                 FROM duplicate_group dg WHERE {where_sql}"
            ),
            params_from_iter(base_parameters.iter()),
            |row| {
                Ok(DuplicateFileReviewSummary {
                    matching_group_count: row.get(0)?,
                    matching_copy_count: row.get(1)?,
                    potential_recoverable_bytes: row.get(2)?,
                    largest_recoverable_bytes: row.get(3)?,
                })
            },
        )?;
        let total = summary.matching_group_count;

        let sort_expression = match query.sort_field {
            DuplicateFileGroupSortField::RecoverableBytes => "recoverable_bytes",
            DuplicateFileGroupSortField::GroupSize => "file_size",
            DuplicateFileGroupSortField::CopyCount => "file_count",
            DuplicateFileGroupSortField::RepresentativeName => "representative_name COLLATE NOCASE",
        };
        let mut page_parameters = base_parameters;
        let mut cursor_clause = String::new();
        if let Some(cursor) = &query.cursor {
            let comparator = cursor_comparator(query.sort_direction, cursor.before);
            let id_comparator = cursor_comparator(SortDirection::Ascending, cursor.before);
            cursor_clause = format!(
                "WHERE ({sort_expression} {comparator} ? OR ({sort_expression} = ? AND id {id_comparator} ?))"
            );
            push_cursor_parameters(
                &mut page_parameters,
                cursor,
                query.sort_field == DuplicateFileGroupSortField::RepresentativeName,
            )?;
        }
        page_parameters.push(SqlValue::Integer(query.limit + 1));
        let order = effective_order(
            query.sort_direction,
            query.cursor.as_ref().is_some_and(|cursor| cursor.before),
        );
        let id_order = effective_order(
            SortDirection::Ascending,
            query.cursor.as_ref().is_some_and(|cursor| cursor.before),
        );
        let sql = format!(
            "WITH result_groups AS (
                SELECT dg.id, dg.run_id, dg.file_size, dg.file_count,
                       dg.wasted_bytes AS recoverable_bytes,
                       COALESCE((
                           SELECT sf.file_name
                           FROM duplicate_group_member representative_member
                           JOIN scanned_file sf ON sf.id = representative_member.file_id
                           WHERE representative_member.group_id = dg.id
                             AND sf.run_id = dg.run_id
                           ORDER BY sf.canonical_path COLLATE NOCASE, sf.id
                           LIMIT 1
                       ), '') AS representative_name
                FROM duplicate_group dg
                WHERE {where_sql}
             )
             SELECT id, run_id, file_size, file_count, recoverable_bytes, representative_name
             FROM result_groups
             {cursor_clause}
             ORDER BY {sort_expression} {order}, id {id_order}
             LIMIT ?"
        );
        let mut statement = self.connection().prepare(&sql)?;
        let mapped = statement.query_map(params_from_iter(page_parameters.iter()), |row| {
            Ok(DuplicateFileGroupResult {
                id: row.get(0)?,
                run_id: row.get(1)?,
                file_size: row.get(2)?,
                file_count: row.get(3)?,
                recoverable_bytes: row.get(4)?,
                representative_name: row.get(5)?,
            })
        })?;
        let mut groups = mapped.collect::<Result<Vec<_>>>()?;
        let has_more = groups.len() > query.limit as usize;
        if has_more {
            groups.pop();
        }
        if query.cursor.as_ref().is_some_and(|cursor| cursor.before) {
            groups.reverse();
        }
        Ok(DuplicateFileGroupPage {
            groups,
            total,
            summary,
            has_more,
        })
    }

    pub fn duplicate_file_group_exists(&self, run_id: i64, group_id: i64) -> Result<bool> {
        self.connection().query_row(
            "SELECT EXISTS(SELECT 1 FROM duplicate_group WHERE run_id = ?1 AND id = ?2)",
            params![run_id, group_id],
            |row| row.get(0),
        )
    }

    pub fn page_duplicate_file_members(
        &self,
        query: &DuplicateFileMemberPageQuery,
    ) -> Result<DuplicateFileMemberPage> {
        let mut predicates = vec!["dg.run_id = ?".to_owned(), "dg.id = ?".to_owned()];
        let mut base_parameters = vec![
            SqlValue::Integer(query.run_id),
            SqlValue::Integer(query.group_id),
        ];
        if let Some(search) = query
            .filter
            .search
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            predicates.push("sf.canonical_path LIKE ? ESCAPE '\\' COLLATE NOCASE".to_owned());
            base_parameters.push(SqlValue::Text(like_pattern(search)));
        }
        let where_sql = predicates.join(" AND ");
        let from_sql = "FROM duplicate_group dg
                        JOIN duplicate_group_member dgm ON dgm.group_id = dg.id
                        JOIN scanned_file sf ON sf.id = dgm.file_id AND sf.run_id = dg.run_id";
        let total: i64 = self.connection().query_row(
            &format!("SELECT COUNT(*) {from_sql} WHERE {where_sql}"),
            params_from_iter(base_parameters.iter()),
            |row| row.get(0),
        )?;

        let sort_expression = match query.sort_field {
            DuplicateFileMemberSortField::Path => "sf.canonical_path COLLATE NOCASE",
            DuplicateFileMemberSortField::ModifiedTime => "sf.last_modified",
            DuplicateFileMemberSortField::Size => "sf.file_size",
        };
        let mut page_parameters = base_parameters;
        let mut cursor_clause = String::new();
        if let Some(cursor) = &query.cursor {
            let comparator = cursor_comparator(query.sort_direction, cursor.before);
            let id_comparator = cursor_comparator(SortDirection::Ascending, cursor.before);
            cursor_clause = format!(
                "AND ({sort_expression} {comparator} ? OR ({sort_expression} = ? AND sf.id {id_comparator} ?))"
            );
            push_cursor_parameters(
                &mut page_parameters,
                cursor,
                query.sort_field == DuplicateFileMemberSortField::Path,
            )?;
        }
        page_parameters.push(SqlValue::Integer(query.limit + 1));
        let order = effective_order(
            query.sort_direction,
            query.cursor.as_ref().is_some_and(|cursor| cursor.before),
        );
        let id_order = effective_order(
            SortDirection::Ascending,
            query.cursor.as_ref().is_some_and(|cursor| cursor.before),
        );
        let sql = format!(
            "SELECT sf.id, dg.id, sf.canonical_path, sf.file_name, sf.parent_dir,
                    sf.root_path, sf.relative_path, sf.drive_letter,
                    sf.file_size, sf.last_modified
             {from_sql}
             WHERE {where_sql} {cursor_clause}
             ORDER BY {sort_expression} {order}, sf.id {id_order}
             LIMIT ?"
        );
        let mut statement = self.connection().prepare(&sql)?;
        let mapped = statement.query_map(params_from_iter(page_parameters.iter()), |row| {
            Ok(DuplicateFileMemberResult {
                id: row.get(0)?,
                group_id: row.get(1)?,
                canonical_path: row.get(2)?,
                file_name: row.get(3)?,
                parent_dir: row.get(4)?,
                root_path: row.get(5)?,
                relative_path: row.get(6)?,
                drive_letter: row.get(7)?,
                file_size: row.get(8)?,
                last_modified: row.get(9)?,
            })
        })?;
        let mut members = mapped.collect::<Result<Vec<_>>>()?;
        let has_more = members.len() > query.limit as usize;
        if has_more {
            members.pop();
        }
        if query.cursor.as_ref().is_some_and(|cursor| cursor.before) {
            members.reverse();
        }
        Ok(DuplicateFileMemberPage {
            members,
            total,
            has_more,
        })
    }

    pub fn replace_exact_folder_groups(
        &self,
        run_id: i64,
        groups: &[ExactFolderGroupInsert],
        cancel_token: &AtomicBool,
    ) -> std::result::Result<usize, crate::Error> {
        let tx = self.connection().unchecked_transaction()?;
        tx.execute(
            "DELETE FROM duplicate_folder_group WHERE run_id = ?1",
            params![run_id],
        )?;
        let mut visible_count = 0usize;
        {
            let mut group_statement = tx.prepare_cached(
                "INSERT INTO duplicate_folder_group
                    (run_id, structural_fingerprint, verified_fingerprint, total_size,
                     file_count, folder_count, is_suppressed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            let mut member_statement = tx.prepare_cached(
                "INSERT INTO duplicate_folder_group_member (group_id, directory_id)
                 SELECT ?1, id FROM directory_node WHERE run_id = ?2 AND id = ?3",
            )?;
            for group in groups {
                if cancel_token.load(Ordering::Relaxed) {
                    return Err(crate::Error::Cancelled);
                }
                group_statement.execute(params![
                    run_id,
                    group.structural_fingerprint,
                    group.verified_fingerprint,
                    group.total_size,
                    group.file_count,
                    group.directory_ids.len() as i64,
                    group.is_suppressed,
                ])?;
                let group_id = tx.last_insert_rowid();
                for directory_id in &group.directory_ids {
                    if member_statement.execute(params![group_id, run_id, directory_id])? != 1 {
                        return Err(crate::Error::Database(Error::InvalidQuery));
                    }
                }
                if !group.is_suppressed {
                    visible_count += 1;
                }
            }
        }
        tx.commit()?;
        Ok(visible_count)
    }

    pub fn duplicate_folder_group_exists(&self, run_id: i64, group_id: i64) -> Result<bool> {
        self.connection().query_row(
            "SELECT EXISTS(SELECT 1 FROM duplicate_folder_group
                           WHERE run_id = ?1 AND id = ?2 AND is_suppressed = 0)",
            params![run_id, group_id],
            |row| row.get(0),
        )
    }

    pub fn page_duplicate_folder_groups(
        &self,
        query: &DuplicateFolderGroupPageQuery,
    ) -> Result<DuplicateFolderGroupPage> {
        let mut predicates = vec![
            "dfg.run_id = ?".to_owned(),
            "dfg.is_suppressed = 0".to_owned(),
        ];
        let mut base_parameters = vec![SqlValue::Integer(query.run_id)];
        predicates.push("dfg.total_size >= ?".to_owned());
        base_parameters.push(SqlValue::Integer(query.filter.minimum_size));
        if let Some(search) = query
            .filter
            .search
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            predicates.push(
                "EXISTS (
                    SELECT 1 FROM duplicate_folder_group_member search_member
                    JOIN directory_node search_directory ON search_directory.id = search_member.directory_id
                    WHERE search_member.group_id = dfg.id
                      AND search_directory.run_id = dfg.run_id
                      AND search_directory.path LIKE ? ESCAPE '\\' COLLATE NOCASE
                )".to_owned(),
            );
            base_parameters.push(SqlValue::Text(like_pattern(search)));
        }
        let where_sql = predicates.join(" AND ");
        let total: i64 = self.connection().query_row(
            &format!("SELECT COUNT(*) FROM duplicate_folder_group dfg WHERE {where_sql}"),
            params_from_iter(base_parameters.iter()),
            |row| row.get(0),
        )?;
        let sort_expression = match query.sort_field {
            DuplicateFolderGroupSortField::TotalBytes => "total_size",
            DuplicateFolderGroupSortField::CopyCount => "folder_count",
            DuplicateFolderGroupSortField::FileCount => "file_count",
            DuplicateFolderGroupSortField::RepresentativePath => {
                "representative_path COLLATE NOCASE"
            }
        };
        let mut page_parameters = base_parameters;
        let mut cursor_clause = String::new();
        if let Some(cursor) = &query.cursor {
            let comparator = cursor_comparator(query.sort_direction, cursor.before);
            let id_comparator = cursor_comparator(SortDirection::Ascending, cursor.before);
            cursor_clause = format!(
                "WHERE ({sort_expression} {comparator} ? OR ({sort_expression} = ? AND id {id_comparator} ?))"
            );
            push_cursor_parameters(
                &mut page_parameters,
                cursor,
                query.sort_field == DuplicateFolderGroupSortField::RepresentativePath,
            )?;
        }
        page_parameters.push(SqlValue::Integer(query.limit + 1));
        let before = query.cursor.as_ref().is_some_and(|cursor| cursor.before);
        let order = effective_order(query.sort_direction, before);
        let id_order = effective_order(SortDirection::Ascending, before);
        let sql = format!(
            "WITH result_groups AS (
                SELECT dfg.id, dfg.run_id, dfg.total_size, dfg.file_count, dfg.folder_count,
                       COALESCE((
                           SELECT dn.path FROM duplicate_folder_group_member representative_member
                           JOIN directory_node dn ON dn.id = representative_member.directory_id
                           WHERE representative_member.group_id = dfg.id AND dn.run_id = dfg.run_id
                           ORDER BY dn.path COLLATE NOCASE, dn.id LIMIT 1
                       ), '') AS representative_path
                FROM duplicate_folder_group dfg WHERE {where_sql}
             )
             SELECT id, run_id, total_size, file_count, folder_count, representative_path
             FROM result_groups {cursor_clause}
             ORDER BY {sort_expression} {order}, id {id_order} LIMIT ?"
        );
        let mut statement = self.connection().prepare(&sql)?;
        let mapped = statement.query_map(params_from_iter(page_parameters.iter()), |row| {
            Ok(DuplicateFolderGroupResult {
                id: row.get(0)?,
                run_id: row.get(1)?,
                total_size: row.get(2)?,
                file_count: row.get(3)?,
                folder_count: row.get(4)?,
                representative_path: row.get(5)?,
            })
        })?;
        let mut groups = mapped.collect::<Result<Vec<_>>>()?;
        let has_more = groups.len() > query.limit as usize;
        if has_more {
            groups.pop();
        }
        if before {
            groups.reverse();
        }
        Ok(DuplicateFolderGroupPage {
            groups,
            total,
            has_more,
        })
    }

    pub fn page_duplicate_folder_members(
        &self,
        query: &DuplicateFolderMemberPageQuery,
    ) -> Result<DuplicateFolderMemberPage> {
        let mut predicates = vec![
            "dfg.run_id = ?".to_owned(),
            "dfg.id = ?".to_owned(),
            "dfg.is_suppressed = 0".to_owned(),
        ];
        let mut base_parameters = vec![
            SqlValue::Integer(query.run_id),
            SqlValue::Integer(query.group_id),
        ];
        if let Some(search) = query
            .filter
            .search
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            predicates.push("dn.path LIKE ? ESCAPE '\\' COLLATE NOCASE".to_owned());
            base_parameters.push(SqlValue::Text(like_pattern(search)));
        }
        let where_sql = predicates.join(" AND ");
        let from_sql = "FROM duplicate_folder_group dfg
                        JOIN duplicate_folder_group_member dfgm ON dfgm.group_id = dfg.id
                        JOIN directory_node dn ON dn.id = dfgm.directory_id AND dn.run_id = dfg.run_id";
        let total: i64 = self.connection().query_row(
            &format!("SELECT COUNT(*) {from_sql} WHERE {where_sql}"),
            params_from_iter(base_parameters.iter()),
            |row| row.get(0),
        )?;
        let sort_expression = match query.sort_field {
            DuplicateFolderMemberSortField::Path => "dn.path COLLATE NOCASE",
        };
        let mut page_parameters = base_parameters;
        let mut cursor_clause = String::new();
        if let Some(cursor) = &query.cursor {
            let comparator = cursor_comparator(query.sort_direction, cursor.before);
            let id_comparator = cursor_comparator(SortDirection::Ascending, cursor.before);
            cursor_clause = format!(
                "AND ({sort_expression} {comparator} ? OR ({sort_expression} = ? AND dfgm.id {id_comparator} ?))"
            );
            push_cursor_parameters(&mut page_parameters, cursor, true)?;
        }
        page_parameters.push(SqlValue::Integer(query.limit + 1));
        let before = query.cursor.as_ref().is_some_and(|cursor| cursor.before);
        let order = effective_order(query.sort_direction, before);
        let id_order = effective_order(SortDirection::Ascending, before);
        let sql = format!(
            "SELECT dfgm.id, dfg.id, dn.path {from_sql}
             WHERE {where_sql} {cursor_clause}
             ORDER BY {sort_expression} {order}, dfgm.id {id_order} LIMIT ?"
        );
        let mut statement = self.connection().prepare(&sql)?;
        let mapped = statement.query_map(params_from_iter(page_parameters.iter()), |row| {
            Ok(DuplicateFolderMemberResult {
                id: row.get(0)?,
                group_id: row.get(1)?,
                path: row.get(2)?,
            })
        })?;
        let mut members = mapped.collect::<Result<Vec<_>>>()?;
        let has_more = members.len() > query.limit as usize;
        if has_more {
            members.pop();
        }
        if before {
            members.reverse();
        }
        Ok(DuplicateFolderMemberPage {
            members,
            total,
            has_more,
        })
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

    pub fn get_directory_id(&self, run_id: i64, path: &str) -> Result<i64> {
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

    pub fn replace_run_exclusions(
        &self,
        run_id: i64,
        exclusions: &[RunExclusionInsert],
    ) -> Result<()> {
        self.connection().execute_batch("BEGIN IMMEDIATE;")?;
        let result = (|| -> Result<()> {
            self.connection().execute(
                "DELETE FROM run_exclusion WHERE run_id = ?1",
                params![run_id],
            )?;
            for exclusion in exclusions {
                self.connection().execute(
                    "INSERT INTO run_exclusion
                        (run_id, path, reason_code, provider_id, provider_name, occurrence_count)
                     VALUES (?1, ?2, ?3, ?4, ?5, 1)
                     ON CONFLICT(run_id, path, reason_code) DO UPDATE SET
                        occurrence_count = run_exclusion.occurrence_count + 1",
                    params![
                        run_id,
                        exclusion.path,
                        exclusion.reason_code,
                        exclusion.provider_id,
                        exclusion.provider_name
                    ],
                )?;
            }
            self.connection().execute(
                "UPDATE scan_run SET excluded_subtree_count =
                    (SELECT COUNT(*) FROM run_exclusion WHERE run_id = ?1)
                 WHERE id = ?1",
                params![run_id],
            )?;
            self.connection().execute_batch("COMMIT;")
        })();
        if result.is_err() {
            let _ = self.connection().execute_batch("ROLLBACK;");
        }
        result
    }

    pub fn page_run_exclusions(
        &self,
        run_id: i64,
        offset: i64,
        limit: i64,
    ) -> Result<(Vec<RunExclusion>, i64)> {
        let total = self.connection().query_row(
            "SELECT COUNT(*) FROM run_exclusion WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        let mut statement = self.connection().prepare(
            "SELECT id, run_id, path, reason_code, provider_id, provider_name, occurrence_count
             FROM run_exclusion WHERE run_id = ?1
             ORDER BY path COLLATE NOCASE, id LIMIT ?2 OFFSET ?3",
        )?;
        let exclusions = statement
            .query_map(params![run_id, limit, offset], |row| {
                Ok(RunExclusion {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    path: row.get(2)?,
                    reason_code: row.get(3)?,
                    provider_id: row.get(4)?,
                    provider_name: row.get(5)?,
                    occurrence_count: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok((exclusions, total))
    }
}

const RUN_SELECT: &str =
    "SELECT id, session_id, parameters_json, status, phase, created_at, started_at,
            completed_at, files_discovered, bytes_discovered, files_hashed,
            duplicate_file_groups, duplicate_folder_groups, wasted_bytes, warning_count,
            excluded_subtree_count, error_message, engine_version FROM scan_run";

fn map_session(row: &rusqlite::Row<'_>) -> Result<ScanSession> {
    Ok(ScanSession {
        id: row.get(0)?,
        name: row.get(1)?,
        roots_json: row.get(2)?,
        ignore_patterns_json: row.get(3)?,
        cloud_policy: row.get(4)?,
        manual_location_exclusions_json: row.get(5)?,
        registered_cloud_locations_json: row.get(6)?,
        cloud_detection_status: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
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
        excluded_subtree_count: row.get(15)?,
        error_message: row.get(16)?,
        engine_version: row.get(17)?,
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

fn like_pattern(value: &str) -> String {
    format!(
        "%{}%",
        value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}

fn cursor_comparator(direction: SortDirection, before: bool) -> &'static str {
    match (direction, before) {
        (SortDirection::Ascending, false) | (SortDirection::Descending, true) => ">",
        (SortDirection::Descending, false) | (SortDirection::Ascending, true) => "<",
    }
}

fn effective_order(direction: SortDirection, before: bool) -> &'static str {
    match (direction, before) {
        (SortDirection::Ascending, false) | (SortDirection::Descending, true) => "ASC",
        (SortDirection::Descending, false) | (SortDirection::Ascending, true) => "DESC",
    }
}

fn push_cursor_parameters(
    parameters: &mut Vec<SqlValue>,
    cursor: &PageCursor,
    expects_text: bool,
) -> Result<()> {
    let value = match (&cursor.value, expects_text) {
        (PageCursorValue::Text(value), true) => SqlValue::Text(value.clone()),
        (PageCursorValue::Integer(value), false) => SqlValue::Integer(*value),
        _ => return Err(Error::InvalidQuery),
    };
    parameters.push(value.clone());
    parameters.push(value);
    parameters.push(SqlValue::Integer(cursor.id));
    Ok(())
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
