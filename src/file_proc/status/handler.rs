use console::Term;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use std::time::SystemTime;
use crate::file_proc::stats::FileProcStats;
use crate::utils::stats::StatsTimer;
use super::types::*;
use crate::AppConfig;

use super::progress_bars::{ *, FileProcStatusType::* };

pub fn handle_status(
    rx: mpsc::Receiver<StatusMessage>,
    stats: Arc<Mutex<FileProcStats>>,
    app_config: AppConfig
) {
    let (bars, _) = FileProcStatusBars::new_progress_bars(app_config);

    let term = Term::stdout();

    for message in rx {
        update_stats(message.clone(), &stats);
        let stats = stats.lock().unwrap();

        match message {
            StatusMessage::ProcessStart(msg) => {
                term.hide_cursor().unwrap();
                println!("Processing: {}", msg.input_paths.join(", "));
            }
            StatusMessage::ScanStart(_msg) => {
                bars[Scan].set_prefix("Scanning...");
                bars[Scan].enable_steady_tick_default();
            }
            StatusMessage::ScanAddInputFile(msg) => {
                let pb = &bars[Scan];

                let message = format!(
                    "{} files ({}) ({})",
                    pb.to_count_style(stats.scan_file_count as u64),
                    pb.to_bytes_style(stats.scan_file_size),
                    msg.file_path.display()
                );
                bars[Scan].set_message(message);
            }
            StatusMessage::ScanAddRetainedFile(_msg) => {}
            StatusMessage::ScanFinish => {
                let message = format!(
                    "Scanned {} files ({}), {} possible dupes ({}) in {}",
                    to_count_style(stats.scan_file_count as u64),
                    to_bytes_style(stats.scan_file_size),
                    to_count_style(stats.scan_size_dupe_file_count as u64),
                    to_bytes_style(stats.scan_size_dupe_file_size),
                    // to_duration_style(bars[Scan].elapsed())
                    to_duration_style(stats.scan_timer.get_duration())
                );

                bars[Scan].finish_with_finish_style(message);
            }
            StatusMessage::HashStart => {
                bars[Hash].enable_steady_tick_default();
                bars[HashBar].set_length(
                    stats.scan_size_dupe_file_count as u64
                );
            }
            StatusMessage::HashProc(_) => {
                let message = format!(
                    "Hashing: {} files ({}) Confirmed Dupes: {}, Cache hit/miss: full={}/{} partial={}/{}",
                    to_bytes_style(stats.hash_scan_file_count as u64),
                    to_bytes_style(stats.hash_scan_file_size),
                    to_count_style(stats.hash_confirmed_dupe_count as u64),
                    to_count_style(stats.hash_cache_hit_full_count as u64),
                    to_count_style(stats.hash_gen_full_count as u64),
                    to_count_style(stats.hash_cache_hit_partial_count as u64),
                    to_count_style(stats.hash_gen_partial_count as u64)
                );

                bars[Hash].set_message(message);
                bars[HashBar].set_message("");
                bars[HashBar].set_position(stats.hash_scan_file_count as u64);
            }

            StatusMessage::HashStartGenerateFileHash(msg) => {
                let message = format!(
                    "Generating {} hash for {}",
                    match msg.hash_type {
                        HashGenerateType::Full => "Full",
                        HashGenerateType::Partial => "Partial",
                    },
                    msg.canonical_path
                );
                bars[HashBar].set_message(message);
            }
            StatusMessage::HashFinishGenerateFileHash(msg) => {
                let message = format!(
                    "Generated {} hash for {}",
                    match msg.hash_type {
                        HashGenerateType::Full => "Full",
                        HashGenerateType::Partial => "Partial",
                    },
                    msg.canonical_path
                );
                bars[HashBar].set_message(message);
            }

            StatusMessage::HashFinish => {
                let message = format!(
                    "Hashed {} files ({}), {} confirmed Dupes - {} ({}) distinct dupe files in {}",
                    to_count_style(stats.hash_scan_file_count as u64),
                    to_bytes_style(stats.hash_scan_file_size),
                    to_count_style(stats.hash_confirmed_dupe_count as u64),
                    to_count_style(
                        stats.hash_confirmed_dupe_distinct_count as u64
                    ),
                    to_bytes_style(stats.hash_confirmed_dupe_distinct_size),
                    to_duration_style(stats.hash_timer.get_duration())
                );

                bars[HashBar].finish_and_clear();
                bars[Hash].finish_with_finish_style(message);
            }
            StatusMessage::CacheToDupeStart => {
                bars[CacheToDupe].set_prefix("Cache to duping...");
                bars[CacheToDupe].enable_steady_tick_default();
                bars[CacheToDupeBar].set_length(
                    stats.hash_confirmed_dupe_count as u64
                );
                bars[CacheToDupeBar].set_prefix(
                    "Converting duplicate hash map to vector for database.."
                );
            }
            StatusMessage::CacheToDupeProc(_msg) => {
                bars[CacheToDupeBar].set_position(
                    stats.cache_map_to_dupe_vec_count as u64
                );
            }
            StatusMessage::CacheToDupeFinish => {
                bars[CacheToDupe].finish_and_clear();
                bars[CacheToDupeBar].finish_and_clear();
            }
            // StatusMessage::DbDupeFileInsertStart => {
            //     bars[DbDupeFile].set_length(stats.cache_map_to_dupe_vec_count as u64);
            // }
            // StatusMessage::DbDupeFileInsertProc(_msg) => {
            //     bars[DbDupeFile].set_position(stats.db_dupe_file_insert_count as u64);
            // }
            // StatusMessage::DbDupeFileInsertFinish => {
            //     let message = format!(
            //         "Inserted {} rows into dupe_file table",
            //         to_count_style(stats.db_dupe_file_insert_count as u64)
            //     );

            //     bars[DbDupeFile].finish_with_finish_style(message);
            // }

            StatusMessage::ProcessFinish => {
                term.show_cursor().unwrap();
                let message = format!(
                    "Process complete in {}",
                    to_duration_style(stats.process_timer.get_duration())
                );
                println!("{}", message);

                // let x = stats.process_timer.get_duration();
                // println!("Done (get_duration): {:?}", stats.process_timer.get_duration());
                // println!("Done (get_duration_secs): {:?}", stats.process_timer.get_duration_secs());
                // println!(
                //     "Done (get_duration_human): {:?}",
                //     stats.process_timer.get_duration_human().to_string()
                // );
                // println!(
                //     "Done (get_duration_string): {:?}",
                //     stats.process_timer.get_duration_string()
                // );
            }
        }
    }
}

