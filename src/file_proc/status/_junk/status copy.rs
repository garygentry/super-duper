use console::{style, Term};
use indicatif::MultiProgress;
use indicatif::{HumanBytes, HumanCount, HumanDuration, ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessStats {
    pub scan_file_count: usize,
    pub scan_file_size: u64,
    pub scan_dupe_file_count: usize,
    pub scan_dupe_file_size: u64,

    pub hash_proc_scan_file_count: usize,
    pub hash_proc_scan_file_size: u64,
    pub hash_full_cache_hit_count: usize,
    pub hash_partial_cache_hit_count: usize,
    pub hash_partial_gen_count: usize,

    pub hash_full_gen_count: usize,
    pub hash_proc_confirmed_dupe_count: usize,

    pub cache_to_dupe_file_count: usize,
    pub db_dupe_file_insert_count: usize,
}

#[derive(Debug, Clone)]
pub struct ScanAddRawStatusMessage {
    pub file_path: PathBuf,
    pub file_size: u64,
}

#[derive(Debug, Default, Clone)]
pub struct ScanAddDupeStatusMessage {
    pub count: usize,
    pub file_size: u64,
}

#[derive(Debug, Default, Clone)]
pub struct HashProcStatusMessage {
    pub scan_file_proc_count: usize,
    pub full_cache_hit_count: usize,
    pub partial_cache_hit_count: usize,
    pub file_size: u64,
    pub confirmed_dupe_count: usize,
}

#[derive(Debug, Default, Clone)]
pub struct HashGenCacheFileStatusMessage {
    pub canonical_path: String,
    pub partial_count: usize,
    pub full_count: usize,
}

#[derive(Debug, Default, Clone)]
pub struct CacheToDupeProcStatusMessage {
    pub count: usize,
}

#[derive(Debug, Default, Clone)]
pub struct DbDupeFileInsertProcStatusMessage {
    pub rows_inserted: usize,
}

#[derive(Clone, Debug)]
pub enum StatusMessage {
    ProcessBegin,
    ScanBegin,
    ScanAddRaw(ScanAddRawStatusMessage),
    ScanAddDupe(ScanAddDupeStatusMessage),
    ScanEnd,
    HashBegin,
    HashProc(HashProcStatusMessage),
    HashGenCacheFile(HashGenCacheFileStatusMessage),
    HashEnd,
    CacheToDupeBegin,
    CacheToDupeProc(CacheToDupeProcStatusMessage),
    CacheToDupeEnd,
    DbDupeFileInsertBegin,
    DbDupeFileInsertProc(DbDupeFileInsertProcStatusMessage),
    DbDupeFileInsertEnd,
    ProcessEnd,
}

fn get_pb() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    let spinner_style =
        ProgressStyle::with_template("[{elapsed_precise}] {prefix:.bold.dim} {spinner} {wide_msg}")
            .unwrap()
            // .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
            .tick_strings(&[".  ", ".. ", "...", " ..", "  .", "   "]);

    pb.set_style(spinner_style);
    pb
}

fn get_pb_bar() -> ProgressBar {
    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::with_template(&format!(
            "{{prefix:.bold}}▕{{bar:.{}}}▏{{msg}}",
            String::from("blue")
        ))
        .unwrap()
        .progress_chars("█▓▒░  "),
    );
    pb
}

// fn update_stats(pb: &ProgressBar, stats: ProcessStats) {
//     let message = format!(
//         "Scanned Files: {}, Scanned File Size: {}, Scan Dupe Count: {}, Scan Dupe Size: {}, Hashed Files: {}, Hashed File Size: {}",
//         stats.scan_file_count,
//         stats.scan_file_size,
//         stats.scan_dupe_file_count,
//         stats.scan_dupe_file_size,
//         stats.hash_file_count,
//         stats.hash_file_size
//     );
//     pb.set_message(message);
// }

