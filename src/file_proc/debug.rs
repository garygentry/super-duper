#![allow(dead_code)]

use dashmap::DashMap;
use std::path::PathBuf;
use tracing::debug;

pub fn print_size_to_files_map(map: &DashMap<u64, Vec<PathBuf>>) {
    for entry in map.iter() {
        let (key, value) = entry.pair();
        println!("Key (file size): {}", key);
        for path in value.iter() {
            println!("\t{:?}", path);
        }
    }
}

pub fn print_content_hash_map(checksum_map: &DashMap<u64, Vec<PathBuf>>) {
    for entry in checksum_map.iter() {
        let (checksum, paths) = entry.pair();
        println!("Hash: {}", checksum);
        for path in paths.iter() {
            println!("\tFile: {:?}", path);
        }
        println!();
    }
}

pub fn print_file_info_vec(file_info_vec: &Vec<super::model::FileInfo>) {
    for file_info in file_info_vec {
        // file_info.print();
        println!("{:?}", file_info)
    }
}

#[derive(Debug)]
pub struct SizeToFilesMapStats {
    pub key_count: u64,
    pub total_distinct_size: u64,
    pub total_file_count: usize,
    pub total_size: u64,
}

pub fn print_size_to_files_map_stats(map: &DashMap<u64, Vec<PathBuf>>) {
    let stats = get_size_to_files_map_stats(map);

    debug!(
        "SIZE_MAP_STATS: Total number of distinct files: {}",
        stats.key_count
    );
    debug!(
        "SIZE_MAP_STATS: Total size of distinct files: {}",
        stats.total_distinct_size
    );
    debug!(
        "SIZE_MAP_STATS: Total number of files: {}",
        stats.total_file_count
    );
    debug!(
        "SIZE_MAP_STATS: Total size of all files: {}",
        stats.total_size
    );
}

pub fn get_size_to_files_map_stats(map: &DashMap<u64, Vec<PathBuf>>) -> SizeToFilesMapStats {
    let mut total_size = 0;
    let mut total_distinct_size = 0;
    let mut key_count = 0;
    let mut total_file_count = 0;

    for entry in map.iter() {
        let key = entry.key();
        let value = entry.value();

        key_count += 1;

        total_distinct_size += *key;

        // Count of all key values
        // Sum of counts for each vector in the value
        total_file_count += value.len();

        // Sum of values calculated by multiplying the key value by the count of items in the vector
        total_size += *key * value.len() as u64;
    }

    SizeToFilesMapStats {
        key_count,
        total_distinct_size,
        total_file_count,
        total_size,
    }
}
