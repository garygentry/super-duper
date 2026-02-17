use crate::callbacks::FfiProgressBridge;
use crate::error::{map_core_error, set_last_error};
use crate::handle::{allocate_handle, destroy_handle, with_handle, EngineState};
use crate::types::*;
use std::ffi::c_char;
use std::sync::atomic::Ordering;
use super_duper_core::{AppConfig, ScanEngine, SilentReporter};
use super_duper_core::storage::Database;

/// Create a new engine instance. Returns a handle (u64) or 0 on failure.
///
/// # Safety
/// `db_path` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn sd_engine_create(db_path: *const c_char) -> u64 {
    let db_path_str = match c_string_to_rust(db_path) {
        Some(s) => s,
        None => "super_duper.db".to_string(),
    };

    let config = AppConfig {
        root_paths: Vec::new(),
        ignore_patterns: Vec::new(),
    };

    let engine = ScanEngine::new(config).with_db_path(&db_path_str);
    let cancel_token = engine.cancel_token();

    let db = match Database::open(&db_path_str) {
        Ok(db) => Some(db),
        Err(e) => {
            set_last_error(format!("Failed to open database: {}", e));
            return 0;
        }
    };

    let state = EngineState {
        engine,
        db,
        db_path: db_path_str,
        root_paths: Vec::new(),
        ignore_patterns: Vec::new(),
        is_scanning: false,
        cancel_token,
        progress_bridge: None,
    };

    allocate_handle(state)
}

/// Destroy an engine instance and free its resources.
#[no_mangle]
pub extern "C" fn sd_engine_destroy(handle: u64) -> SdResultCode {
    if destroy_handle(handle) {
        SdResultCode::Ok
    } else {
        set_last_error("Invalid handle".to_string());
        SdResultCode::InvalidHandle
    }
}

