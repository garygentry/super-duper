// use colored::*;
// use dashmap::DashMap;
// use rayon::prelude::*;
// use std::io;
// use std::time::Instant;
// use tracing::*;

// use crate::config::AppConfig;
// use crate::debug;
// use crate::file_cache::CacheFile;
// use crate::file_proc;

// // use super::file_cache::{
// //     delete_cache_file_path_buf, get_file_cache_len, get_put_cache_file_path_buf, put_cache_file,
// //     CacheFile,
// // };

// // pub fn test_cache() -> Result<(), Box<dyn std::error::Error>> {
// //     use std::fs;

// //     let f = fs::canonicalize("C:\\dev\\deduper\\test-data\\folder2\\file1.wav").unwrap();
// //     debug!("Deleteing cache file entry");
// //     delete_cache_file_path_buf(&f).unwrap();
// //     let count = get_file_cache_len().unwrap();
// //     debug!("Count={}", count);
// //     debug!("get_put_cache_file_path_buf");
// //     let tmp = get_put_cache_file_path_buf(&f).unwrap();
// //     let count = get_file_cache_len().unwrap();
// //     println!("Count={}", count);
// //     // assert_eq!(count, 1);

// //     // println!("File: {}", f.to_string_lossy().into_owned());
// //     Ok(())
// // }

// pub fn test_scan(config: &AppConfig) {
//     info!("Config: {:?}", config);
//     let map = file_proc::scan::build_size_to_files_map(&config.root_paths, &config.ignore_patterns)
//         .unwrap();
//     debug::print_size_to_files_map(&map);
// }

// pub fn test_hash(config: &AppConfig) {
//     let scan_start = Instant::now();
//     let size_map =
//         file_proc::scan::build_size_to_files_map(&config.root_paths, &config.ignore_patterns)
//             .unwrap();
//     let scan_duration = scan_start.elapsed();

//     let hash_start = Instant::now();
//     let hash_map = file_proc::hash::build_content_hash_map(&size_map).unwrap();
//     let hash_duration = hash_start.elapsed();

//     // debug::print_size_to_files_map(&size_map);
//     // debug::print_hash_to_files_map(&hash_map);
//     let dupe_files = cache_file_map_to_dupe_files(hash_map).unwrap();
//     for dupe_file in dupe_files.iter() {
//         debug!("{:?}", dupe_file);
//     }

//     /*
//         Summary
//     */
//     debug!(
//         // "File Scan completed in {} seconds, File Hash completed in {} seconds, File Info completed in {} seconds, Database update completed in {} seconds",
//         "File Scan completed in {} seconds, File Hash completed in {} seconds",
//         format_args!("{}", format!("{:.2}", &scan_duration.as_secs_f64()).green()),
//         format_args!("{}", format!("{:.2}", &hash_duration.as_secs_f64()).green()),
//         // format_args!("{}", format!("{:.2}", &fi_duration.as_secs_f64()).green()),
//         // format_args!("{}", format!("{:.2}", &db_duration.as_secs_f64()).green())
//     );
// }

// fn cache_file_map_to_dupe_files(
//     map: DashMap<u64, Vec<CacheFile>>,
// ) -> io::Result<Vec<crate::db::dupe_file::DupeFile>> {
//     let entries: Vec<_> = map.iter().collect();
//     let dupe_file_vec: Result<Vec<_>, io::Error> = entries
//         .par_iter()
//         .flat_map(|entry| {
//             let cache_files = entry.value();

//             cache_files.par_iter().map(move |cache_file| {
//                 let dupe_file = crate::db::dupe_file::DupeFile::from_cache_file(cache_file);
//                 Ok(dupe_file)
//             })
//         })
//         .collect();
//     dupe_file_vec
// }
