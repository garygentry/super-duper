use console::{ style, Term };
use indicatif::MultiProgress;
use indicatif::{ HumanBytes, HumanCount, HumanDuration, ProgressBar, ProgressStyle };
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
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

#[derive(Hash, Eq, PartialEq)]
enum ProgressBarType {
    Scan,
    Hash,
    HashBar,
    CacheToDupe,
    Db,
}

fn init_pb() -> HashMap<ProgressBarType, ProgressBar> {
    let m = MultiProgress::new();
    let pb_scan = get_pb();
    let pb_hash = get_pb();
    let mb_bar = get_pb_bar();
    let pb_cache_to_dupe = get_pb();
    let pb_db = get_pb();

    let mut pb_map = HashMap::new();
    pb_map.insert(ProgressBarType::Scan, m.add(pb_scan));
    pb_map.insert(ProgressBarType::Hash, m.add(pb_hash));
    pb_map.insert(ProgressBarType::HashBar, m.add(mb_bar));
    pb_map.insert(ProgressBarType::CacheToDupe, m.add(pb_cache_to_dupe));
    pb_map.insert(ProgressBarType::Db, m.add(pb_db));

    // let bars = Arc::new(Mutex::new(pb_map));
    let bars = pb_map;

    bars
}

pub fn handle_status(rx: mpsc::Receiver<StatusMessage>, stats: Arc<Mutex<ProcessStats>>) {
    // let m = MultiProgress::new();

    // let pb_scan = m.add(get_pb());
    // let pb_hash = m.add(get_pb());
    // let mb_bar = m.add(get_pb_bar());
    // let pb_cache_to_dupe = m.add(get_pb());
    // let pb_db = m.add(get_pb());
    let bars = init_pb();

    let term = Term::stdout();
    let mut i = 0;
    let mut k = 0;

    for message in rx {
        i += 1;
        let mut _stats = stats.lock().unwrap();
        // let bars = bars.lock().unwrap();
        match message {
            StatusMessage::ProcessBegin => {
                term.hide_cursor().unwrap();
                // m.println("Processing...");
            }
            StatusMessage::ScanBegin => {
                let message = format!("ScanBegin: {}", i);
                let b = bars.get(&ProgressBarType::Scan).unwrap();
                b.set_prefix("Scanning...");
                b.set_message(message);
            }
            StatusMessage::ScanAddRaw(_msg) => {
                let message = format!("ScanAddRaw: {}", i);
                let b = bars.get(&ProgressBarType::Scan).unwrap();
                b.set_message(message);
            }
            StatusMessage::ScanAddDupe(_msg) => {
                let message = format!("ScanAddDupe: {}", i);
                let b = bars.get(&ProgressBarType::Scan).unwrap();
                b.set_message(message);
            }
            StatusMessage::ScanEnd => {
                let message = format!("ScanEnd: {}", i);
                let b = bars.get(&ProgressBarType::Scan).unwrap();
                b.set_message(message.to_string());
                println!("DONE");
                b.finish_with_message(message.to_string());
            }
            StatusMessage::HashBegin => {
                let message = format!("HashBegin: {}", i);
                let b = bars.get(&ProgressBarType::Hash).unwrap();
                b.set_prefix("Hashing...");
                b.set_message(message);
                b.set_length(100);
            }
            StatusMessage::HashProc(_msg) => {
                let message = format!("HashProc: {}", i);
                let b = bars.get(&ProgressBarType::Hash).unwrap();
                b.set_message(message);
                if k < 100 {
                    k += 1;
                    b.set_position(k);
                } else {
                    b.finish_and_clear();
                }
            }
            StatusMessage::HashGenCacheFile(_msg) => {
                let message = format!("HashGenCacheFile: {}", i);
                let b = bars.get(&ProgressBarType::Hash).unwrap();
                b.set_message(message);
            }

            StatusMessage::HashEnd => {
                let message = format!("HashEnd: {}", i);
                let b = bars.get(&ProgressBarType::Hash).unwrap();
                b.set_message(message.to_string());
                b.finish_with_message(message.to_string());
            }
            StatusMessage::CacheToDupeBegin => {
                let message = format!("CacheToDupeBegin: {}", i);
                let b = bars.get(&ProgressBarType::CacheToDupe).unwrap();
                b.set_prefix("Cache to duping...");
                b.set_message(message);
            }
            StatusMessage::CacheToDupeProc(_msg) => {
                let message = format!("CacheToDupeProc: {}", i);
                let b = bars.get(&ProgressBarType::CacheToDupe).unwrap();
                b.set_message(message);
            }
            StatusMessage::CacheToDupeEnd => {
                let message = format!("CacheToDupeEnd: {}", i);
                let b = bars.get(&ProgressBarType::CacheToDupe).unwrap();
                b.set_message(message.to_string());
                b.finish_with_message(message.to_string());
            }
            StatusMessage::DbDupeFileInsertBegin => {
                let message = format!("DbDupeFileInsertBegin: {}", i);
                let b = bars.get(&ProgressBarType::Db).unwrap();
                b.set_prefix("DB Inserting...");
                b.set_message(message);
            }
            StatusMessage::DbDupeFileInsertProc(_msg) => {
                let message = format!("DbDupeFileInsertProc: {}", i);
                let b = bars.get(&ProgressBarType::Db).unwrap();
                b.set_message(message);
            }
            StatusMessage::DbDupeFileInsertEnd => {
                let message = format!("DbDupeFileInsertEnd: {}", i);
                let b = bars.get(&ProgressBarType::Db).unwrap();
                b.set_message(message.to_string());
                b.finish_with_message(message.to_string());
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