/// Set the scan paths for an engine instance.
///
/// # Safety
/// `paths` must be a valid array of `count` null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn sd_engine_set_scan_paths(
    handle: u64,
    paths: *const *const c_char,
    count: u32,
) -> SdResultCode {
    if paths.is_null() {
        set_last_error("paths is null".to_string());
        return SdResultCode::InvalidArgument;
    }

    let mut root_paths = Vec::new();
    for i in 0..count {
        let path_ptr = *paths.add(i as usize);
        match c_string_to_rust(path_ptr) {
            Some(s) => root_paths.push(s),
            None => {
                set_last_error(format!("Invalid path at index {}", i));
                return SdResultCode::InvalidArgument;
            }
        }
    }

    let result = with_handle(handle, |state| {
        state.root_paths = root_paths;
        let config = AppConfig {
            root_paths: state.root_paths.clone(),
            ignore_patterns: state.ignore_patterns.clone(),
        };
        state.engine = ScanEngine::new(config).with_db_path(&state.db_path);
        state.cancel_token = state.engine.cancel_token();
        SdResultCode::Ok
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Set ignore patterns for file scanning.
///
/// # Safety
/// `patterns` must be a valid array of `count` null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn sd_engine_set_ignore_patterns(
    handle: u64,
    patterns: *const *const c_char,
    count: u32,
) -> SdResultCode {
    if patterns.is_null() && count > 0 {
        set_last_error("patterns is null".to_string());
        return SdResultCode::InvalidArgument;
    }

    let mut ignore_patterns = Vec::new();
    for i in 0..count {
        let pattern_ptr = *patterns.add(i as usize);
        match c_string_to_rust(pattern_ptr) {
            Some(s) => ignore_patterns.push(s),
            None => {
                set_last_error(format!("Invalid pattern at index {}", i));
                return SdResultCode::InvalidArgument;
            }
        }
    }

    let result = with_handle(handle, |state| {
        state.ignore_patterns = ignore_patterns;
        let config = AppConfig {
            root_paths: state.root_paths.clone(),
            ignore_patterns: state.ignore_patterns.clone(),
        };
        state.engine = ScanEngine::new(config).with_db_path(&state.db_path);
        state.cancel_token = state.engine.cancel_token();
        SdResultCode::Ok
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Set a progress callback for scan operations.
#[no_mangle]
pub extern "C" fn sd_set_progress_callback(
    handle: u64,
    callback: SdProgressCallback,
) -> SdResultCode {
    let result = with_handle(handle, |state| {
        state.progress_bridge = Some(FfiProgressBridge::new(callback));
        SdResultCode::Ok
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Clear the progress callback.
#[no_mangle]
pub extern "C" fn sd_clear_progress_callback(handle: u64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        state.progress_bridge = None;
        SdResultCode::Ok
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Start a synchronous scan. Blocks until complete.
#[no_mangle]
pub extern "C" fn sd_scan_start(handle: u64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        if state.is_scanning {
            set_last_error("Scan already in progress".to_string());
            return SdResultCode::ScanInProgress;
        }

        state.is_scanning = true;
        let scan_result = if let Some(ref bridge) = state.progress_bridge {
            state.engine.scan(bridge)
        } else {
            state.engine.scan(&SilentReporter)
        };
        state.is_scanning = false;

        match scan_result {
            Ok(_) => SdResultCode::Ok,
            Err(e) => map_core_error(e),
        }
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Request cancellation of the current scan.
#[no_mangle]
pub extern "C" fn sd_scan_cancel(handle: u64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        state.cancel_token.store(true, Ordering::Relaxed);
        SdResultCode::Ok
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Check if a scan is currently running.
#[no_mangle]
pub extern "C" fn sd_scan_is_running(handle: u64) -> bool {
    with_handle(handle, |state| state.is_scanning).unwrap_or(false)
}

/// Mark a file for deletion.
#[no_mangle]
pub extern "C" fn sd_mark_file_for_deletion(handle: u64, file_id: i64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        let db = match &state.db {
            Some(db) => db,
            None => {
                set_last_error("No database open".to_string());
                return SdResultCode::DatabaseError;
            }
        };
        match db.mark_file_for_deletion(file_id, None) {
            Ok(()) => SdResultCode::Ok,
            Err(e) => {
                set_last_error(format!("Failed to mark file: {}", e));
                SdResultCode::DatabaseError
            }
        }
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Unmark a file from the deletion plan.
#[no_mangle]
pub extern "C" fn sd_unmark_file_for_deletion(handle: u64, file_id: i64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        let db = match &state.db {
            Some(db) => db,
            None => {
                set_last_error("No database open".to_string());
                return SdResultCode::DatabaseError;
            }
        };
        match db.unmark_file_for_deletion(file_id) {
            Ok(()) => SdResultCode::Ok,
            Err(e) => {
                set_last_error(format!("Failed to unmark file: {}", e));
                SdResultCode::DatabaseError
            }
        }
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Get deletion plan summary: (file_count, total_bytes).
///
/// # Safety
/// `out_count` and `out_bytes` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn sd_deletion_plan_summary(
    handle: u64,
    out_count: *mut i64,
    out_bytes: *mut i64,
) -> SdResultCode {
    if out_count.is_null() || out_bytes.is_null() {
        set_last_error("Output pointers are null".to_string());
        return SdResultCode::InvalidArgument;
    }

    let result = with_handle(handle, |state| {
        let db = match &state.db {
            Some(db) => db,
            None => {
                set_last_error("No database open".to_string());
                return SdResultCode::DatabaseError;
            }
        };
        match db.get_deletion_plan_summary() {
            Ok((count, bytes)) => {
                *out_count = count;
                *out_bytes = bytes;
                SdResultCode::Ok
            }
            Err(e) => {
                set_last_error(format!("Query error: {}", e));
                SdResultCode::DatabaseError
            }
        }
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Mark all files in a directory for deletion.
///
/// # Safety
/// `directory_path` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn sd_mark_directory_for_deletion(
    handle: u64,
    directory_path: *const c_char,
) -> SdResultCode {
    let path_str = match c_string_to_rust(directory_path) {
        Some(s) => s,
        None => {
            set_last_error("directory_path is null".to_string());
            return SdResultCode::InvalidArgument;
        }
    };

    let result = with_handle(handle, |state| {
        let db = match &state.db {
            Some(db) => db,
            None => {
                set_last_error("No database open".to_string());
                return SdResultCode::DatabaseError;
            }
        };
        match super_duper_core::analysis::deletion_plan::mark_directory_for_deletion(
            db, &path_str, None,
        ) {
            Ok(_) => SdResultCode::Ok,
            Err(e) => map_core_error(e),
        }
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Auto-mark duplicate files for deletion (keeps first alphabetically).
#[no_mangle]
pub extern "C" fn sd_auto_mark_for_deletion(handle: u64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        let db = match &state.db {
            Some(db) => db,
            None => {
                set_last_error("No database open".to_string());
                return SdResultCode::DatabaseError;
            }
        };
        match super_duper_core::analysis::deletion_plan::auto_mark_duplicates(db, Some("auto")) {
            Ok(_) => SdResultCode::Ok,
            Err(e) => map_core_error(e),
        }
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Execute the deletion plan. Returns success/error counts via out parameters.
///
/// # Safety
/// `out_result` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn sd_deletion_execute(
    handle: u64,
    out_result: *mut SdDeletionResult,
) -> SdResultCode {
    if out_result.is_null() {
        set_last_error("out_result is null".to_string());
        return SdResultCode::InvalidArgument;
    }

    let result = with_handle(handle, |state| {
        let db = match &state.db {
            Some(db) => db,
            None => {
                set_last_error("No database open".to_string());
                return SdResultCode::DatabaseError;
            }
        };
        match super_duper_core::analysis::deletion_plan::execute_deletion_plan(db) {
            Ok((success, errors)) => {
                *out_result = SdDeletionResult {
                    success_count: success as u32,
                    error_count: errors as u32,
                };
                SdResultCode::Ok
            }
            Err(e) => map_core_error(e),
        }
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}
