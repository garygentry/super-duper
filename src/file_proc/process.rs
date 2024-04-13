use super::hash;
use super::scan;
use crate::db::dupe_file::{DupeFile, DupeFileDb};
use crate::debug;
use crate::file_cache::CacheFile;
use colored::*;
use dashmap::DashMap;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{io, time::Instant};
use tracing::*;

pub fn process(
    root_paths: &[String],
    ignore_patterns: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Scanning input directories...");
    let scan_start = Instant::now();
    let size_map = scan::build_size_to_files_map(root_paths, ignore_patterns).unwrap();
    let scan_duration = scan_start.elapsed();
    debug::print_size_to_files_map(&size_map);

    info!("Creating hash map...");
    let hash_start = Instant::now();
    let hash_map = hash::build_content_hash_map(&size_map).unwrap();
    let hash_duration = hash_start.elapsed();
    debug::print_hash_to_files_map(&hash_map);

    info!("Preparing dupe files...");
    let dupe_start = Instant::now();
    let dupe_files = cache_file_map_to_dupe_files(hash_map).unwrap();
    let dupe_duration = dupe_start.elapsed();
    debug::print_dupe_files(&dupe_files);

    info!("Writing dupe files to database...");
    let db_start = Instant::now();
    let db_rows = DupeFileDb::insert_dupe_files(&dupe_files)?;
    let db_duration = db_start.elapsed();

    debug!(
        "File Scan completed in {} seconds, File Hash completed in {} seconds, File Info completed in {} seconds, Inserted {} rows in {} seconds",
        format_args!("{}", format!("{:.2}", &scan_duration.as_secs_f64()).green()),
        format_args!("{}", format!("{:.2}", &hash_duration.as_secs_f64()).green()),
        format_args!("{}", format!("{:.2}", &dupe_duration.as_secs_f64()).green()),
        format_args!("{}", format!("{:.2}", &db_rows).green()),
        format_args!("{}", format!("{:.2}", &db_duration.as_secs_f64()).green())
    );
    Ok(())
}

fn cache_file_map_to_dupe_files(map: DashMap<u64, Vec<CacheFile>>) -> io::Result<Vec<DupeFile>> {
    let entries: Vec<_> = map.iter().collect();
    let dupe_file_vec: Result<Vec<_>, io::Error> = entries
        .par_iter()
        .flat_map(|entry| {
            let cache_files = entry.value();

            cache_files.par_iter().map(move |cache_file| {
                let dupe_file = DupeFile::from_cache_file(cache_file);
                Ok(dupe_file)
            })
        })
        .collect();
    dupe_file_vec
}
