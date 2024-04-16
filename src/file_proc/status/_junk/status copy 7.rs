use console::{ style, Term };
use indicatif::MultiProgress;
use indicatif::{ HumanBytes, HumanCount, HumanDuration, ProgressBar, ProgressStyle };
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

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
    let spinner_style = ProgressStyle::with_template(
        "[{elapsed_precise}] {spinner} {prefix:.bold.dim} {wide_msg}"
    )
        .unwrap()
        // .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
        .tick_strings(&[".  ", ".. ", "...", " ..", "  .", "   "]);

    pb.set_style(spinner_style);
    pb
}

fn get_pb_bar() -> ProgressBar {
    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::with_template(
            &format!(
                "[{{elapsed_precise}}] {{prefix:.bold}}▕{{bar:.{}}}▏{{percent}} {{msg}}",
                String::from("blue")
            )
        )
            .unwrap()
            .progress_chars("█▓▒░  ")
    );
    pb
}

#[derive(Hash, Eq, PartialEq)]
enum ProgressBarType {
    Scan,
    Hash,
    HashBar,
    CacheToDupe,
    CacheToDupeBar,
    Db,
}

fn init_pb() -> (HashMap<ProgressBarType, ProgressBar>, MultiProgress) {
    let m = MultiProgress::new();

    // let pb_scan = m.add(get_pb());
    // let pb_hash = m.add(get_pb());
    // let pb_hash_bar = m.add(get_pb_bar());
    // let pb_cache_to_dupe = m.add(get_pb());
    // let pb_cache_to_dupe_proc_bar = m.add(get_pb_bar());
    // let pb_db_dupe_file_proc_bar = m.add(get_pb_bar());

    let mut pb_map = HashMap::new();
    pb_map.insert(ProgressBarType::Scan, m.add(get_pb()));
    pb_map.insert(ProgressBarType::Hash, m.add(get_pb()));
    pb_map.insert(ProgressBarType::HashBar, m.add(get_pb_bar()));
    pb_map.insert(ProgressBarType::CacheToDupe, m.add(get_pb()));
    pb_map.insert(ProgressBarType::CacheToDupeBar, m.add(get_pb_bar()));
    pb_map.insert(ProgressBarType::Db, m.add(get_pb_bar()));

    (pb_map, m)
}

