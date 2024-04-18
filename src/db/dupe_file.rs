use super::schema;
use super::sd_pg::*;
use crate::file_cache::CacheFile;
use crate::file_proc::status::{ DbDupeFileInsertProcStatusMessage, StatusMessage };
use crate::utils;
use diesel::prelude::*;
use diesel::result::Error;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;

pub struct DupeFileDb {}

#[derive(Debug, Insertable)]
#[diesel(table_name = schema::dupe_file)]
pub struct DupeFile {
    // pub id: i32,
    pub canonical_path: String,
    pub path_no_drive: String,
    pub file_name: String,
    pub file_extension: Option<String>,
    pub drive_letter: Option<String>,
    pub parent_dir: String,
    pub file_size: i64,
    pub last_modified: SystemTime,
    pub full_hash: i64,
    pub partial_hash: i64,
}
pub const DUPE_FILE_FIELD_COUNT: usize = 10;

impl DupeFile {
    pub fn from_cache_file(cache_file: &CacheFile) -> DupeFile {
        let path_parts = utils::path::extract_path_components(&cache_file.path);
        // dbg!(&path_parts);

        DupeFile {
            // id: 0,
            canonical_path: cache_file.canonical_path.clone(),
            path_no_drive: path_parts.path_without_drive,
            file_name: path_parts.base_filename,
            file_extension: path_parts.extension,
            drive_letter: path_parts.drive_letter,
            parent_dir: path_parts.parent_dir,
            file_size: cache_file.file_size,
            last_modified: cache_file.last_modified,
            full_hash: cache_file.full_hash.unwrap_or(0) as i64,
            partial_hash: cache_file.partial_hash.unwrap_or(0) as i64,
        }
    }
}

impl DupeFileDb {
    pub fn insert_dupe_files(
        dupe_files: &[DupeFile],
        tx_status: &Arc<dyn Fn(StatusMessage) + Send + Sync>
    ) -> Result<usize, Error> {
        tx_status(StatusMessage::DbDupeFileInsertStart);
        let mut connection = establish_connection();

        let chunk_size: usize = POSTGRES_MAX_PARAMETERS / DUPE_FILE_FIELD_COUNT;

        let mut rows_added = 0;

        for chunk in dupe_files.chunks(chunk_size) {
            let rows = diesel
                ::insert_into(schema::dupe_file::table)
                .values(chunk)
                .execute(&mut connection)?;
            rows_added += rows;

            tx_status(
                StatusMessage::DbDupeFileInsertProc(DbDupeFileInsertProcStatusMessage {
                    rows_inserted: rows,
                })
            );

            // TODO: Remove this sleep after testing
            thread::sleep(Duration::from_millis(crate::debug::DEBUG_DB_DUPE_FILE_SLEEP_TIME));
        }

        tx_status(StatusMessage::DbDupeFileInsertFinish);

        Ok(rows_added)
    }
}
