use rusqlite::{Connection, Error, Result};

pub const CURRENT_STATUS_SCHEMA_VERSION: i64 = 1;

/// Worker-owned connection to the separate local scan status database.
///
/// This database never owns immutable scan results, review state, preflight state, or operation
/// evidence. Higher-level worker code must keep it on the worker side of the process boundary.
pub struct StatusDatabase {
    conn: Connection,
}

impl StatusDatabase {
    pub fn open(path: &str) -> Result<Self> {
        let database = Self {
            conn: Connection::open(path)?,
        };
        database.configure_pragmas()?;
        database.migrate_schema()?;
        Ok(database)
    }

    pub fn open_in_memory() -> Result<Self> {
        let database = Self {
            conn: Connection::open_in_memory()?,
        };
        database.configure_pragmas()?;
        database.migrate_schema()?;
        Ok(database)
    }

    fn configure_pragmas(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
    }

    pub fn schema_version(&self) -> Result<i64> {
        self.conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    fn has_user_tables(&self) -> Result<bool> {
        self.conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             )",
            [],
            |row| row.get(0),
        )
    }

    fn migrate_schema(&self) -> Result<()> {
        let version = self.schema_version()?;
        match version {
            CURRENT_STATUS_SCHEMA_VERSION => {
                self.conn.execute_batch(include_str!("status_schema.sql"))?;
            }
            0 if !self.has_user_tables()? => {
                self.conn.execute_batch(include_str!("status_schema.sql"))?;
            }
            0 => {
                return Err(Error::InvalidParameterName(
                    "unversioned non-empty status database was not modified".to_owned(),
                ));
            }
            newer if newer > CURRENT_STATUS_SCHEMA_VERSION => {
                return Err(Error::InvalidParameterName(format!(
                    "status database schema version {newer} is newer than supported version {CURRENT_STATUS_SCHEMA_VERSION}"
                )));
            }
            older => {
                return Err(Error::InvalidParameterName(format!(
                    "unsupported status database schema version {older}; database was not modified"
                )));
            }
        }
        Ok(())
    }
}
