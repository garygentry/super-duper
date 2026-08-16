/// Trait for reporting scan progress.
///
/// CLI implements with tracing/indicatif, FFI implements with C function pointer callbacks.
/// All methods have default no-op implementations.
pub trait ProgressReporter: Send + Sync {
    fn on_scan_start(&self) {}
    fn on_scan_progress(&self, _files_found: usize, _current_path: &str) {}
    fn on_discovery_progress(
        &self,
        files_found: usize,
        _bytes_found: u64,
        _warning_count: usize,
        current_path: &str,
    ) {
        self.on_scan_progress(files_found, current_path);
    }
    fn on_scan_complete(&self, _total_files: usize, _duration_secs: f64) {}
    fn on_hash_start(&self) {}
    fn on_hash_progress(&self, _files_hashed: usize, _total_files: usize) {}
    fn on_hash_progress_detailed(
        &self,
        files_hashed: usize,
        total_files: usize,
        _warning_count: usize,
        _current_path: Option<&str>,
    ) {
        self.on_hash_progress(files_hashed, total_files);
    }
    fn on_hash_complete(&self, _total_dupes: usize, _duration_secs: f64) {}
    fn on_db_write_start(&self) {}
    fn on_db_write_progress(&self, _rows: usize, _total_rows: usize) {}
    fn on_db_write_complete(&self, _rows: usize, _duration_secs: f64) {}
    fn on_dir_analysis_start(&self) {}
    fn on_dir_analysis_progress(&self, _completed: usize, _total: usize) {}
    fn on_dir_analysis_complete(
        &self,
        _fingerprints: usize,
        _similarity_pairs: usize,
        _duration_secs: f64,
    ) {
    }
    fn on_finalizing(&self) {}
    fn on_finalizing_complete(&self, _duration_secs: f64) {}
}

/// No-op progress reporter for silent operation.
pub struct SilentReporter;

impl ProgressReporter for SilentReporter {}
