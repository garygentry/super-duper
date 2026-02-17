/// Trait for reporting scan progress.
///
/// CLI implements with tracing/indicatif, FFI implements with C function pointer callbacks.
/// All methods have default no-op implementations.
pub trait ProgressReporter: Send + Sync {
    fn on_scan_start(&self) {}
    fn on_scan_progress(&self, _files_found: usize, _current_path: &str) {}
    fn on_scan_complete(&self, _total_files: usize, _duration_secs: f64) {}
    fn on_hash_start(&self) {}
    fn on_hash_progress(&self, _files_hashed: usize, _total_files: usize) {}
    fn on_hash_complete(&self, _total_dupes: usize, _duration_secs: f64) {}
    fn on_db_write_start(&self) {}
    fn on_db_write_complete(&self, _rows: usize, _duration_secs: f64) {}
}

/// No-op progress reporter for silent operation.
pub struct SilentReporter;

impl ProgressReporter for SilentReporter {}
