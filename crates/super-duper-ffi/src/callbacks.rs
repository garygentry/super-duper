use crate::types::SdProgressCallback;
use std::ffi::CString;
use super_duper_core::ProgressReporter;

/// FFI progress bridge that implements ProgressReporter by forwarding to a C callback.
pub struct FfiProgressBridge {
    callback: SdProgressCallback,
}

// Safety: The C callback function pointer is a static function that is safe to call from any thread.
unsafe impl Send for FfiProgressBridge {}
unsafe impl Sync for FfiProgressBridge {}

impl FfiProgressBridge {
    pub fn new(callback: SdProgressCallback) -> Self {
        Self { callback }
    }

    fn fire(&self, phase: u32, current: u64, total: u64, message: &str) {
        let c_msg = CString::new(message).unwrap_or_else(|_| CString::new("").unwrap());
        (self.callback)(phase, current, total, c_msg.as_ptr());
    }
}

impl ProgressReporter for FfiProgressBridge {
    fn on_scan_start(&self) {
        self.fire(0, 0, 0, "scan_start");
    }

    fn on_scan_progress(&self, files_found: usize, current_path: &str) {
        self.fire(0, files_found as u64, 0, current_path);
    }

    fn on_scan_complete(&self, total_files: usize, _duration_secs: f64) {
        self.fire(0, total_files as u64, total_files as u64, "scan_complete");
    }

    fn on_hash_start(&self) {
        self.fire(1, 0, 0, "hash_start");
    }

    fn on_hash_progress(&self, files_hashed: usize, total_files: usize) {
        self.fire(1, files_hashed as u64, total_files as u64, "");
    }

    fn on_hash_complete(&self, total_dupes: usize, _duration_secs: f64) {
        self.fire(1, total_dupes as u64, total_dupes as u64, "hash_complete");
    }

    fn on_db_write_start(&self) {
        self.fire(2, 0, 0, "db_write_start");
    }

    fn on_db_write_complete(&self, rows: usize, _duration_secs: f64) {
        self.fire(2, rows as u64, rows as u64, "db_write_complete");
    }
}
