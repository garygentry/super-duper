use std::cmp::Ordering;

use chrono::Utc;
use rusqlite::{params, Connection, Error, Result};
use tracing::{debug, info};

pub const CURRENT_SCHEMA_VERSION: i64 = 5;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let db = Self::open_connection(path)?;
        db.reconcile_interrupted_runs()?;
        Ok(db)
    }

    /// Opens an additional connection after process startup without treating currently active
    /// runs as abandoned. Long-lived worker commands and scan threads must use this entry point.
    pub fn open_connection(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Database { conn };
        db.configure_pragmas()?;
        db.migrate_schema()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Database { conn };
        db.configure_pragmas()?;
        db.migrate_schema()?;
        db.reconcile_interrupted_runs()?;
        Ok(db)
    }

    fn configure_pragmas(&self) -> Result<()> {
        self.conn
            .create_collation("UNICODE_NOCASE", unicode_nocase_compare)?;
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA cache_size = -64000;
             PRAGMA mmap_size = 268435456;
             PRAGMA busy_timeout = 5000;",
        )?;
        Ok(())
    }

    pub fn schema_version(&self) -> Result<i64> {
        self.conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
    }

    fn has_user_tables(&self) -> Result<bool> {
        self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%')",
            [],
            |row| row.get(0),
        )
    }

    fn migrate_schema(&self) -> Result<()> {
        let version = self.schema_version()?;
        match version {
            CURRENT_SCHEMA_VERSION => self.conn.execute_batch(include_str!("schema.sql"))?,
            4 => self.migrate_v4_to_v5()?,
            3 => {
                self.migrate_v3_to_v4()?;
                self.migrate_v4_to_v5()?;
            }
            2 => self.migrate_v2_to_v3()?,
            0 if !self.has_user_tables()? => self.conn.execute_batch(include_str!("schema.sql"))?,
            0 | 1 => {
                return Err(Error::InvalidParameterName(format!(
                    "unsupported legacy schema version {version}; database was not modified"
                )))
            }
            newer if newer > CURRENT_SCHEMA_VERSION => {
                return Err(Error::InvalidParameterName(format!(
                    "database schema version {newer} is newer than supported version {CURRENT_SCHEMA_VERSION}"
                )))
            }
            _ => return Err(Error::InvalidQuery),
        }
        self.reconcile_extension_keys()?;
        debug!("SQLite schema ready at version {}", CURRENT_SCHEMA_VERSION);
        Ok(())
    }

    fn reconcile_extension_keys(&self) -> Result<()> {
        let has_scanned_file = self.conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'scanned_file'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?;
        if !has_scanned_file {
            return Ok(());
        }
        let has_extension_key = self.conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM pragma_table_info('scanned_file') WHERE name = 'extension_key'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?;
        let has_extension_index = self.conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'index' AND name = 'idx_file_run_extension_key'
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?;
        let has_missing_keys = has_extension_key
            && self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM scanned_file WHERE extension_key IS NULL LIMIT 1)",
                [],
                |row| row.get::<_, bool>(0),
            )?;
        if has_extension_key && has_extension_index && !has_missing_keys {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        if !has_extension_key {
            tx.execute_batch("ALTER TABLE scanned_file ADD COLUMN extension_key TEXT;")?;
        }

        loop {
            let rows = {
                let mut statement = tx.prepare(
                    "SELECT id, file_name FROM scanned_file
                     WHERE extension_key IS NULL ORDER BY id LIMIT 500",
                )?;
                let rows = statement
                    .query_map([], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>>>()?;
                rows
            };
            if rows.is_empty() {
                break;
            }
            {
                let mut update = tx.prepare_cached(
                    "UPDATE scanned_file SET extension_key = ?1
                     WHERE id = ?2 AND extension_key IS NULL",
                )?;
                for (id, file_name) in rows {
                    update.execute(params![normalized_file_extension_key(&file_name), id])?;
                }
            }
        }
        tx.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_file_run_extension_key
                 ON scanned_file(run_id, extension_key, id);",
        )?;
        tx.commit()
    }

    fn migrate_v2_to_v3(&self) -> Result<()> {
        info!("Migrating SQLite schema from version 2 to 3");
        let now = Utc::now().to_rfc3339();
        self.conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

        let migration = (|| -> Result<()> {
            self.conn.execute_batch(
                "BEGIN IMMEDIATE;
                 ALTER TABLE scan_session RENAME TO legacy_scan_session;
                 ALTER TABLE scanned_file RENAME TO legacy_scanned_file;
                 ALTER TABLE duplicate_group RENAME TO legacy_duplicate_group;
                 ALTER TABLE duplicate_group_member RENAME TO legacy_duplicate_group_member;
                 ALTER TABLE directory_node RENAME TO legacy_directory_node;
                 ALTER TABLE directory_fingerprint RENAME TO legacy_directory_fingerprint;
                 ALTER TABLE directory_similarity RENAME TO legacy_directory_similarity;
                 ALTER TABLE deletion_plan RENAME TO legacy_deletion_plan;",
            )?;

            self.conn.execute_batch(include_str!("schema.sql"))?;

            self.conn.execute(
                "INSERT INTO scan_session
                    (id, name, roots_json, ignore_patterns_json, created_at, updated_at)
                 SELECT id, 'Imported scan ' || id, root_paths, '[]', started_at,
                        COALESCE(completed_at, started_at)
                 FROM legacy_scan_session ORDER BY id",
                [],
            )?;

            self.conn.execute(
                "INSERT INTO scan_run
                    (id, session_id, parameters_json, status, phase, created_at, started_at,
                     completed_at, files_discovered, bytes_discovered, files_hashed,
                     duplicate_file_groups, duplicate_folder_groups, wasted_bytes,
                     warning_count, error_message, engine_version)
                 SELECT ss.id, ss.id,
                        '{\"roots\":' || ss.root_paths ||
                        ',\"ignore_patterns\":[],\"directory_similarity_threshold_millis\":500}',
                        CASE
                            WHEN ss.status = 'completed' THEN 'completed'
                            WHEN ss.status = 'cancelled' THEN 'cancelled'
                            WHEN ss.status = 'failed' THEN 'failed'
                            ELSE 'interrupted'
                        END,
                        'finalizing', ss.started_at, ss.started_at,
                        COALESCE(ss.completed_at, ?1),
                        COALESCE(ss.files_scanned, 0), COALESCE(ss.total_bytes, 0),
                        (SELECT COUNT(DISTINCT dgm.file_id)
                         FROM legacy_duplicate_group dg
                         JOIN legacy_duplicate_group_member dgm ON dgm.group_id = dg.id
                         WHERE dg.session_id = ss.id),
                        (SELECT COUNT(*) FROM legacy_duplicate_group dg WHERE dg.session_id = ss.id),
                        0,
                        (SELECT COALESCE(SUM(dg.wasted_bytes), 0)
                         FROM legacy_duplicate_group dg WHERE dg.session_id = ss.id),
                        0,
                        CASE WHEN ss.status IN ('completed', 'cancelled', 'failed') THEN NULL
                             ELSE 'Interrupted while upgrading legacy scan state' END,
                        'legacy-v2'
                 FROM legacy_scan_session ss ORDER BY ss.id",
                params![now],
            )?;

            // Reconstruct one immutable file snapshot per run/group membership. The v2 path row
            // was mutable, so this is the most history that can be recovered without invention.
            self.conn.execute(
                "INSERT OR IGNORE INTO scanned_file
                    (run_id, root_path, canonical_path, relative_path, file_name, parent_dir,
                     drive_letter, file_size, last_modified, partial_hash, content_hash,
                     file_identity, warning_message, marked_deleted)
                 SELECT DISTINCT dg.session_id, '', sf.canonical_path, '', sf.file_name,
                        sf.parent_dir, COALESCE(sf.drive_letter, ''), sf.file_size,
                        sf.last_modified, sf.partial_hash, sf.content_hash, NULL,
                        'Migrated from mutable v2 path index', sf.marked_deleted
                 FROM legacy_duplicate_group_member dgm
                 JOIN legacy_duplicate_group dg ON dg.id = dgm.group_id
                 JOIN legacy_scanned_file sf ON sf.id = dgm.file_id",
                [],
            )?;
            self.conn.execute(
                "INSERT OR IGNORE INTO scanned_file
                    (run_id, root_path, canonical_path, relative_path, file_name, parent_dir,
                     drive_letter, file_size, last_modified, partial_hash, content_hash,
                     file_identity, warning_message, marked_deleted)
                 SELECT sf.last_seen_session_id, '', sf.canonical_path, '', sf.file_name,
                        sf.parent_dir, COALESCE(sf.drive_letter, ''), sf.file_size,
                        sf.last_modified, sf.partial_hash, sf.content_hash, NULL,
                        'Migrated from mutable v2 path index', sf.marked_deleted
                 FROM legacy_scanned_file sf
                 WHERE sf.last_seen_session_id IS NOT NULL
                   AND EXISTS(SELECT 1 FROM scan_run r WHERE r.id = sf.last_seen_session_id)",
                [],
            )?;

            self.conn.execute(
                "INSERT INTO duplicate_group
                    (id, run_id, content_hash, file_size, file_count, wasted_bytes)
                 SELECT id, session_id, content_hash, file_size, file_count, wasted_bytes
                 FROM legacy_duplicate_group",
                [],
            )?;
            self.conn.execute(
                "INSERT INTO duplicate_group_member (group_id, file_id)
                 SELECT dgm.group_id, sf3.id
                 FROM legacy_duplicate_group_member dgm
                 JOIN legacy_duplicate_group dg ON dg.id = dgm.group_id
                 JOIN legacy_scanned_file sf2 ON sf2.id = dgm.file_id
                 JOIN scanned_file sf3 ON sf3.run_id = dg.session_id
                                      AND sf3.canonical_path = sf2.canonical_path",
                [],
            )?;

            // v2 directory data represented only the most recently analyzed global index.
            self.conn.execute(
                "INSERT INTO directory_node
                    (id, run_id, path, name, parent_id, total_size, file_count, depth)
                 SELECT id, (SELECT id FROM scan_run ORDER BY id DESC LIMIT 1), path, name,
                        parent_id, total_size, file_count, depth
                 FROM legacy_directory_node
                 WHERE EXISTS(SELECT 1 FROM scan_run)",
                [],
            )?;
            self.conn.execute(
                "INSERT INTO directory_fingerprint
                    (id, directory_id, content_fingerprint, file_hash_set)
                 SELECT id, directory_id, content_fingerprint, file_hash_set
                 FROM legacy_directory_fingerprint ldf
                 WHERE EXISTS(SELECT 1 FROM directory_node dn WHERE dn.id = ldf.directory_id)",
                [],
            )?;
            self.conn.execute(
                "INSERT INTO directory_similarity
                    (id, run_id, dir_a_id, dir_b_id, similarity_score, shared_bytes, match_type)
                 SELECT id, (SELECT id FROM scan_run ORDER BY id DESC LIMIT 1), dir_a_id,
                        dir_b_id, similarity_score, shared_bytes, match_type
                 FROM legacy_directory_similarity lds
                 WHERE EXISTS(SELECT 1 FROM scan_run)
                   AND EXISTS(SELECT 1 FROM directory_node dn WHERE dn.id = lds.dir_a_id)
                   AND EXISTS(SELECT 1 FROM directory_node dn WHERE dn.id = lds.dir_b_id)",
                [],
            )?;

            self.conn.execute(
                "INSERT OR IGNORE INTO deletion_plan
                    (id, file_id, marked_at, strategy, executed_at, execution_result)
                 SELECT dp.id, sf3.id, dp.marked_at, dp.strategy, dp.executed_at,
                        dp.execution_result
                 FROM legacy_deletion_plan dp
                 JOIN legacy_scanned_file sf2 ON sf2.id = dp.file_id
                 JOIN scanned_file sf3 ON sf3.canonical_path = sf2.canonical_path
                 WHERE sf3.id = (SELECT MAX(sf4.id) FROM scanned_file sf4
                                 WHERE sf4.canonical_path = sf2.canonical_path)",
                [],
            )?;

            self.conn.execute_batch(
                "DROP TABLE legacy_deletion_plan;
                 DROP TABLE legacy_directory_similarity;
                 DROP TABLE legacy_directory_fingerprint;
                 DROP TABLE legacy_directory_node;
                 DROP TABLE legacy_duplicate_group_member;
                 DROP TABLE legacy_duplicate_group;
                 DROP TABLE legacy_scanned_file;
                 DROP TABLE legacy_scan_session;",
            )?;
            // Reapply indexes after the legacy indexes with the same names have been dropped.
            self.conn.execute_batch(include_str!("schema.sql"))?;
            let violations: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM pragma_foreign_key_check",
                [],
                |row| row.get(0),
            )?;
            if violations != 0 {
                return Err(Error::InvalidQuery);
            }
            self.conn.execute_batch("COMMIT;")?;
            Ok(())
        })();

        if migration.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK;");
        }
        self.conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        migration?;

        Ok(())
    }

    fn migrate_v3_to_v4(&self) -> Result<()> {
        info!("Migrating SQLite schema from version 3 to 4");
        let migration = self.conn.execute_batch(
            "BEGIN IMMEDIATE;
             ALTER TABLE scan_session ADD COLUMN cloud_policy TEXT NOT NULL
                 DEFAULT 'exclude_registered_roots'
                 CHECK(cloud_policy IN ('exclude_registered_roots', 'include_sync_roots_skip_placeholders', 'allow_cloud_access'));
             ALTER TABLE scan_session ADD COLUMN manual_location_exclusions_json TEXT NOT NULL DEFAULT '[]';
             ALTER TABLE scan_session ADD COLUMN registered_cloud_locations_json TEXT NOT NULL DEFAULT '[]';
             ALTER TABLE scan_session ADD COLUMN cloud_detection_status TEXT NOT NULL DEFAULT 'unavailable'
                 CHECK(cloud_detection_status IN ('complete', 'unsupported', 'unavailable'));
             ALTER TABLE scan_run ADD COLUMN excluded_subtree_count INTEGER NOT NULL DEFAULT 0
                 CHECK(excluded_subtree_count >= 0);
             CREATE TABLE run_exclusion (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
                 path TEXT NOT NULL,
                 reason_code TEXT NOT NULL,
                 provider_id TEXT,
                 provider_name TEXT,
                 occurrence_count INTEGER NOT NULL DEFAULT 1 CHECK(occurrence_count > 0),
                 UNIQUE(run_id, path, reason_code)
             );
             CREATE INDEX idx_run_exclusion_run_path
                 ON run_exclusion(run_id, path COLLATE NOCASE, id);
             PRAGMA user_version = 4;
             COMMIT;",
        );
        if migration.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK;");
        }
        migration
    }

    fn migrate_v4_to_v5(&self) -> Result<()> {
        info!("Migrating SQLite schema from version 4 to 5");
        let migration = self.conn.execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE review_plan (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
                 state TEXT NOT NULL DEFAULT 'active' CHECK(state IN ('active', 'archived')),
                 revision INTEGER NOT NULL DEFAULT 0 CHECK(revision >= 0),
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );
             CREATE TABLE review_decision (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
                 group_id INTEGER NOT NULL REFERENCES duplicate_group(id) ON DELETE CASCADE,
                 file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
                 decision TEXT NOT NULL CHECK(decision IN ('keep', 'remove', 'undecided')),
                 provenance TEXT NOT NULL CHECK(provenance = 'manual'),
                 decided_at TEXT NOT NULL,
                 snapshot_canonical_path TEXT NOT NULL,
                 snapshot_file_identity TEXT,
                 snapshot_file_size INTEGER NOT NULL CHECK(snapshot_file_size >= 0),
                 snapshot_last_modified INTEGER NOT NULL,
                 snapshot_content_hash INTEGER,
                 UNIQUE(plan_id, file_id)
             );
             CREATE TABLE review_command (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
                 operation_id TEXT NOT NULL,
                 run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
                 group_id INTEGER NOT NULL REFERENCES duplicate_group(id) ON DELETE CASCADE,
                 file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
                 decision TEXT NOT NULL CHECK(decision IN ('keep', 'remove', 'undecided')),
                 expected_revision INTEGER NOT NULL CHECK(expected_revision >= 0),
                 applied_revision INTEGER NOT NULL CHECK(applied_revision > 0),
                 created_at TEXT NOT NULL,
                 UNIQUE(plan_id, operation_id)
             );
             CREATE UNIQUE INDEX idx_review_plan_one_active_run
                 ON review_plan(run_id) WHERE state = 'active';
             CREATE INDEX idx_review_decision_plan_group
                 ON review_decision(plan_id, group_id, file_id);
             CREATE INDEX idx_review_decision_plan_decision
                 ON review_decision(plan_id, decision, group_id);
             CREATE INDEX idx_review_command_plan_operation
                 ON review_command(plan_id, operation_id);
             PRAGMA user_version = 5;
             COMMIT;",
        );
        if migration.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK;");
        }
        migration
    }

    pub fn reconcile_interrupted_runs(&self) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        let count = self.conn.execute(
            "UPDATE scan_run
             SET status = 'interrupted', phase = 'finalizing', completed_at = ?1,
                 error_message = COALESCE(error_message, 'Run interrupted before a terminal state was persisted')
             WHERE status IN ('running', 'cancelling')",
            params![now],
        )?;
        if count > 0 {
            info!("Reconciled {} abandoned scan run(s) to interrupted", count);
        }
        Ok(count)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn truncate_all(&self) -> Result<()> {
        self.conn.execute_batch(
            "BEGIN;
             DELETE FROM review_command;
             DELETE FROM review_decision;
             DELETE FROM review_plan;
             DELETE FROM deletion_plan;
             DELETE FROM run_exclusion;
             DELETE FROM directory_similarity;
             DELETE FROM duplicate_folder_group_member;
             DELETE FROM duplicate_folder_group;
             DELETE FROM directory_fingerprint;
             DELETE FROM directory_node;
             DELETE FROM duplicate_group_member;
             DELETE FROM duplicate_group;
             DELETE FROM scanned_file;
             DELETE FROM scan_run;
             DELETE FROM scan_session;
             COMMIT;",
        )?;
        Ok(())
    }

    pub fn delete_all_sessions(&self) -> Result<()> {
        self.conn.execute("DELETE FROM scan_session", [])?;
        Ok(())
    }
}

fn unicode_nocase_compare(left: &str, right: &str) -> Ordering {
    left.to_lowercase().cmp(&right.to_lowercase())
}

pub(super) fn normalized_file_extension_key(file_name: &str) -> String {
    let Some(dot_index) = file_name.rfind('.') else {
        return String::new();
    };
    if dot_index == 0 || dot_index + 1 == file_name.len() {
        return String::new();
    }
    file_name[dot_index + 1..].to_lowercase()
}
