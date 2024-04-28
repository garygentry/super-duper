use super::sd_pg::*;
use std::sync::Arc;
use std::sync::Mutex;
use crate::file_proc::stats::FileProcStats;
use super::dupe_file::DupeFile;
use super::dedupe_session::DedupeSession;
use super::dupe_file_part::DupeFilePart;

pub struct SuperDuperStore {}

impl SuperDuperStore {
    pub fn persist(
        stats: Arc<Mutex<FileProcStats>>,
        dupe_files: &[DupeFile]
        // tx_status: &Arc<dyn Fn(StatusMessage) + Send + Sync>
    ) -> Result<i32, diesel::result::Error> {
        // tx_status(StatusMessage::DbDupeFileInsertStart);
        let mut connection = establish_connection();

        println!("Creating session...");
        let session_id = DedupeSession::insert_session(&mut connection, stats)?;
        println!("Session id={}", session_id);

        println!("Inserting dupe files...");
        DupeFile::insert_dupe_files(dupe_files, session_id)?;

        println!("Creating dupe file parts...");
        let parts = DupeFilePart::parts_from_dupe_files(dupe_files, &session_id);

        println!("Inserting dupe file parts...");
        DupeFilePart::insert_dupe_file_parts(&mut connection, &parts)?;

        Ok(session_id)
    }
}
