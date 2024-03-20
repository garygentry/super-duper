use crate::db;
use crate::model;
use colored::*;
use std::time::Instant;

mod debug;
mod file_info;
mod hash;
mod scan;
mod win;

pub fn process(root_paths: &Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing paths: {:?}", root_paths);
    let root_path_slices: Vec<&str> = root_paths.iter().map(|s| s.as_str()).collect();

    /*
        Scan and build index on file size
    */
    let scan_start_time = Instant::now();
    let size_to_files_map = scan::build_size_to_files_map(&&root_path_slices)?;
    let scan_duration = scan_start_time.elapsed();
    debug::print_size_to_files_map(&size_to_files_map);

    /*
        Build ccontent Hash
    */
    let hash_start_time = Instant::now();
    let content_hash_map = hash::build_content_hash_map(size_to_files_map)?;
    let hash_duration = hash_start_time.elapsed();
    debug::print_content_hash_map(&content_hash_map);

    /*
        Build File Info
    */
    let fi_start_time = Instant::now();
    let file_info_vector = file_info::build_file_info_vec(content_hash_map)?;
    let fi_duration = fi_start_time.elapsed();
    debug::print_file_info_vec(&file_info_vector);

    /*
        Write to db
    */
    let db_start_time = Instant::now();
    db::dupe_file::insert_file_info_vec(file_info_vector)?;
    let db_duration = db_start_time.elapsed();
    // debug::print_file_info_vec(&file_info_vector);

    /*
        Summary
    */
    println!(
        "File Scan completed in {} seconds",
        format!("{}", format!("{:.2}", scan_duration.as_secs_f64()).green())
    );

    println!(
        "File Hash completed in {} seconds",
        format!("{}", format!("{:.2}", hash_duration.as_secs_f64()).green())
    );

    println!(
        "File Info completed in {} seconds",
        format!("{}", format!("{:.2}", fi_duration.as_secs_f64()).green())
    );

    println!(
        "Db update completed in {} seconds",
        format!("{}", format!("{:.2}", db_duration.as_secs_f64()).green())
    );

    Ok(())
}

// pub fn process(root_paths: &Vec<String>) {
//     println!("Root Files: {:?}", root_paths);
//     /*
//      Scan
//     */
//     let root_path_slices: Vec<&str> = root_paths.iter().map(|s| s.as_str()).collect();
//     let scan_start_time = Instant::now();
//     // let mut scan_duration Instant::min(self, other);
//     match scan::scan(&&root_path_slices) {
//         Ok(map) => {
//             let scan_duration = scan_start_time.elapsed();
//             let hash_start_time = Instant::now();
//             let _ = hash::calculate_checksums(map, false);
//             let hash_duration = hash_start_time.elapsed();
//             println!(
//                 "File Hash completed in {} seconds",
//                 format!("{}", format!("{:.2}", hash_duration.as_secs_f64()).green())
//             );
//         }
//         Err(e) => eprintln!("Error scanning directories: {}", e),
//     }

//     println!(
//         "File Scan completed in {} seconds",
//         format!("{}", format!("{:.2}", scan_duration.as_secs_f64()).green())
//     );
// }
