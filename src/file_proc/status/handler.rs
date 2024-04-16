use console::{ style, Term };
use indicatif::{ HumanBytes, HumanCount, HumanDuration, ProgressStyle };
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use super::types::*;

use super::progress_bars::FileProcStatusType::{ *, self };
use super::progress_bars::{ FileProcProgressBar, FileProcStatusBars };

pub fn handle_status(rx: mpsc::Receiver<StatusMessage>, stats: Arc<Mutex<FileProcStats>>) {
    let (bars, _) = FileProcStatusBars::new_progress_bars();

    let term = Term::stdout();

    for message in rx {
        update_stats(message.clone(), &stats);
        let stats = stats.lock().unwrap();
        match message {
            StatusMessage::ProcessBegin => {
                term.hide_cursor().unwrap();
            }
            StatusMessage::ScanBegin => {
                bars[Scan].set_prefix("Scanning:");
                bars[Scan].enable_steady_tick_default();
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
                bars[Scan].set_message(message);
            }
            StatusMessage::ScanAddDupe(_msg) => {}
            StatusMessage::ScanEnd => {
                let message = format!(
                    "Scanned {} files, total size {} in {}",
                    style(HumanCount(stats.scan_file_count as u64))
                        .bold()
                        .green(),
                    style(HumanBytes(stats.scan_file_size)).bold().green(),
                    HumanDuration(bars[Scan].elapsed())
                );

                bars[Scan].finish_with_finish_style(message);
            }
            StatusMessage::HashBegin => {
                bars[Hash].set_prefix("Hashing:");
                bars[Hash].enable_steady_tick_default();
                bars[HashBar].set_length(stats.scan_dupe_file_size);
            }
            StatusMessage::HashProc(msg) => {
                let message = format!(
                    "Hashed {} files, file size {}, total count {}, total size {}",
                    msg.scan_file_proc_count,
                    msg.file_size,
                    stats.hash_proc_scan_file_count,
                    stats.hash_proc_scan_file_size
                );
                bars[Hash].set_message(message);
                bars[HashBar].set_position(stats.hash_proc_scan_file_size);
            }
            StatusMessage::HashGenCacheFile(_msg) => {}

            StatusMessage::HashEnd => {
                let message = format!(
                    "Hashed {} files, total size {} in {}",
                    style(HumanCount(stats.scan_file_count as u64))
                        .bold()
                        .green(),
                    style(HumanBytes(stats.scan_file_size)).bold().green(),
                    HumanDuration(bars[Hash].elapsed())
                );

                bars[HashBar].finish_and_clear();
                bars[Hash].finish_with_finish_style(message);
            }
            StatusMessage::CacheToDupeBegin => {
                bars[CacheToDupe].set_prefix("Cache to duping...");
                bars[CacheToDupe].enable_steady_tick_default();
                bars[CacheToDupeBar].set_length(stats.hash_proc_confirmed_dupe_count as u64);
            }
            StatusMessage::CacheToDupeProc(_msg) => {
                bars[CacheToDupeBar].set_position(stats.cache_to_dupe_file_count as u64);
            }
            StatusMessage::CacheToDupeEnd => {
                bars[CacheToDupe].finish_with_finish_style("Dupe Files Vec created successfully");
                bars[CacheToDupeBar].finish_and_clear();
            }
            StatusMessage::DbDupeFileInsertBegin => {
                bars[DbDupeFile].set_length(stats.cache_to_dupe_file_count as u64);
            }
            StatusMessage::DbDupeFileInsertProc(_msg) => {
                bars[DbDupeFile].set_position(stats.db_dupe_file_insert_count as u64);
            }
            StatusMessage::DbDupeFileInsertEnd => {
                let message = format!(
                    "{} rows inserted for dupe_files in db",
                    stats.db_dupe_file_insert_count
                );

                bars[DbDupeFile].finish_with_finish_style(message);
            }

            StatusMessage::ProcessEnd => {
                term.show_cursor().unwrap();
                println!("Done");
            }
        }
    }
}

fn update_stats(message: StatusMessage, stats: &Arc<Mutex<FileProcStats>>) {
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
