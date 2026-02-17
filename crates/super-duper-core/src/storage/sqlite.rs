use rusqlite::{Connection, Result};
use tracing::debug;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Database { conn };
        db.configure_pragmas()?;
        db.create_tables()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Database { conn };
        db.configure_pragmas()?;
        db.create_tables()?;
        Ok(db)
    }

    fn configure_pragmas(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA cache_size = -64000;
             PRAGMA mmap_size = 268435456;
             PRAGMA busy_timeout = 5000;",
        )?;
        debug!("SQLite pragmas configured (WAL mode, 64MB cache, 256MB mmap)");
        Ok(())
    }

    fn create_tables(&self) -> Result<()> {
        self.conn
            .execute_batch(include_str!("schema.sql"))?;
        debug!("SQLite schema initialized");
        Ok(())
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn truncate_all(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM deletion_plan;
             DELETE FROM directory_similarity;
             DELETE FROM directory_fingerprint;
             DELETE FROM directory_node;
             DELETE FROM duplicate_group_member;
             DELETE FROM duplicate_group;
             DELETE FROM scanned_file;
             DELETE FROM scan_session;",
        )?;
        debug!("All tables truncated");
        Ok(())
    }
}
