use super::hash;
use super::scan;
use super::status;
use super::stats::FileProcStats;
use super::status::ProcessStartStatusMessage;
use super::status::{ CacheToDupeProcStatusMessage, StatusMessage };
use crate::db::dupe_file::DupeFile;
use crate::db::dedupe_session::DupeFileDb;
use crate::file_cache::CacheFile;
use crate::utils;
use dashmap::DashMap;
use rayon::iter::{ IntoParallelRefIterator, ParallelIterator };
use std::io;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

pub fn process(
    root_paths: Vec<String>,
    ignore_patterns: Vec<String>
) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the process stats
    let stats = Arc::new(Mutex::new(FileProcStats::default()));

    // Initialize channel for sending status messages
    let (tx, rx): (
        mpsc::Sender<status::StatusMessage>,
        mpsc::Receiver<status::StatusMessage>,
    ) = mpsc::channel();

    // Create a closure so simplify sending status messages
    let tx_status: Arc<dyn Fn(StatusMessage) + Send + Sync> = Arc::new(
        move |msg: status::StatusMessage| {
            tx.send(msg).unwrap(); // Handle this more gracefully in real applications
        }
    );

    // Spawn a thread to process the files
    let process_status = Arc::clone(&stats);
    let process_handle = thread::spawn(move || {
        // let stats = Arc::clone(&stats);
        process_inner(
            root_paths.to_vec(),
            ignore_patterns.to_vec(),
            Arc::clone(&process_status),
            // tx,
            &tx_status
        ).unwrap();
    });

    let status_stats = Arc::clone(&stats);
    let status_handle = thread::spawn({
        move || {
            status::handle_status(rx, Arc::clone(&status_stats));
        }
    });

    // // Wait for the threads to finish
    process_handle.join().unwrap();
    status_handle.join().unwrap();

    let final_stats: FileProcStats = stats.lock().unwrap().clone();
    let final_stats_clone = final_stats.clone();

    final_stats.print();
    let _ = final_stats_clone.write_csv("stats.csv");

    Ok(())
}

fn process_inner(
    root_paths: Vec<String>,
    ignore_patterns: Vec<String>,
    stats: Arc<Mutex<FileProcStats>>,
    tx_status: &Arc<dyn Fn(status::StatusMessage) + Send + Sync>
) -> Result<(), Box<dyn std::error::Error>> {
    let input_paths = utils::to_non_overlapping_directories(&root_paths.clone());

    tx_status(
        StatusMessage::ProcessStart(ProcessStartStatusMessage {
            input_paths: input_paths.clone(),
        })
    );

    let size_map = scan
        ::build_size_to_files_map(&input_paths, &ignore_patterns, tx_status)
        .unwrap();

    let hash_map = hash::build_content_hash_map(&size_map, tx_status).unwrap();

    let dupe_files = cache_file_map_to_dupe_files(hash_map, tx_status).unwrap();

    // let _db_rows = DupeFileDb::insert_dupe_files(&dupe_files, tx_status)?;
    let stats = Arc::clone(&stats);
    // let stats = stats.lock().unwrap();
    let write_id = DupeFileDb::write_session(stats, &dupe_files, tx_status)?;

    tx_status(StatusMessage::ProcessFinish);

    Ok(())
}

fn cache_file_map_to_dupe_files(
    map: DashMap<u64, Vec<CacheFile>>,
    tx_status: &Arc<dyn Fn(status::StatusMessage) + Send + Sync>
) -> io::Result<Vec<DupeFile>> {
    tx_status(StatusMessage::CacheToDupeStart);
    let entries: Vec<_> = map.iter().collect();
    let dupe_file_vec: Result<Vec<_>, io::Error> = entries
        .par_iter()
        .flat_map(|entry| {
            let cache_files = entry.value();

            // TODO: Remove this sleep after testing
            thread::sleep(Duration::from_millis(crate::debug::DEBUG_CACHE_TO_VEC_SLEEP_TIME));

            tx_status(
                StatusMessage::CacheToDupeProc(CacheToDupeProcStatusMessage {
                    count: cache_files.len(),
                })
            );

            cache_files.par_iter().map(move |cache_file| {
                let dupe_file = DupeFile::from_cache_file(cache_file);
                Ok(dupe_file)
            })
        })
        .collect();
    tx_status(StatusMessage::CacheToDupeFinish);
    dupe_file_vec
}
