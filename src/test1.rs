use colored::*;
use std::time::Instant;
use tracing::*;

use crate::config::AppConfig;
use crate::debug;
use crate::file_proc;

// use super::file_cache::{
//     delete_cache_file_path_buf, get_file_cache_len, get_put_cache_file_path_buf, put_cache_file,
//     CacheFile,
// };

// pub fn test_cache() -> Result<(), Box<dyn std::error::Error>> {
//     use std::fs;

//     let f = fs::canonicalize("C:\\dev\\deduper\\test-data\\folder2\\file1.wav").unwrap();
//     debug!("Deleteing cache file entry");
//     delete_cache_file_path_buf(&f).unwrap();
//     let count = get_file_cache_len().unwrap();
//     debug!("Count={}", count);
//     debug!("get_put_cache_file_path_buf");
//     let tmp = get_put_cache_file_path_buf(&f).unwrap();
//     let count = get_file_cache_len().unwrap();
//     println!("Count={}", count);
//     // assert_eq!(count, 1);

//     // println!("File: {}", f.to_string_lossy().into_owned());
//     Ok(())
// }

pub fn test_scan(config: &AppConfig) {
    info!("Config: {:?}", config);
    let map = file_proc::scan::build_size_to_files_map(&config.root_paths, &config.ignore_patterns)
        .unwrap();
    debug::print_size_to_files_map(&map);
}

pub fn test_hash(config: &AppConfig) {
    let scan_start = Instant::now();
    let size_map =
        file_proc::scan::build_size_to_files_map(&config.root_paths, &config.ignore_patterns)
            .unwrap();
    let scan_duration = scan_start.elapsed();

    let hash_start = Instant::now();
    let hash_map = file_proc::hash::build_content_hash_map(&size_map).unwrap();
    let hash_duration = hash_start.elapsed();

    debug::print_size_to_files_map(&size_map);
    debug::print_hash_to_files_map(&hash_map);

    debug!(
        "scan completed in {} seconds",
        format_args!("{}", format!("{:.2}", &scan_duration.as_secs_f64()).green()),
    );

    debug!(
        "hash completed in {} seconds",
        format_args!("{}", format!("{:.2}", &hash_duration.as_secs_f64()).green()),
    );
}
