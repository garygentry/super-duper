use console::{style, Term};
use indicatif::MultiProgress;
use indicatif::{HumanBytes, HumanCount, HumanDuration, ProgressBar, ProgressStyle};
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
    pub hash_file_count: usize,
    pub hash_file_size: u64,
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
pub struct HashProcScanFilesStatusMessage {
    pub count: usize,
    pub file_size: u64,
}
#[derive(Clone, Debug)]
pub enum StatusMessage {
    ProcessBegin,
    ScanBegin,
    ScanAddRaw(ScanAddRawStatusMessage),
    ScanAddDupe(ScanAddDupeStatusMessage),
    ScanEnd,
    HashBegin,
    HashProcScanFiles(HashProcScanFilesStatusMessage),
    HashEnd,
    ProcessEnd,
}

lazy_static::lazy_static! {
  static ref PB_INSTANCE: Arc<Mutex<ProgressBar>> = {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(120));
    pb.set_style(
        ProgressStyle::with_template("{spinner:.blue} {msg}")
            .unwrap()
            // For more spinners check out the cli-spinners project:
            // https://github.com/sindresorhus/cli-spinners/blob/master/spinners.json
            .tick_strings(&[
                "▹▹▹▹▹",
                "▸▹▹▹▹",
                "▹▸▹▹▹",
                "▹▹▸▹▹",
                "▹▹▹▸▹",
                "▹▹▹▹▸",
                "▪▪▪▪▪",
            ]),
    );
    Arc::new(Mutex::new(pb))
  };
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

fn update_stats(pb: &ProgressBar, stats: ProcessStats) {
    let message = format!(
        "Scanned Files: {}, Scanned File Size: {}, Scan Dupe Count: {}, Scan Dupe Size: {}, Hashed Files: {}, Hashed File Size: {}",
        stats.scan_file_count,
        stats.scan_file_size,
        stats.scan_dupe_file_count,
        stats.scan_dupe_file_size,
        stats.hash_file_count,
        stats.hash_file_size
    );
    pb.set_message(message);
}

pub fn handle_status(rx: mpsc::Receiver<StatusMessage>, stats: Arc<Mutex<ProcessStats>>) {
    let m = MultiProgress::new();

    // let pb_stats = m.add(get_pb());
    let pb_scan = m.add(get_pb());
    let pb_hash = m.add(get_pb());
    let pb_hash_bar = m.add(get_pb_bar());
    let term = Term::stdout();

    for message in rx {
        let mut stats = stats.lock().unwrap();
        match message {
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
            StatusMessage::ScanBegin => {
                // pb_scan.set_prefix("Scanning...")
                // pb_scan.println("Scanning files...");
            }
            StatusMessage::ScanEnd => {
                let message = format!(
                    "Scanned {} files, total size {}",
                    stats.scan_file_count, stats.scan_file_size,
                );
                pb_scan.finish_with_message(message);
            }
            StatusMessage::HashBegin => {
                pb_hash_bar.set_length(stats.scan_dupe_file_size);
                pb_hash_bar.set_prefix("Hashing files:  ")
            }
            StatusMessage::HashProcScanFiles(msg) => {
                stats.hash_file_count += msg.count;
                stats.hash_file_size += msg.count as u64 * msg.file_size;
                let message = format!(
                    "Hashed {} files, file size {}, total count {}, total size {}",
                    msg.count, msg.file_size, stats.hash_file_count, stats.hash_file_size
                );
                pb_hash.tick();
                pb_hash.set_message(message);
                pb_hash_bar.set_position(stats.hash_file_size);
            }
            StatusMessage::HashEnd => {
                pb_hash_bar.set_position(stats.hash_file_count as u64);
                pb_hash.finish_with_message("Hashing files complete.");
            }
            StatusMessage::ProcessBegin => {
                term.hide_cursor().unwrap();
            }
            StatusMessage::ProcessEnd => {
                println!("ProcessEnd");
                let message = format!(
                    "ALL Scanned Files: {}, Scanned File Size: {}, Scan Dupe Count: {}, Scan Dupe Size: {}, Hashed Files: {}, Hashed File Size: {}",
                    stats.scan_file_count,
                    stats.scan_file_size,
                    stats.scan_dupe_file_count,
                    stats.scan_dupe_file_size,
                    stats.hash_file_count,
                    stats.hash_file_size
                );
                // pb_stats.finish_with_message(message);
                term.show_cursor().unwrap();
            }
        }
        // update_stats(&pb_stats, *stats);
    }
}
