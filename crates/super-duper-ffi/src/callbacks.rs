use crate::types::SdProgressCallback;
use std::ffi::c_char;

/// Stores the current progress callback for the active scan.
/// Thread-safety: only set/cleared from the main thread; called from worker threads.
static mut PROGRESS_CALLBACK: Option<SdProgressCallback> = None;

/// Set the progress callback. Must be called before starting a scan.
///
/// # Safety
/// Must only be called from the main thread.
pub unsafe fn set_progress_callback(cb: Option<SdProgressCallback>) {
    PROGRESS_CALLBACK = cb;
}

/// Fire the progress callback if one is set.
///
/// # Safety
/// The callback pointer must still be valid.
pub unsafe fn fire_progress(phase: u32, current: u64, total: u64, message: *const c_char) {
    if let Some(cb) = PROGRESS_CALLBACK {
        cb(phase, current, total, message);
    }
}
