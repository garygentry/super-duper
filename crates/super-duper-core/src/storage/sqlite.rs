use std::cmp::Ordering;

use chrono::Utc;
use rusqlite::{params, Connection, Error, Result};
use tracing::{debug, info};

pub const CURRENT_SCHEMA_VERSION: i64 = 12;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let db = Self::open_connection(path)?;
        db.reconcile_interrupted_runs()?;
        db.reconcile_interrupted_preflights()?;
        db.reconcile_interrupted_recycle_operations()?;
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
        db.reconcile_interrupted_preflights()?;
        db.reconcile_interrupted_recycle_operations()?;
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
            11 => self.migrate_v11_to_v12()?,
            10 => {
                self.migrate_v10_to_v11()?;
                self.migrate_v11_to_v12()?;
            }
            9 => {
                self.migrate_v9_to_v10()?;
                self.migrate_v10_to_v11()?;
                self.migrate_v11_to_v12()?;
            }
            8 => {
                self.migrate_v8_to_v9()?;
                self.migrate_v9_to_v10()?;
                self.migrate_v10_to_v11()?;
                self.migrate_v11_to_v12()?;
            }
            7 => {
                self.migrate_v7_to_v8()?;
                self.migrate_v8_to_v9()?;
                self.migrate_v9_to_v10()?;
                self.migrate_v10_to_v11()?;
                self.migrate_v11_to_v12()?;
            }
            6 => {
                self.migrate_v6_to_v7()?;
                self.migrate_v7_to_v8()?;
                self.migrate_v8_to_v9()?;
                self.migrate_v9_to_v10()?;
                self.migrate_v10_to_v11()?;
                self.migrate_v11_to_v12()?;
            }
            5 => {
                self.migrate_v5_to_v6()?;
                self.migrate_v6_to_v7()?;
                self.migrate_v7_to_v8()?;
                self.migrate_v8_to_v9()?;
                self.migrate_v9_to_v10()?;
                self.migrate_v10_to_v11()?;
                self.migrate_v11_to_v12()?;
            }
            4 => {
                self.migrate_v4_to_v5()?;
                self.migrate_v5_to_v6()?;
                self.migrate_v6_to_v7()?;
                self.migrate_v7_to_v8()?;
                self.migrate_v8_to_v9()?;
                self.migrate_v9_to_v10()?;
                self.migrate_v10_to_v11()?;
                self.migrate_v11_to_v12()?;
            }
            3 => {
                self.migrate_v3_to_v4()?;
                self.migrate_v4_to_v5()?;
                self.migrate_v5_to_v6()?;
                self.migrate_v6_to_v7()?;
                self.migrate_v7_to_v8()?;
                self.migrate_v8_to_v9()?;
                self.migrate_v9_to_v10()?;
                self.migrate_v10_to_v11()?;
                self.migrate_v11_to_v12()?;
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
        // Reconcile idempotent tables and indexes after every supported forward migration. This
        // also keeps narrowly constructed historical fixtures aligned with the complete schema.
        self.conn.execute_batch(include_str!("schema.sql"))?;
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

    fn migrate_v5_to_v6(&self) -> Result<()> {
        info!("Migrating SQLite schema from version 5 to 6");
        let migration = self.conn.execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE review_folder_decision (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
                 folder_group_id INTEGER NOT NULL REFERENCES duplicate_folder_group(id) ON DELETE CASCADE,
                 folder_member_id INTEGER NOT NULL REFERENCES duplicate_folder_group_member(id) ON DELETE CASCADE,
                 directory_id INTEGER NOT NULL REFERENCES directory_node(id) ON DELETE CASCADE,
                 decision TEXT NOT NULL CHECK(decision IN ('keep', 'remove', 'undecided')),
                 provenance TEXT NOT NULL CHECK(provenance = 'manual'),
                 decided_at TEXT NOT NULL,
                 snapshot_path TEXT NOT NULL,
                 snapshot_total_size INTEGER NOT NULL CHECK(snapshot_total_size >= 0),
                 snapshot_file_count INTEGER NOT NULL CHECK(snapshot_file_count > 0),
                 snapshot_structural_fingerprint TEXT NOT NULL,
                 snapshot_verified_fingerprint TEXT NOT NULL,
                 UNIQUE(plan_id, folder_member_id)
             );
             CREATE TABLE review_folder_command (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
                 operation_id TEXT NOT NULL,
                 run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
                 folder_group_id INTEGER NOT NULL REFERENCES duplicate_folder_group(id) ON DELETE CASCADE,
                 folder_member_id INTEGER NOT NULL REFERENCES duplicate_folder_group_member(id) ON DELETE CASCADE,
                 decision TEXT NOT NULL CHECK(decision IN ('keep', 'remove', 'undecided')),
                 expected_revision INTEGER NOT NULL CHECK(expected_revision >= 0),
                 applied_revision INTEGER NOT NULL CHECK(applied_revision > 0),
                 created_at TEXT NOT NULL,
                 UNIQUE(plan_id, operation_id)
             );
             CREATE INDEX idx_review_folder_decision_plan_group
                 ON review_folder_decision(plan_id, folder_group_id, folder_member_id);
             CREATE INDEX idx_review_folder_decision_plan_decision
                 ON review_folder_decision(plan_id, decision, directory_id);
             CREATE INDEX idx_review_folder_command_plan_operation
                 ON review_folder_command(plan_id, operation_id);
             PRAGMA user_version = 6;
             COMMIT;",
        );
        if migration.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK;");
        }
        migration
    }

    fn migrate_v6_to_v7(&self) -> Result<()> {
        info!("Migrating SQLite schema from version 6 to 7");
        let migration = self.conn.execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE preference_rule (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL COLLATE UNICODE_NOCASE UNIQUE,
                 kind TEXT NOT NULL CHECK(kind = 'ordered_preferred_scan_roots'),
                 state TEXT NOT NULL DEFAULT 'active' CHECK(state IN ('active', 'archived')),
                 revision INTEGER NOT NULL CHECK(revision > 0),
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );
             CREATE TABLE preference_rule_root (
                 rule_id INTEGER NOT NULL REFERENCES preference_rule(id) ON DELETE CASCADE,
                 ordinal INTEGER NOT NULL CHECK(ordinal >= 0 AND ordinal < 64),
                 root_path TEXT NOT NULL CHECK(root_path <> ''),
                 PRIMARY KEY(rule_id, ordinal),
                 UNIQUE(rule_id, root_path COLLATE UNICODE_NOCASE)
             );
             CREATE TABLE preference_rule_command (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 operation_id TEXT NOT NULL UNIQUE,
                 requested_rule_id INTEGER,
                 name TEXT NOT NULL,
                 roots_json TEXT NOT NULL,
                 expected_revision INTEGER NOT NULL CHECK(expected_revision >= 0),
                 applied_rule_id INTEGER NOT NULL REFERENCES preference_rule(id) ON DELETE CASCADE,
                 applied_revision INTEGER NOT NULL CHECK(applied_revision > 0),
                 created_at TEXT NOT NULL
             );
             CREATE INDEX idx_preference_rule_state_name
                 ON preference_rule(state, name COLLATE UNICODE_NOCASE, id);
             CREATE INDEX idx_preference_rule_root_path
                 ON preference_rule_root(rule_id, root_path COLLATE UNICODE_NOCASE, ordinal);
             PRAGMA user_version = 7;
             COMMIT;",
        );
        if migration.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK;");
        }
        migration
    }

    fn migrate_v7_to_v8(&self) -> Result<()> {
        info!("Migrating SQLite schema from version 7 to 8");
        let migration = self.conn.execute_batch(
            "BEGIN IMMEDIATE;
             ALTER TABLE review_decision ADD COLUMN manual_revision INTEGER NOT NULL DEFAULT 0
                 CHECK(manual_revision >= 0);
             CREATE TABLE review_rule_application (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
                 operation_id TEXT NOT NULL UNIQUE,
                 run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
                 rule_id INTEGER NOT NULL REFERENCES preference_rule(id) ON DELETE RESTRICT,
                 rule_revision INTEGER NOT NULL CHECK(rule_revision > 0),
                 rule_name TEXT NOT NULL,
                 rule_kind TEXT NOT NULL CHECK(rule_kind = 'ordered_preferred_scan_roots'),
                 rule_roots_json TEXT NOT NULL,
                 scope_kind TEXT NOT NULL CHECK(scope_kind IN ('selected_sets', 'current_filter', 'completed_run')),
                 scope_json TEXT NOT NULL,
                 scope_signature TEXT NOT NULL,
                 preview_signature TEXT NOT NULL,
                 source_review_revision INTEGER NOT NULL CHECK(source_review_revision >= 0),
                 applied_revision INTEGER NOT NULL CHECK(applied_revision > 0),
                 scoped_group_count INTEGER NOT NULL CHECK(scoped_group_count >= 0),
                 applicable_group_count INTEGER NOT NULL CHECK(applicable_group_count >= 0),
                 blocked_group_count INTEGER NOT NULL CHECK(blocked_group_count >= 0),
                 rule_keep_path_count INTEGER NOT NULL CHECK(rule_keep_path_count >= 0),
                 rule_remove_path_count INTEGER NOT NULL CHECK(rule_remove_path_count >= 0),
                 rule_remove_physical_item_count INTEGER NOT NULL CHECK(rule_remove_physical_item_count >= 0),
                 rule_remove_bytes INTEGER NOT NULL CHECK(rule_remove_bytes >= 0),
                 state TEXT NOT NULL DEFAULT 'active' CHECK(state IN ('active', 'reversed')),
                 created_at TEXT NOT NULL,
                 reversed_at TEXT
             );
             CREATE TABLE review_rule_decision (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 application_id INTEGER NOT NULL REFERENCES review_rule_application(id) ON DELETE CASCADE,
                 plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
                 group_id INTEGER NOT NULL REFERENCES duplicate_group(id) ON DELETE CASCADE,
                 file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
                 decision TEXT NOT NULL CHECK(decision IN ('keep', 'remove')),
                 explanation_code TEXT NOT NULL,
                 preferred_rank INTEGER CHECK(preferred_rank IS NULL OR preferred_rank >= 0),
                 decided_at TEXT NOT NULL,
                 snapshot_canonical_path TEXT NOT NULL,
                 snapshot_file_identity TEXT,
                 snapshot_file_size INTEGER NOT NULL CHECK(snapshot_file_size >= 0),
                 snapshot_last_modified INTEGER NOT NULL,
                 snapshot_content_hash INTEGER,
                 UNIQUE(plan_id, file_id)
             );
             CREATE TABLE review_rule_reversal_command (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
                 operation_id TEXT NOT NULL UNIQUE,
                 run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
                 application_id INTEGER NOT NULL REFERENCES review_rule_application(id) ON DELETE CASCADE,
                 expected_revision INTEGER NOT NULL CHECK(expected_revision >= 0),
                 applied_revision INTEGER NOT NULL CHECK(applied_revision > 0),
                 removed_keep_count INTEGER NOT NULL CHECK(removed_keep_count >= 0),
                 removed_remove_count INTEGER NOT NULL CHECK(removed_remove_count >= 0),
                 created_at TEXT NOT NULL
             );
             CREATE INDEX idx_review_rule_application_plan_state
                 ON review_rule_application(plan_id, state, id DESC);
             CREATE INDEX idx_review_rule_application_run_rule
                 ON review_rule_application(run_id, rule_id, id DESC);
             CREATE INDEX idx_review_rule_decision_application
                 ON review_rule_decision(application_id, group_id, file_id);
             CREATE INDEX idx_review_rule_decision_plan_group
                 ON review_rule_decision(plan_id, group_id, decision, file_id);
             CREATE INDEX idx_review_rule_reversal_plan_operation
                 ON review_rule_reversal_command(plan_id, operation_id);
             PRAGMA user_version = 8;
             COMMIT;",
        );
        if migration.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK;");
        }
        migration
    }

    fn migrate_v8_to_v9(&self) -> Result<()> {
        info!("Migrating SQLite schema from version 8 to 9");
        let migration = self.conn.execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE preflight (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 operation_id TEXT NOT NULL UNIQUE,
                 run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
                 plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
                 review_revision INTEGER NOT NULL CHECK(review_revision >= 0),
                 snapshot_signature TEXT NOT NULL,
                 status TEXT NOT NULL DEFAULT 'pending'
                     CHECK(status IN ('pending', 'running', 'cancelling', 'completed', 'cancelled', 'interrupted', 'failed')),
                 logical_removal_count INTEGER NOT NULL CHECK(logical_removal_count >= 0),
                 physical_removal_count INTEGER NOT NULL CHECK(physical_removal_count >= 0),
                 folder_removal_count INTEGER NOT NULL CHECK(folder_removal_count >= 0),
                 affected_group_count INTEGER NOT NULL CHECK(affected_group_count >= 0),
                 planned_removal_bytes INTEGER NOT NULL CHECK(planned_removal_bytes >= 0),
                 total_item_count INTEGER NOT NULL CHECK(total_item_count >= 0),
                 processed_item_count INTEGER NOT NULL DEFAULT 0 CHECK(processed_item_count >= 0),
                 ready_count INTEGER NOT NULL DEFAULT 0 CHECK(ready_count >= 0),
                 changed_count INTEGER NOT NULL DEFAULT 0 CHECK(changed_count >= 0),
                 missing_count INTEGER NOT NULL DEFAULT 0 CHECK(missing_count >= 0),
                 unavailable_count INTEGER NOT NULL DEFAULT 0 CHECK(unavailable_count >= 0),
                 conflict_count INTEGER NOT NULL DEFAULT 0 CHECK(conflict_count >= 0),
                 created_at TEXT NOT NULL,
                 started_at TEXT,
                 completed_at TEXT,
                 error_code TEXT,
                 error_detail TEXT
             );
             CREATE TABLE preflight_item (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 preflight_id INTEGER NOT NULL REFERENCES preflight(id) ON DELETE CASCADE,
                 ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
                 target_kind TEXT NOT NULL CHECK(target_kind IN ('file', 'folder')),
                 target_role TEXT NOT NULL CHECK(target_role IN ('remove', 'survivor')),
                 physical_key TEXT NOT NULL,
                 group_id INTEGER REFERENCES duplicate_group(id) ON DELETE CASCADE,
                 folder_group_id INTEGER REFERENCES duplicate_folder_group(id) ON DELETE CASCADE,
                 folder_member_id INTEGER REFERENCES duplicate_folder_group_member(id) ON DELETE CASCADE,
                 snapshot_file_id INTEGER REFERENCES scanned_file(id) ON DELETE CASCADE,
                 snapshot_directory_id INTEGER REFERENCES directory_node(id) ON DELETE CASCADE,
                 snapshot_path TEXT NOT NULL,
                 snapshot_file_identity TEXT,
                 snapshot_file_size INTEGER CHECK(snapshot_file_size IS NULL OR snapshot_file_size >= 0),
                 snapshot_last_modified INTEGER,
                 snapshot_content_hash INTEGER,
                 snapshot_structural_fingerprint TEXT,
                 snapshot_verified_fingerprint TEXT,
                 outcome TEXT NOT NULL DEFAULT 'pending'
                     CHECK(outcome IN ('pending', 'ready', 'changed', 'missing', 'unavailable', 'conflict')),
                 reason_code TEXT,
                 observed_file_identity TEXT,
                 observed_file_size INTEGER,
                 observed_last_modified INTEGER,
                 observed_content_hash INTEGER,
                 os_error INTEGER,
                 observed_at TEXT,
                 UNIQUE(preflight_id, ordinal)
             );
             CREATE TABLE preflight_item_source (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 item_id INTEGER NOT NULL REFERENCES preflight_item(id) ON DELETE CASCADE,
                 source_kind TEXT NOT NULL CHECK(source_kind IN ('file_decision', 'folder_decision', 'survivor')),
                 group_id INTEGER REFERENCES duplicate_group(id) ON DELETE CASCADE,
                 folder_group_id INTEGER REFERENCES duplicate_folder_group(id) ON DELETE CASCADE,
                 folder_member_id INTEGER REFERENCES duplicate_folder_group_member(id) ON DELETE CASCADE,
                 file_id INTEGER REFERENCES scanned_file(id) ON DELETE CASCADE,
                 directory_id INTEGER REFERENCES directory_node(id) ON DELETE CASCADE,
                 snapshot_path TEXT NOT NULL
             );
             CREATE INDEX idx_preflight_run_created ON preflight(run_id, id DESC);
             CREATE INDEX idx_preflight_status ON preflight(status, id);
             CREATE INDEX idx_preflight_item_page
                 ON preflight_item(preflight_id, outcome, target_role, target_kind, snapshot_path COLLATE UNICODE_NOCASE, id);
             CREATE INDEX idx_preflight_item_pending
                 ON preflight_item(preflight_id, ordinal) WHERE outcome = 'pending';
             CREATE INDEX idx_preflight_item_source_item ON preflight_item_source(item_id, id);
             PRAGMA user_version = 9;
             COMMIT;",
        );
        if migration.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK;");
        }
        migration
    }

    fn migrate_v9_to_v10(&self) -> Result<()> {
        info!("Migrating SQLite schema from version 9 to 10");
        let migration = self.conn.execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE recycle_operation (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 operation_id TEXT NOT NULL UNIQUE,
                 run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
                 plan_id INTEGER NOT NULL REFERENCES review_plan(id) ON DELETE CASCADE,
                 preflight_id INTEGER NOT NULL REFERENCES preflight(id) ON DELETE CASCADE,
                 review_revision INTEGER NOT NULL CHECK(review_revision >= 0),
                 preflight_snapshot_signature TEXT NOT NULL,
                 intent_signature TEXT NOT NULL,
                 policy_version INTEGER NOT NULL DEFAULT 1 CHECK(policy_version > 0),
                 status TEXT NOT NULL DEFAULT 'prepared'
                     CHECK(status IN ('prepared', 'awaiting_confirmation', 'submitted', 'executing',
                                      'cancelling', 'expired', 'cancelled', 'completed',
                                      'partially_completed', 'failed', 'recovery_required')),
                 logical_removal_count INTEGER NOT NULL CHECK(logical_removal_count >= 0),
                 shell_item_count INTEGER NOT NULL CHECK(shell_item_count >= 0),
                 physical_item_count INTEGER NOT NULL CHECK(physical_item_count >= 0),
                 folder_item_count INTEGER NOT NULL CHECK(folder_item_count >= 0),
                 affected_group_count INTEGER NOT NULL CHECK(affected_group_count >= 0),
                 planned_removal_bytes INTEGER NOT NULL CHECK(planned_removal_bytes >= 0),
                 affected_location_count INTEGER NOT NULL DEFAULT 0 CHECK(affected_location_count >= 0),
                 exclusion_count INTEGER NOT NULL DEFAULT 0 CHECK(exclusion_count >= 0),
                 prepared_at TEXT NOT NULL,
                 confirmation_signature TEXT,
                 confirmation_expires_at TEXT,
                 submitted_at TEXT,
                 completed_at TEXT,
                 cancellation_requested INTEGER NOT NULL DEFAULT 0 CHECK(cancellation_requested IN (0, 1)),
                 error_code TEXT,
                 error_detail TEXT
             );
             CREATE TABLE recycle_operation_batch (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 recycle_operation_id INTEGER NOT NULL REFERENCES recycle_operation(id) ON DELETE CASCADE,
                 ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
                 item_signature TEXT NOT NULL,
                 status TEXT NOT NULL DEFAULT 'pending'
                     CHECK(status IN ('pending', 'admitted', 'shell_started', 'reported', 'skipped', 'ambiguous')),
                 admission_expires_at TEXT,
                 shell_attempt_id TEXT,
                 started_at TEXT,
                 reported_at TEXT,
                 UNIQUE(recycle_operation_id, ordinal)
             );
             CREATE TABLE recycle_operation_item (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 recycle_operation_id INTEGER NOT NULL REFERENCES recycle_operation(id) ON DELETE CASCADE,
                 batch_id INTEGER NOT NULL REFERENCES recycle_operation_batch(id) ON DELETE CASCADE,
                 ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
                 preflight_item_id INTEGER NOT NULL REFERENCES preflight_item(id) ON DELETE CASCADE,
                 preflight_source_id INTEGER REFERENCES preflight_item_source(id) ON DELETE CASCADE,
                 target_kind TEXT NOT NULL CHECK(target_kind IN ('file', 'folder')),
                 physical_key TEXT NOT NULL,
                 snapshot_path TEXT NOT NULL,
                 group_id INTEGER REFERENCES duplicate_group(id) ON DELETE CASCADE,
                 folder_group_id INTEGER REFERENCES duplicate_folder_group(id) ON DELETE CASCADE,
                 folder_member_id INTEGER REFERENCES duplicate_folder_group_member(id) ON DELETE CASCADE,
                 snapshot_file_id INTEGER REFERENCES scanned_file(id) ON DELETE CASCADE,
                 snapshot_directory_id INTEGER REFERENCES directory_node(id) ON DELETE CASCADE,
                 planned_bytes INTEGER NOT NULL DEFAULT 0 CHECK(planned_bytes >= 0),
                 eligibility_status TEXT NOT NULL DEFAULT 'pending'
                     CHECK(eligibility_status IN ('pending', 'eligible', 'non_recyclable')),
                 eligibility_code TEXT,
                 result_status TEXT NOT NULL DEFAULT 'pending'
                     CHECK(result_status IN ('pending', 'recycled', 'failed', 'cancelled', 'unknown')),
                 result_code TEXT,
                 shell_hresult INTEGER,
                 recycled_item_present INTEGER CHECK(recycled_item_present IS NULL OR recycled_item_present IN (0, 1)),
                 result_at TEXT,
                 UNIQUE(recycle_operation_id, ordinal)
             );
             CREATE TABLE recycle_operation_report (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 recycle_operation_id INTEGER NOT NULL REFERENCES recycle_operation(id) ON DELETE CASCADE,
                 batch_id INTEGER REFERENCES recycle_operation_batch(id) ON DELETE CASCADE,
                 report_operation_id TEXT NOT NULL UNIQUE,
                 report_kind TEXT NOT NULL CHECK(report_kind IN ('eligibility', 'confirmation', 'batch_begin', 'result', 'recovery')),
                 payload_signature TEXT NOT NULL,
                 created_at TEXT NOT NULL
             );
             CREATE TABLE recycle_operation_recovery (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 recycle_operation_id INTEGER NOT NULL REFERENCES recycle_operation(id) ON DELETE CASCADE,
                 batch_id INTEGER REFERENCES recycle_operation_batch(id) ON DELETE CASCADE,
                 item_id INTEGER REFERENCES recycle_operation_item(id) ON DELETE CASCADE,
                 reason_code TEXT NOT NULL,
                 detail TEXT,
                 created_at TEXT NOT NULL
             );
             CREATE INDEX idx_recycle_operation_run_created ON recycle_operation(run_id, id DESC);
             CREATE INDEX idx_recycle_operation_status ON recycle_operation(status, id);
             CREATE INDEX idx_recycle_operation_preflight ON recycle_operation(preflight_id, id DESC);
             CREATE INDEX idx_recycle_operation_batch_state
                 ON recycle_operation_batch(recycle_operation_id, status, ordinal);
             CREATE INDEX idx_recycle_operation_item_page
                 ON recycle_operation_item(recycle_operation_id, result_status, eligibility_status,
                                           target_kind, snapshot_path COLLATE UNICODE_NOCASE, id);
             CREATE INDEX idx_recycle_operation_item_batch ON recycle_operation_item(batch_id, ordinal);
             CREATE INDEX idx_recycle_operation_report_operation
                 ON recycle_operation_report(recycle_operation_id, id);
             CREATE INDEX idx_recycle_operation_recovery_operation
                 ON recycle_operation_recovery(recycle_operation_id, id);
             PRAGMA user_version = 10;
             COMMIT;",
        );
        if migration.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK;");
        }
        migration
    }

    fn migrate_v10_to_v11(&self) -> Result<()> {
        info!("Migrating SQLite schema from version 10 to 11");
        let migration = self.conn.execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE recovery_review_observation (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 request_id TEXT NOT NULL UNIQUE,
                 payload_signature TEXT NOT NULL,
                 recycle_operation_id INTEGER NOT NULL
                     REFERENCES recycle_operation(id) ON DELETE CASCADE,
                 item_id INTEGER NOT NULL REFERENCES recycle_operation_item(id) ON DELETE CASCADE,
                 observation TEXT NOT NULL
                     CHECK(observation IN ('observed_in_recycle_bin', 'observed_at_source',
                                           'observed_in_both', 'observed_in_neither',
                                           'deferred_unresolved')),
                 observed_at TEXT NOT NULL,
                 note TEXT,
                 evidence_version INTEGER NOT NULL CHECK(evidence_version = 1),
                 supersedes_observation_id INTEGER UNIQUE
                     REFERENCES recovery_review_observation(id) ON DELETE CASCADE,
                 correction_reason TEXT,
                 created_at TEXT NOT NULL,
                 CHECK((supersedes_observation_id IS NULL AND correction_reason IS NULL)
                       OR (supersedes_observation_id IS NOT NULL AND correction_reason IS NOT NULL))
             );
             CREATE INDEX idx_recovery_review_operation_item
                 ON recovery_review_observation(recycle_operation_id, item_id, id);
             CREATE INDEX idx_recovery_review_supersession
                 ON recovery_review_observation(supersedes_observation_id);
             PRAGMA user_version = 11;
             COMMIT;",
        );
        if migration.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK;");
        }
        migration
    }

    fn migrate_v11_to_v12(&self) -> Result<()> {
        info!("Migrating SQLite schema from version 11 to 12");
        let migration = self.conn.execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE review_live_validation (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 operation_id TEXT NOT NULL UNIQUE,
                 request_signature TEXT NOT NULL,
                 run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
                 group_id INTEGER NOT NULL REFERENCES duplicate_group(id) ON DELETE CASCADE,
                 expected_review_revision INTEGER NOT NULL CHECK(expected_review_revision >= 0),
                 scope TEXT NOT NULL CHECK(scope IN ('selection', 'visible_page')),
                 item_count INTEGER NOT NULL CHECK(item_count BETWEEN 1 AND 200),
                 present_count INTEGER NOT NULL CHECK(present_count >= 0),
                 changed_count INTEGER NOT NULL CHECK(changed_count >= 0),
                 missing_count INTEGER NOT NULL CHECK(missing_count >= 0),
                 unavailable_count INTEGER NOT NULL CHECK(unavailable_count >= 0),
                 invalidated_decision_count INTEGER NOT NULL CHECK(invalidated_decision_count >= 0),
                 created_at TEXT NOT NULL
             );
             CREATE TABLE review_live_validation_item (
                 validation_id INTEGER NOT NULL REFERENCES review_live_validation(id) ON DELETE CASCADE,
                 ordinal INTEGER NOT NULL CHECK(ordinal >= 0 AND ordinal < 200),
                 file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
                 state TEXT NOT NULL CHECK(state IN ('present', 'changed', 'missing', 'unavailable')),
                 reason_code TEXT NOT NULL,
                 observed_file_identity TEXT,
                 observed_file_size INTEGER CHECK(observed_file_size IS NULL OR observed_file_size >= 0),
                 observed_last_modified INTEGER,
                 os_error INTEGER,
                 decision_invalidated INTEGER NOT NULL CHECK(decision_invalidated IN (0, 1)),
                 invalidated_decision TEXT CHECK(invalidated_decision IS NULL OR invalidated_decision IN ('keep', 'remove')),
                 observed_at TEXT NOT NULL,
                 PRIMARY KEY(validation_id, ordinal),
                 UNIQUE(validation_id, file_id)
             );
             CREATE TABLE review_live_file_state (
                 run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
                 file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
                 validation_id INTEGER NOT NULL REFERENCES review_live_validation(id) ON DELETE CASCADE,
                 state TEXT NOT NULL CHECK(state IN ('present', 'changed', 'missing', 'unavailable')),
                 reason_code TEXT NOT NULL,
                 observed_file_identity TEXT,
                 observed_file_size INTEGER CHECK(observed_file_size IS NULL OR observed_file_size >= 0),
                 observed_last_modified INTEGER,
                 os_error INTEGER,
                 decision_invalidated INTEGER NOT NULL CHECK(decision_invalidated IN (0, 1)),
                 invalidated_decision TEXT CHECK(invalidated_decision IS NULL OR invalidated_decision IN ('keep', 'remove')),
                 observed_at TEXT NOT NULL,
                 PRIMARY KEY(run_id, file_id)
             );
             DROP VIEW IF EXISTS effective_review_decision;
             CREATE VIEW recorded_review_decision AS
             SELECT manual.plan_id, manual.group_id, manual.file_id, manual.decision,
                    'manual' AS provenance, manual.decided_at, NULL AS application_id
             FROM review_decision manual
             LEFT JOIN (
                 SELECT rule_decision.plan_id, rule_decision.file_id, application.applied_revision
                 FROM review_rule_decision rule_decision
                 JOIN review_rule_application application
                   ON application.id = rule_decision.application_id AND application.state = 'active'
             ) rule ON rule.plan_id = manual.plan_id AND rule.file_id = manual.file_id
             WHERE manual.decision IN ('keep', 'remove')
                OR rule.file_id IS NULL
                OR manual.manual_revision > rule.applied_revision
             UNION ALL
             SELECT rule_decision.plan_id, rule_decision.group_id, rule_decision.file_id,
                    rule_decision.decision, 'rule' AS provenance, rule_decision.decided_at,
                    rule_decision.application_id
             FROM review_rule_decision rule_decision
             JOIN review_rule_application application
               ON application.id = rule_decision.application_id AND application.state = 'active'
             WHERE NOT EXISTS (
                 SELECT 1 FROM review_decision manual
                 WHERE manual.plan_id = rule_decision.plan_id
                   AND manual.file_id = rule_decision.file_id
                   AND (manual.decision IN ('keep', 'remove')
                        OR manual.manual_revision > application.applied_revision)
             );
             CREATE VIEW effective_review_decision AS
             SELECT recorded.plan_id, recorded.group_id, recorded.file_id, recorded.decision,
                    recorded.provenance, recorded.decided_at, recorded.application_id
             FROM recorded_review_decision recorded
             JOIN review_plan plan ON plan.id = recorded.plan_id
             LEFT JOIN review_live_file_state live
               ON live.run_id = plan.run_id AND live.file_id = recorded.file_id
             WHERE live.decision_invalidated IS NULL OR live.decision_invalidated = 0;
             CREATE INDEX idx_review_live_validation_run
                 ON review_live_validation(run_id, group_id, id DESC);
             CREATE INDEX idx_review_live_validation_item_file
                 ON review_live_validation_item(file_id, validation_id DESC);
             CREATE INDEX idx_review_live_file_state_invalidated
                 ON review_live_file_state(run_id, decision_invalidated, file_id);
             PRAGMA user_version = 12;
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

    pub fn reconcile_interrupted_preflights(&self) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        let count = self.conn.execute(
            "UPDATE preflight
             SET status = 'interrupted', completed_at = ?1,
                 error_code = COALESCE(error_code, 'worker_interrupted'),
                 error_detail = COALESCE(error_detail, 'Worker exited before preflight reached a terminal state')
             WHERE status IN ('running', 'cancelling')",
            params![now],
        )?;
        if count > 0 {
            info!("Reconciled {} abandoned preflight(s) to interrupted", count);
        }
        Ok(count)
    }

    pub fn reconcile_interrupted_recycle_operations(&self) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO recycle_operation_recovery
                (recycle_operation_id, batch_id, item_id, reason_code, detail, created_at)
             SELECT item.recycle_operation_id, item.batch_id, item.id,
                    'worker_interrupted_after_shell_start',
                    'Shell work may have occurred before the worker received a durable item result', ?1
             FROM recycle_operation_item item
             JOIN recycle_operation_batch batch ON batch.id = item.batch_id
             WHERE batch.status = 'shell_started' AND item.result_status = 'pending'
               AND NOT EXISTS (
                   SELECT 1 FROM recycle_operation_recovery recovery
                   WHERE recovery.item_id = item.id
                     AND recovery.reason_code = 'worker_interrupted_after_shell_start'
               )",
            params![now],
        )?;
        tx.execute(
            "UPDATE recycle_operation_item
             SET result_status = 'unknown', result_code = 'worker_interrupted_after_shell_start',
                 result_at = ?1
             WHERE result_status = 'pending' AND batch_id IN
                (SELECT id FROM recycle_operation_batch WHERE status = 'shell_started')",
            params![now],
        )?;
        tx.execute(
            "UPDATE recycle_operation_batch
             SET status = 'ambiguous', reported_at = ?1
             WHERE status = 'shell_started'",
            params![now],
        )?;
        let expired = tx.execute(
            "UPDATE recycle_operation
             SET status = 'expired', completed_at = ?1,
                 error_code = COALESCE(error_code, 'application_restarted'),
                 error_detail = COALESCE(error_detail, 'Preparation expired when the application restarted')
             WHERE status IN ('prepared', 'awaiting_confirmation')",
            params![now],
        )?;
        let ambiguous = tx.execute(
            "UPDATE recycle_operation
             SET status = 'recovery_required', completed_at = ?1,
                 error_code = COALESCE(error_code, 'shell_result_ambiguous'),
                 error_detail = COALESCE(error_detail, 'Shell work may have started before durable results were received')
             WHERE status IN ('submitted', 'executing', 'cancelling')",
            params![now],
        )?;
        tx.commit()?;
        let count = expired + ambiguous;
        if count > 0 {
            info!("Reconciled {} abandoned recycle operation(s)", count);
        }
        Ok(count)
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn truncate_all(&self) -> Result<()> {
        self.conn.execute_batch(
            "BEGIN;
             DELETE FROM recycle_operation_recovery;
             DELETE FROM recycle_operation_report;
             DELETE FROM recycle_operation_item;
             DELETE FROM recycle_operation_batch;
             DELETE FROM recycle_operation;
             DELETE FROM preflight_item_source;
             DELETE FROM preflight_item;
             DELETE FROM preflight;
             DELETE FROM preference_rule_command;
             DELETE FROM review_rule_reversal_command;
             DELETE FROM review_rule_decision;
             DELETE FROM review_rule_application;
             DELETE FROM preference_rule_root;
             DELETE FROM preference_rule;
             DELETE FROM review_folder_command;
             DELETE FROM review_folder_decision;
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
