use crate::db::schema;
use diesel::prelude::*;
use std::time::SystemTime;

#[derive(Debug, Insertable)]
#[diesel(table_name = schema::dupe_file)]
pub struct FileInfo {
    pub canonical_name: String,
    pub file_index: i64,
    pub drive_letter: String,
    pub path_no_drive: String,
    pub file_size: i64,
    pub last_modified: SystemTime,
    pub content_hash: i64,
    pub volume_serial_number: i32,
}

impl FileInfo {
    pub fn print(&self) {
        // let duration = self.last_modified.duration_since(UNIX_EPOCH).unwrap();
        // println!("Canonical Name: {}", self.canonical_name);
        // println!("File Size: {}", self.file_size);
        // println!(
        //     "Last Modified: {} seconds since UNIX_EPOCH",
        //     duration.as_secs()
        // );
        println!("Hash: {}", self.content_hash);
    }
}