pub fn handle_status(rx: mpsc::Receiver<StatusMessage>, stats: Arc<Mutex<ProcessStats>>) {
    let m = MultiProgress::new();

    // let pb_stats = m.add(get_pb());
    let pb_scan = m.add(get_pb());
    let pb_hash = m.add(get_pb());
    let pb_hash_bar = m.add(get_pb_bar());
    let pb_cache_to_dupe_proc_bar = m.add(get_pb_bar());
    let pb_db_dupe_file_proc_bar = m.add(get_pb_bar());

    let term = Term::stdout();

    for message in rx {
        let mut stats = stats.lock().unwrap();
        match message {
            StatusMessage::ProcessBegin => {
                term.hide_cursor().unwrap();
            }
            StatusMessage::ScanBegin => {
                // pb_scan.set_prefix("Scanning...")
                // pb_scan.println("Scanning files...");
            }
            StatusMessage::ScanAddRaw(msg) => {
                stats.scan_file_count += 1;
                stats.scan_file_size += msg.file_size;
                let message = format!(
                    "Scanned {} files, total size {} ({})",
                    style(HumanCount(stats.scan_file_count as u64))
                        .bold()
                        .green(),
                    HumanBytes(stats.scan_file_size),
                    msg.file_path.display()
                );
                pb_scan.tick();
                pb_scan.set_message(message);
            }
            StatusMessage::ScanAddDupe(msg) => {
                stats.scan_dupe_file_count += msg.count;
                stats.scan_dupe_file_size += msg.count as u64 * msg.file_size;
            }
            StatusMessage::ScanEnd => {
                let message = format!(
                    "Scanned {} files, total size {} in {}",
                    stats.scan_file_count,
                    stats.scan_file_size,
                    HumanDuration(pb_scan.elapsed())
                );
                pb_scan.finish_with_message(message);
            }
            StatusMessage::HashBegin => {
                pb_hash_bar.set_length(stats.scan_dupe_file_size);
                pb_hash_bar.set_prefix("Hashing files:  ")
            }
            StatusMessage::HashProc(msg) => {
                stats.hash_proc_scan_file_count += msg.scan_file_proc_count;
                stats.hash_proc_scan_file_size += msg.scan_file_proc_count as u64 * msg.file_size;
                stats.hash_full_cache_hit_count += msg.full_cache_hit_count;
                stats.hash_partial_cache_hit_count += msg.partial_cache_hit_count;
                stats.hash_proc_confirmed_dupe_count += msg.confirmed_dupe_count;

                let message = format!(
                    "Hashed {} files, file size {}, total count {}, total size {}",
                    msg.scan_file_proc_count,
                    msg.file_size,
                    stats.hash_proc_scan_file_count,
                    stats.hash_proc_scan_file_size
                );
                pb_hash.tick();
                pb_hash.set_message(message);
                pb_hash_bar.set_position(stats.hash_proc_scan_file_size);
            }
            StatusMessage::HashGenCacheFile(msg) => {
                // msg.canonical_path
                stats.hash_proc_confirmed_dupe_count += msg.full_count;
            }

            StatusMessage::HashEnd => {
                pb_hash_bar.set_position(stats.hash_proc_scan_file_count as u64);
                pb_hash.finish_with_message("Hashing files complete.");
            }
            StatusMessage::CacheToDupeBegin => {
                pb_cache_to_dupe_proc_bar.set_length(stats.hash_proc_confirmed_dupe_count as u64);
            }
            StatusMessage::CacheToDupeProc(msg) => {
                stats.cache_to_dupe_file_count += msg.count;
                pb_cache_to_dupe_proc_bar.set_position(stats.cache_to_dupe_file_count as u64);
            }
            StatusMessage::CacheToDupeEnd => {
                pb_cache_to_dupe_proc_bar
                    .finish_with_message("Dupe Files Vec created successfully.");
            }
            StatusMessage::DbDupeFileInsertBegin => {
                pb_db_dupe_file_proc_bar.set_length(stats.cache_to_dupe_file_count as u64);
            }
            StatusMessage::DbDupeFileInsertProc(msg) => {
                stats.db_dupe_file_insert_count += msg.rows_inserted;
                pb_db_dupe_file_proc_bar.set_position(stats.db_dupe_file_insert_count as u64);
            }
            StatusMessage::DbDupeFileInsertEnd => {
                let msg = format!(
                    "{} rows inserted for dupe_files in db",
                    stats.db_dupe_file_insert_count
                );

                pb_db_dupe_file_proc_bar.finish_with_message(msg);
            }

            StatusMessage::ProcessEnd => {
                // println!("Done");
                // println!("ProcessEnd");
                // let message = format!(
                //     "ALL Scanned Files: {}, Scanned File Size: {}, Scan Dupe Count: {}, Scan Dupe Size: {}, Hashed Files: {}, Hashed File Size: {}",
                //     stats.scan_file_count,
                //     stats.scan_file_size,
                //     stats.scan_dupe_file_count,
                //     stats.scan_dupe_file_size,
                //     stats.hash_file_count,
                //     stats.hash_file_size
                // );
                // pb_stats.finish_with_message(message);
                term.show_cursor().unwrap();
            }
        }
        // update_stats(&pb_stats, *stats);
    }
}