pub fn handle_status(rx: mpsc::Receiver<StatusMessage>, stats: Arc<Mutex<ProcessStats>>) {
    let finish_style = ProgressStyle::with_template("[{elapsed_precise}] {msg}").unwrap();

    let (bars, _) = init_pb();
    let pb_scan = bars.get(&ProgressBarType::Scan).unwrap();
    let pb_hash = bars.get(&ProgressBarType::Hash).unwrap();
    let pb_hash_bar = bars.get(&ProgressBarType::HashBar).unwrap();
    let pb_cache_to_dupe = bars.get(&ProgressBarType::CacheToDupe).unwrap();
    let pb_cache_to_dupe_proc_bar = bars.get(&ProgressBarType::CacheToDupeBar).unwrap();
    let pb_db_dupe_file_proc_bar = bars.get(&ProgressBarType::Db).unwrap();

    let term = Term::stdout();

    for message in rx {
        update_stats(message.clone(), &stats);
        let stats = stats.lock().unwrap();
        match message {
            StatusMessage::ProcessBegin => {
                term.hide_cursor().unwrap();
            }
            StatusMessage::ScanBegin => {
                pb_scan.set_prefix("Scanning:");
                pb_scan.enable_steady_tick(Duration::from_millis(100));
            }
            StatusMessage::ScanAddRaw(msg) => {
                let message = format!(
                    "Scanned {} files, total size {} ({})",
                    style(HumanCount(stats.scan_file_count as u64))
                        .bold()
                        .green(),
                    style(HumanBytes(stats.scan_file_size)).bold().green(),
                    msg.file_path.display()
                );
                pb_scan.set_message(message);
            }
            StatusMessage::ScanAddDupe(_msg) => {}
            StatusMessage::ScanEnd => {
                let message = format!(
                    "Scanned {} files, total size {} in {}",
                    style(HumanCount(stats.scan_file_count as u64))
                        .bold()
                        .green(),
                    style(HumanBytes(stats.scan_file_size)).bold().green(),
                    HumanDuration(pb_scan.elapsed())
                );

                pb_scan.set_style(finish_style.clone());
                pb_scan.finish_with_message(message);
            }
            StatusMessage::HashBegin => {
                pb_hash.enable_steady_tick(Duration::from_millis(100));
                pb_hash_bar.set_length(stats.scan_dupe_file_size);
                pb_hash_bar.set_prefix("Hashing files:");
            }
            StatusMessage::HashProc(msg) => {
                let message = format!(
                    "Hashed {} files, file size {}, total count {}, total size {}",
                    msg.scan_file_proc_count,
                    msg.file_size,
                    stats.hash_proc_scan_file_count,
                    stats.hash_proc_scan_file_size
                );
                pb_hash.set_message(message);
                pb_hash_bar.set_position(stats.hash_proc_scan_file_size);
            }
            StatusMessage::HashGenCacheFile(_msg) => {
                // msg.canonical_path
                // stats.hash_proc_confirmed_dupe_count += msg.full_count;
            }

            StatusMessage::HashEnd => {
                pb_hash_bar.set_position(stats.hash_proc_scan_file_count as u64);
                pb_hash_bar.finish_and_clear();
                pb_hash.finish_with_message("Hashing files complete.");

                let message = format!(
                    "Hashed {} files, total size {} in {}",
                    style(HumanCount(stats.scan_file_count as u64))
                        .bold()
                        .green(),
                    style(HumanBytes(stats.scan_file_size)).bold().green(),
                    HumanDuration(pb_hash.elapsed())
                );

                pb_hash.set_style(finish_style.clone());
                pb_hash.finish_with_message(message);
            }
            StatusMessage::CacheToDupeBegin => {
                pb_cache_to_dupe.set_prefix("Cache to duping...");
                pb_cache_to_dupe_proc_bar.set_length(stats.hash_proc_confirmed_dupe_count as u64);
            }
            StatusMessage::CacheToDupeProc(msg) => {
                // stats.cache_to_dupe_file_count += msg.count;
                pb_cache_to_dupe_proc_bar.set_position(stats.cache_to_dupe_file_count as u64);
            }
            StatusMessage::CacheToDupeEnd => {
                // pb_cache_to_dupe.set_prefix("");
                pb_cache_to_dupe.set_style(finish_style.clone());
                pb_cache_to_dupe.finish_with_message("Dupe Files Vec created successfully");
                pb_cache_to_dupe_proc_bar.finish_and_clear();
            }
            StatusMessage::DbDupeFileInsertBegin => {
                pb_db_dupe_file_proc_bar.set_length(stats.cache_to_dupe_file_count as u64);
            }
            StatusMessage::DbDupeFileInsertProc(_msg) => {
                // stats.db_dupe_file_insert_count += msg.rows_inserted;
                pb_db_dupe_file_proc_bar.set_position(stats.db_dupe_file_insert_count as u64);
            }
            StatusMessage::DbDupeFileInsertEnd => {
                let msg = format!(
                    "{} rows inserted for dupe_files in db",
                    stats.db_dupe_file_insert_count
                );

                pb_db_dupe_file_proc_bar.set_style(finish_style.clone());
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
                println!("Done");
            }
        }
    }
}

fn update_stats(message: StatusMessage, stats: &Arc<Mutex<ProcessStats>>) {
    let mut stats = stats.lock().unwrap();
    match message {
        StatusMessage::ProcessBegin => {}
        StatusMessage::ScanBegin => {}
        StatusMessage::ScanAddRaw(msg) => {
            stats.scan_file_count += 1;
            stats.scan_file_size += msg.file_size;
        }
        StatusMessage::ScanAddDupe(msg) => {
            stats.scan_dupe_file_count += msg.count;
            stats.scan_dupe_file_size += (msg.count as u64) * msg.file_size;
        }
        StatusMessage::ScanEnd => {}
        StatusMessage::HashBegin => {}
        StatusMessage::HashProc(msg) => {
            stats.hash_proc_scan_file_count += msg.scan_file_proc_count;
            stats.hash_proc_scan_file_size += (msg.scan_file_proc_count as u64) * msg.file_size;
            stats.hash_full_cache_hit_count += msg.full_cache_hit_count;
            stats.hash_partial_cache_hit_count += msg.partial_cache_hit_count;
            stats.hash_proc_confirmed_dupe_count += msg.confirmed_dupe_count;
        }
        StatusMessage::HashGenCacheFile(msg) => {
            stats.hash_proc_confirmed_dupe_count += msg.full_count;
        }

        StatusMessage::HashEnd => {}
        StatusMessage::CacheToDupeBegin => {}
        StatusMessage::CacheToDupeProc(msg) => {
            stats.cache_to_dupe_file_count += msg.count;
        }
        StatusMessage::CacheToDupeEnd => {}
        StatusMessage::DbDupeFileInsertBegin => {}
        StatusMessage::DbDupeFileInsertProc(msg) => {
            stats.db_dupe_file_insert_count += msg.rows_inserted;
        }
        StatusMessage::DbDupeFileInsertEnd => {}

        StatusMessage::ProcessEnd => {}
    }
}
