use crate::db::schema;
use diesel::prelude::*;
use std::time::SystemTime;

#[derive(Debug, Insertable)]
#[diesel(table_name = schema::dupe_file)]
pub struct FileInfo_OLD {
    pub canonical_name: String,
    pub drive_letter: String,
    pub path_no_drive: String,
    pub file_size: i64,
    pub last_modified: SystemTime,
    pub content_hash: i64,
    pub parent_dir: String,
}

#[derive(Debug)]
pub struct FileInfo2 {
    pub canonical_name: String,
    pub drive_letter: String,
    pub path_no_drive: String,
    pub file_size: i64,
    pub last_modified: SystemTime,
    pub content_hash: Option<i64>,
    pub partial_hash: Option<i64>,
    pub parent_dir: String,
}

impl FileInfo_OLD {
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

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = schema::dupe_file)]
pub struct DupeFileRead {
    pub id: i32,
    pub canonical_name: String,
    pub drive_letter: String,
    pub path_no_drive: String,
    pub file_size: i64,
    pub last_modified: SystemTime,
    pub content_hash: i64,
    pub parent_dir: String,
}
pub const DUPE_FILE_FIELD_COUNT: usize = 8;

#[derive(Debug, Queryable, Insertable, Selectable, Clone)]
#[diesel(table_name = schema::path_part)]
pub struct PathPart {
    pub id: i32,
    pub canonical_name: String,
    pub name: String,
    pub file_size: i64,
    pub parent_id: Option<i32>,
    pub part_type: i32,
}

pub const PATH_PART_FIELD_COUNT: usize = 6;

// #[derive(Debug, Queryable, Insertable)]
// #[diesel(table_name = schema::path_part)]
// pub struct PathPart {
//     pub id: Option<i32>,
//     pub canonical_name: String,
//     pub name: String,
//     pub file_size: i64,
//     pub parent_id: Option<i32>,
//     pub part_type: i32,
// }
