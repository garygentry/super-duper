use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Mutex;
use super_duper_core::ProgressReporter;

/// CLI progress reporter using indicatif progress bars.
///
/// - Scan phase: spinner (unknown total files upfront)
/// - Hash phase: progress bar (total files known from scan)
/// - DB write phase: spinner
pub struct CliReporter {
    bar: Mutex<Option<ProgressBar>>,
}

impl CliReporter {
    pub fn new() -> Self {
        Self {
            bar: Mutex::new(None),
        }
    }

    fn set_bar(&self, pb: ProgressBar) {
        let mut guard = self.bar.lock().unwrap();
        if let Some(old) = guard.take() {
            old.finish_and_clear();
        }
        *guard = Some(pb);
    }

    fn finish_bar(&self) {
        let mut guard = self.bar.lock().unwrap();
        if let Some(pb) = guard.take() {
            pb.finish_and_clear();
        }
    }
}

impl ProgressReporter for CliReporter {
    fn on_scan_start(&self) {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        pb.set_message("Scanning files...");
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        self.set_bar(pb);
    }

    fn on_scan_progress(&self, files_found: usize, _current_path: &str) {
        let guard = self.bar.lock().unwrap();
        if let Some(pb) = guard.as_ref() {
            pb.set_message(format!("Scanning... {} files found", files_found));
        }
    }

    fn on_scan_complete(&self, total_files: usize, duration_secs: f64) {
        self.finish_bar();
        eprintln!(
            "  \x1b[32m✓\x1b[0m Scan complete: {} files in {:.2}s",
            total_files, duration_secs
        );
    }

    fn on_hash_start(&self) {
        // We don't know total yet — it'll be set on first on_hash_progress
        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::with_template(
                "  {spinner:.cyan} Hashing [{bar:30.cyan/dim}] {pos}/{len} files ({eta} remaining)",
            )
            .unwrap()
            .progress_chars("━╸─")
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        self.set_bar(pb);
    }

    fn on_hash_progress(&self, files_hashed: usize, total_files: usize) {
        let guard = self.bar.lock().unwrap();
        if let Some(pb) = guard.as_ref() {
            if pb.length() != Some(total_files as u64) {
                pb.set_length(total_files as u64);
            }
            pb.set_position(files_hashed as u64);
        }
    }

    fn on_hash_complete(&self, total_dupes: usize, duration_secs: f64) {
        self.finish_bar();
        eprintln!(
            "  \x1b[32m✓\x1b[0m Hash complete: {} duplicate groups in {:.2}s",
            total_dupes, duration_secs
        );
    }

    fn on_db_write_start(&self) {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        pb.set_message("Writing to database...");
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        self.set_bar(pb);
    }

    fn on_db_write_complete(&self, rows: usize, duration_secs: f64) {
        self.finish_bar();
        eprintln!(
            "  \x1b[32m✓\x1b[0m Database write complete: {} records in {:.2}s",
            rows, duration_secs
        );
    }

    fn on_dir_analysis_start(&self) {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        pb.set_message("Analyzing directory structure...");
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        self.set_bar(pb);
    }

    fn on_dir_analysis_complete(&self, fingerprints: usize, similarity_pairs: usize, duration_secs: f64) {
        self.finish_bar();
        eprintln!(
            "  \x1b[32m✓\x1b[0m Directory analysis complete: {} fingerprints, {} similar pairs in {:.2}s",
            fingerprints, similarity_pairs, duration_secs
        );
    }
}
