fn get_pb() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    let spinner_style = ProgressStyle::with_template(
        "[{elapsed_precise}] {prefix:.bold.dim} {spinner} {wide_msg}"
    )
        .unwrap()
        .tick_strings(&[".  ", ".. ", "...", " ..", "  .", "   "]);

    pb.set_style(spinner_style);
    pb
}

fn get_pb_bar() -> ProgressBar {
    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::with_template(
            &format!("{{prefix:.bold}}▕{{bar:.{}}}▏{{msg}}", String::from("blue"))
        )
            .unwrap()
            .progress_chars("█▓▒░  ")
    );
    pb
}

pub fn handle_status(rx: mpsc::Receiver<StatusMessage>, stats: Arc<Mutex<ProcessStats>>) {
    let m = MultiProgress::new();

    let pb_scan = m.add(get_pb());
    let pb_hash = m.add(get_pb());
    let mb_bar = m.add(get_pb_bar());
    let pb_cache_to_dupe = m.add(get_pb());
    let pb_db = m.add(get_pb());

    let term = Term::stdout();
    let mut i = 0;
    let mut k = 0;

    let pb_scan_thread = thread::spawn(move || {
        for message in rx {
            i += 1;
            let mut _stats = stats.lock().unwrap();
            match message {
                StatusMessage::ScanBegin => {
                    let message = format!("ScanBegin: {}", i);
                    pb_scan.set_prefix("Scanning...");
                    pb_scan.set_message(message);
                }
                StatusMessage::ScanAddRaw(_msg) => {
                    let message = format!("ScanAddRaw: {}", i);
                    pb_scan.set_message(message);
                }
                StatusMessage::ScanAddDupe(_msg) => {
                    let message = format!("ScanAddDupe: {}", i);
                    pb_scan.set_message(message);
                }
                StatusMessage::ScanEnd => {
                    let message = format!("ScanEnd: {}", i);
                    pb_scan.set_message(message.to_string());
                    pb_scan.finish_with_message(message.to_string());
                }
                _ => {}
            }
        }
    });

    let pb_hash_thread = thread::spawn(move || {
        for message in rx {
            i += 1;
            let mut _stats = stats.lock().unwrap();
            match message {
                StatusMessage::HashBegin => {
                    let message = format!("HashBegin: {}", i);
                    pb_hash.set_prefix("Hashing...");
                    pb_hash.set_message(message);
                    mb_bar.set_length(100);
                }
                StatusMessage::HashProc(_msg) => {
                    let message = format!("HashProc: {}", i);
                    pb_hash.set_message(message);
                    if k < 100 {
                        k += 1;
                        mb_bar.set_position(k);
                    } else {
                        mb_bar.finish_and_clear();
                    }
                }
                StatusMessage::HashGenCacheFile(_msg) => {
                    let message = format!("HashGenCacheFile: {}", i);
                    pb_hash.set_message(message);
                }
                StatusMessage::HashEnd => {
                    let message = format!("HashEnd: {}", i);
                    pb_hash.set_message(message.to_string());
                    pb_hash.finish_with_message(message.to_string());
                }
                _ => {}
            }
        }
    });

    let pb_cache_to_dupe_thread = thread::spawn(move || {
        for message in rx {
            i += 1;
            let mut _stats = stats.lock().unwrap();
            match message {
                StatusMessage::CacheToDupeBegin => {
                    let message = format!("CacheToDupeBegin: {}", i);
                    pb_cache_to_dupe.set_prefix("Cache to duping...");
                    pb_cache_to_dupe.set_message(message);
                }
                StatusMessage::CacheToDupeProc(_msg) => {
                    let message = format!("CacheToDupeProc: {}", i);
                    pb_cache_to_dupe.set_message(message);
                }
                StatusMessage::CacheToDupeEnd => {
                    let message = format!("CacheToDupeEnd: {}", i);
                    pb_cache_to_dupe.set_message(message.to_string());
                    pb_cache_to_dupe.finish_with_message(message.to_string());
                }
                _ => {}
            }
        }
    });

    let pb_db_thread = thread::spawn(move || {
        for message in rx {
            i += 1;
            let mut _stats = stats.lock().unwrap();
            match message {
                StatusMessage::DbDupeFileInsertBegin => {
                    let message = format!("DbDupeFileInsertBegin: {}", i);
                    pb_db.set_prefix("DB Inserting...");
                    pb_db.set_message(message);
                }
                StatusMessage::DbDupeFileInsertProc(_msg) => {
                    let message = format!("DbDupeFileInsertProc: {}", i);
                    pb_db.set_message(message);
                }
                StatusMessage::DbDupeFileInsertEnd => {
                    let message = format!("DbDupeFileInsertEnd: {}", i);
                    pb_db.set_message(message.to_string());
                    pb_db.finish_with_message(message.to_string());
                }
                _ => {}
            }
        }
    });

    pb_scan_thread.join().unwrap();
    pb_hash_thread.join().unwrap();
    pb_cache_to_dupe_thread.join().unwrap();
    pb_db_thread.join().unwrap();

    term.show_cursor().unwrap();
}
use indicatif::MultiProgress;
use indicatif::{ HumanBytes, HumanCount, HumanDuration, ProgressBar, ProgressStyle };
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use console::{ style, Term };
use std::thread;

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
        "[{elapsed_precise}] {prefix:.bold.dim} {spinner} {wide_msg}"
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
            &format!("{{prefix:.bold}}▕{{bar:.{}}}▏{{msg}}", String::from("blue"))
        )
            .unwrap()
            .progress_chars("█▓▒░  ")
    );
    pb
}

