#![allow(dead_code)]

use crate::{file_cache::CacheFile, model::ScanFile};
use dashmap::DashMap;
// use std::path::PathBuf;
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
