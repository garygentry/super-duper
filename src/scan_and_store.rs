use crate::file_proc::status::{
    StatusMessage,
    ProcessStartStatusMessage,
    handle_status,
};
use crate::file_proc::stats::FileProcStats;
use crate::store::SuperDuperStore;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use crate::file_proc::dupes::build_dupe_files;
use crate::config::AppConfig;
use crate::utils::stats::StatsTimer;

pub fn scan_and_store_dupes(
    root_paths: Vec<String>,
    ignore_patterns: Vec<String>,
    app_config: AppConfig
) -> Result<(), Box<dyn std::error::Error + Send>> {
    // Initialize the process stats
    let stats = Arc::new(Mutex::new(FileProcStats::default()));

    // Initialize channel for sending status messages
    let (tx, rx): (
        mpsc::Sender<StatusMessage>,
        mpsc::Receiver<StatusMessage>,
    ) = mpsc::channel();

    // Create a closure so simplify sending status messages
    let tx_status: Arc<dyn Fn(StatusMessage) + Send + Sync> = Arc::new(
        move |msg: StatusMessage| {
            tx.send(msg).unwrap();
        }
    );

    let status_stats = Arc::clone(&stats);
    let status_handle = thread::spawn({
        move || {
            handle_status(rx, Arc::clone(&status_stats), app_config.clone());
        }
    });

    // Spawn a thread to process the files
    let process_handle = thread::spawn(move || {
        tx_status(
            StatusMessage::ProcessStart(ProcessStartStatusMessage {
                input_paths: root_paths.clone(),
            })
        );

        let dupes = build_dupe_files(
            root_paths.to_vec(),
            ignore_patterns.to_vec(),
            &tx_status
        );

        tx_status(StatusMessage::ProcessFinish);
        dupes
    });

    // Wait for the threads to finish
    let dupe_files = process_handle.join().unwrap()?;
    status_handle.join().unwrap();

    let stats_db = Arc::clone(&stats);

    let mut persist_timer = StatsTimer::new();
    SuperDuperStore::persist(stats_db, &dupe_files).unwrap();
    persist_timer.finish();

    let final_stats: FileProcStats = stats.lock().unwrap().clone();
    let final_stats_clone = final_stats.clone();

    final_stats.print();
    let _ = final_stats_clone.write_csv("stats.csv");

    println!("\nPersist to store in: {}", persist_timer.get_duration_human());

    Ok(())
}