pub fn handle_status(rx: mpsc::Receiver<StatusMessage>, stats: Arc<Mutex<ProcessStats>>) {
    let m = MultiProgress::new();

    let pb_scan = m.add(get_pb());
    let pb_hash = m.add(get_pb());
    let mb_bar = m.add(get_pb_bar());
    let pb_cache_to_dupe = m.add(get_pb());
    let pb_db = m.add(get_pb());

    let term = Term::stdout();
    let mut i = 0;
    let mut k = 0;

    for message in rx {
        i += 1;
        let mut _stats = stats.lock().unwrap();
        match message {
            StatusMessage::ProcessBegin => {
                term.hide_cursor().unwrap();
                m.println("Processing...");
            }
            StatusMessage::ScanBegin => {
                let message = format!("ScanBegin: {}", i);
                pb_scan.set_prefix("Scanning...");
                pb_scan.set_message(message);
            }
            StatusMessage::ScanAddRaw(_msg) => {
                let message = format!("ScanAddRaw: {}", i);
                pb_scan.set_message(message);
            }
            StatusMessage::ScanAddDupe(_msg) => {
                let message = format!("ScanAddDupe: {}", i);
                pb_scan.set_message(message);
            }
            StatusMessage::ScanEnd => {
                let message = format!("ScanEnd: {}", i);
                pb_scan.set_message(message.to_string());
                pb_scan.finish_with_message(message.to_string());
            }
            StatusMessage::HashBegin => {
                let message = format!("HashBegin: {}", i);
                pb_hash.set_prefix("Hashing...");
                pb_hash.set_message(message);
                mb_bar.set_length(100);
            }
            StatusMessage::HashProc(_msg) => {
                let message = format!("HashProc: {}", i);
                pb_hash.set_message(message);
                if k < 100 {
                    k += 1;
                    mb_bar.set_position(k);
                } else {
                    mb_bar.finish_and_clear();
                }
            }
            StatusMessage::HashGenCacheFile(_msg) => {
                let message = format!("HashGenCacheFile: {}", i);
                pb_hash.set_message(message);
            }

            StatusMessage::HashEnd => {
                let message = format!("HashEnd: {}", i);
                pb_hash.set_message(message.to_string());
                pb_hash.finish_with_message(message.to_string());
            }
            StatusMessage::CacheToDupeBegin => {
                let message = format!("CacheToDupeBegin: {}", i);
                pb_cache_to_dupe.set_prefix("Cache to duping...");
                pb_cache_to_dupe.set_message(message);
            }
            StatusMessage::CacheToDupeProc(_msg) => {
                let message = format!("CacheToDupeProc: {}", i);
                pb_cache_to_dupe.set_message(message);
            }
            StatusMessage::CacheToDupeEnd => {
                let message = format!("CacheToDupeEnd: {}", i);
                pb_cache_to_dupe.set_message(message.to_string());
                pb_cache_to_dupe.finish_with_message(message.to_string());
            }
            StatusMessage::DbDupeFileInsertBegin => {
                let message = format!("DbDupeFileInsertBegin: {}", i);
                pb_db.set_prefix("DB Inserting...");
                pb_db.set_message(message);
            }
            StatusMessage::DbDupeFileInsertProc(_msg) => {
                let message = format!("DbDupeFileInsertProc: {}", i);
                pb_db.set_message(message);
            }
            StatusMessage::DbDupeFileInsertEnd => {
                let message = format!("DbDupeFileInsertEnd: {}", i);
                pb_db.set_message(message.to_string());
                pb_db.finish_with_message(message.to_string());
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
