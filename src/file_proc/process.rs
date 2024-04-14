use super::hash;
use super::scan;
use super::status;
use super::status::StatusMessage;
use crate::db::dupe_file::{DupeFile, DupeFileDb};
use crate::file_cache::CacheFile;
use colored::*;
use dashmap::DashMap;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::{io, time::Instant};
use tracing::*;

pub fn process(
    root_paths: Vec<String>,
    ignore_patterns: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the process stats
    let stats = Arc::new(Mutex::new(status::ProcessStats::default()));

    // Initialize channel for sending status messages
    let (tx, rx): (
        mpsc::Sender<status::StatusMessage>,
        mpsc::Receiver<status::StatusMessage>,
    ) = mpsc::channel();

    // Create a closure so simplify sending status messages
    let tx_status: Arc<dyn Fn(StatusMessage) + Send + Sync> =
        Arc::new(move |msg: status::StatusMessage| {
            tx.send(msg).unwrap(); // Handle this more gracefully in real applications
        });

    // Spawn a thread to process the files
    let process_handle = thread::spawn(move || {
        process_inner(
            root_paths.to_vec(),
            ignore_patterns.to_vec(),
            // tx,
            &tx_status,
        )
        .unwrap();
    });

    // Spawn a thread to handle status messages
    let status_handle = thread::spawn(move || {
        status::handle_status(rx, Arc::clone(&stats));
    });

    // Wait for the threads to finish
    process_handle.join().unwrap();
    status_handle.join().unwrap();

    Ok(())
}

fn process_inner(
    root_paths: Vec<String>,
    ignore_patterns: Vec<String>,
    tx_status: &Arc<dyn Fn(status::StatusMessage) + Send + Sync>,
) -> Result<(), Box<dyn std::error::Error>> {
    tx_status(StatusMessage::ProcessBegin);

    let scan_start = Instant::now();

    let size_map = scan::build_size_to_files_map(&root_paths, &ignore_patterns, tx_status).unwrap();

    let scan_duration = scan_start.elapsed();
    // debug::print_size_to_files_map(&size_map);

    let hash_start = Instant::now();
    let hash_map = hash::build_content_hash_map(&size_map, tx_status).unwrap();

    let hash_duration = hash_start.elapsed();
    // debug::print_hash_to_files_map(&hash_map);

    let dupe_start = Instant::now();
    let dupe_files = cache_file_map_to_dupe_files(hash_map).unwrap();
    let dupe_duration = dupe_start.elapsed();
    // debug::print_dupe_files(&dupe_files);

    let db_start = Instant::now();
    let db_rows = DupeFileDb::insert_dupe_files(&dupe_files)?;
    let db_duration = db_start.elapsed();

    // info!(
    //     "File Scan completed in {} seconds, File Hash completed in {} seconds, File Info completed in {} seconds, Inserted {} rows in {} seconds",
    //     format_args!("{}", format!("{:.2}", &scan_duration.as_secs_f64()).green()),
    //     format_args!("{}", format!("{:.2}", &hash_duration.as_secs_f64()).green()),
    //     format_args!("{}", format!("{:.2}", &dupe_duration.as_secs_f64()).green()),
    //     format_args!("{}", format!("{:.2}", &db_rows).green()),
    //     format_args!("{}", format!("{:.2}", &db_duration.as_secs_f64()).green())
    // );
    // tx_status.send(StatusMessage::ProcessEnd).unwrap();
    tx_status(StatusMessage::ProcessEnd);

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