fn update_stats(message: StatusMessage, stats: &Arc<Mutex<FileProcStats>>) {
    let mut stats = stats.lock().unwrap();
    match message {
        StatusMessage::ProcessStart(_msg) => {
            stats.run_start_time = Some(SystemTime::now());
            stats.process_start = Some(Instant::now());
            stats.process_timer = StatsTimer::new();
        }
        StatusMessage::ScanStart(msg) => {
            stats.scan_start = Some(Instant::now());
            stats.scan_timer = StatsTimer::new();
            stats.scan_input_paths = msg.input_paths.clone();
        }
        StatusMessage::ScanAddInputFile(msg) => {
            stats.scan_file_count += 1;
            stats.scan_file_size += msg.file_size;
        }
        StatusMessage::ScanAddRetainedFile(msg) => {
            stats.scan_size_dupe_file_count += msg.count;
            stats.scan_size_dupe_file_size +=
                (msg.count as u64) * msg.file_size;
        }
        StatusMessage::ScanFinish => {
            stats.scan_finish = Some(Instant::now());
            stats.scan_timer.finish();
        }
        StatusMessage::HashStart => {
            stats.hash_start = Some(Instant::now());
            stats.hash_timer = StatsTimer::new();
        }
        StatusMessage::HashProc(msg) => {
            stats.hash_scan_file_count += msg.scan_file_count;
            stats.hash_scan_file_size +=
                (msg.scan_file_count as u64) * msg.file_size;
            stats.hash_cache_hit_full_count += msg.cache_hit_full_count;
            stats.hash_cache_hit_partial_count += msg.cache_hit_partial_count;
            stats.hash_confirmed_dupe_count += msg.confirmed_dupe_count;
            stats.hash_confirmed_dupe_size +=
                (msg.confirmed_dupe_count as u64) * msg.file_size;

            // Assumes all dupes are grouped when this is called  (believe this _should_ be true...)
            if msg.confirmed_dupe_count > 0 {
                stats.hash_confirmed_dupe_distinct_count += 1;
                stats.hash_confirmed_dupe_distinct_size += msg.file_size;
            }
        }
        StatusMessage::HashStartGenerateFileHash(_) => {
            // stats.hash_gen_full_count += msg.full_count;
            // stats.hash_gen_partial_count += msg.partial_count;
        }
        StatusMessage::HashFinishGenerateFileHash(msg) => {
            match msg.hash_type {
                HashGenerateType::Full => {
                    stats.hash_gen_full_count += 1;
                    stats.hash_gen_full_duration += msg.duration;
                    stats.hash_gen_full_file_size += msg.file_size;
                }
                HashGenerateType::Partial => {
                    stats.hash_gen_partial_count += 1;
                    stats.hash_gen_partial_duration += msg.duration;
                    stats.hash_gen_partial_file_size += msg.file_size;
                }
            }
        }
        StatusMessage::HashFinish => {
            stats.hash_finish = Some(Instant::now());
            stats.hash_timer.finish();
        }
        StatusMessage::CacheToDupeStart => {
            stats.cache_map_to_dupe_vec_start = Some(Instant::now());
        }
        StatusMessage::CacheToDupeProc(msg) => {
            stats.cache_map_to_dupe_vec_count += msg.count;
        }
        StatusMessage::CacheToDupeFinish => {
            stats.cache_map_to_dupe_vec_finish = Some(Instant::now());
        }
        // StatusMessage::DbDupeFileInsertStart => {
        //     stats.db_dupe_file_insert_start = Some(Instant::now());
        // }
        // StatusMessage::DbDupeFileInsertProc(msg) => {
        //     stats.db_dupe_file_insert_count += msg.rows_inserted;
        // }
        // StatusMessage::DbDupeFileInsertFinish => {
        //     stats.db_dupe_file_insert_finish = Some(Instant::now());
        // }

        StatusMessage::ProcessFinish => {
            stats.process_finish = Some(Instant::now());
            stats.process_timer.finish();
        }
    }
}
