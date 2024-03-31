use crate::db;
use crate::model;
use colored::*;
use std::time::Instant;
use tracing::{debug, info};

mod debug;
mod file_info;
pub mod hash;
mod scan;
mod scan_dir;
mod win;

pub fn process(
    root_paths: &Vec<String>,
    ignore_patterns: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing paths: {:?}", root_paths);
    let root_path_slices: Vec<&str> = root_paths.iter().map(|s| s.as_str()).collect();
    let ignore_patterns_slices: Vec<&str> = ignore_patterns.iter().map(|s| s.as_str()).collect();

    /*
        Scan and build index on file size
    */
    info!("Scanning files...");
    let scan_start_time = Instant::now();
    let size_to_files_map =
        scan::build_size_to_files_map(&root_path_slices, &ignore_patterns_slices)?;
    let scan_duration = scan_start_time.elapsed();
    // debug::print_size_to_files_map(&size_to_files_map);
    debug!(
        "Scan files completed in {} seconds",
        format_args!("{}", format!("{:.2}", &scan_duration.as_secs_f64()).green()),
    );

    let print_size_map_start_time = Instant::now();
    debug::print_size_to_files_map_stats(&size_to_files_map);
    let print_size_map_duration = print_size_map_start_time.elapsed();
    debug!(
        "print size map completed in {} seconds",
        format_args!(
            "{}",
            format!("{:.2}", &print_size_map_duration.as_secs_f64()).green()
        ),
    );
    /*
        Build content Hash
    */
    info!("Building content hash for possible dupes...");
    let hash_start_time = Instant::now();
    let content_hash_map = hash::build_content_hash_map(size_to_files_map)?;
    let hash_duration = hash_start_time.elapsed();
    let dupe_file_count = content_hash_map.len();
    debug!(
        "Build content hash completed in {} seconds",
        format_args!("{}", format!("{:.2}", &hash_duration.as_secs_f64()).green()),
    );
    info!(
        "{} files with duplicates",
        format_args!("{}", format!("{:.2}", &dupe_file_count).red()),
    );

    /*
        Build File Info
    */
    info!("Collecting file info...");
    let fi_start_time = Instant::now();
    let file_info_vector = file_info::build_file_info_vec(content_hash_map)?;
    let fi_duration = fi_start_time.elapsed();
    debug!(
        "Build file_info completed in {} seconds",
        format_args!("{}", format!("{:.2}", &fi_duration.as_secs_f64()).green()),
    );

    /*
        Write to db
    */
    info!("Writing to database...");
    let db_start_time = Instant::now();
    db::dupe_file::insert_file_info_vec(file_info_vector)?;
    let db_duration = db_start_time.elapsed();
    debug!(
        "Write to database completed in {} seconds",
        format_args!("{}", format!("{:.2}", &db_duration.as_secs_f64()).green()),
    );

    /*
        Summary
    */
    debug!(
        "File Scan completed in {} seconds, File Hash completed in {} seconds, File Info completed in {} seconds, Database update completed in {} seconds",
        format_args!("{}", format!("{:.2}", &scan_duration.as_secs_f64()).green()),
        format_args!("{}", format!("{:.2}", &hash_duration.as_secs_f64()).green()),
        format_args!("{}", format!("{:.2}", &fi_duration.as_secs_f64()).green()),
        format_args!("{}", format!("{:.2}", &db_duration.as_secs_f64()).green())
    );

    Ok(())
}
