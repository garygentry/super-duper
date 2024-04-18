#![allow(dead_code)]

pub const DEBUG_SCAN_SLEEP_TIME: u64 = 0;
pub const DEBUG_HASH_SLEEP_TIME: u64 = 0;
pub const DEBUG_CACHE_TO_VEC_SLEEP_TIME: u64 = 0;
pub const DEBUG_DB_DUPE_FILE_SLEEP_TIME: u64 = 0;

use crate::{ db::dupe_file::DupeFile, file_cache::CacheFile, file_proc::scan::ScanFile };
use dashmap::DashMap;
use tracing::*;

pub fn print_size_to_files_map(map: &DashMap<u64, Vec<ScanFile>>) {
    for entry in map.iter() {
        let (key, value) = entry.pair();
        debug!("Key (file size): {}", key);
        for path in value.iter() {
            debug!("\t{:?}", path.path_buf);
        }
    }
}

pub fn print_hash_to_files_map(map: &DashMap<u64, Vec<CacheFile>>) {
    for entry in map.iter() {
        let (key, value) = entry.pair();
        debug!("Key (file size): {}", key);
        for cf in value.iter() {
            debug!("\t{:?}", cf);
        }
    }
}

pub fn print_dupe_files(dupe_files: &[DupeFile]) {
    for dupe_file in dupe_files.iter() {
        debug!("{:?}", dupe_file);
    }
}
