use crate::callbacks::FfiProgressBridge;
use crate::error::{map_core_error, set_last_error};
use crate::handle::{EngineState, allocate_handle, destroy_handle, with_handle};
use crate::types::*;
use std::ffi::c_char;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use super_duper_core::storage::Database;
use super_duper_core::{AppConfig, ScanEngine, SilentReporter};

/// Create a new engine instance. Returns a handle (u64) or 0 on failure.
///
/// # Safety
/// `db_path` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sd_engine_create(db_path: *const c_char) -> u64 {
    unsafe {
        let db_path_str = match c_string_to_rust(db_path) {
            Some(s) => s,
            None => "super_duper.db".to_string(),
        };

        let config = AppConfig {
            root_paths: Vec::new(),
            ignore_patterns: Vec::new(),
            ..Default::default()
        };

        let engine = Arc::new(ScanEngine::new(config).with_db_path(&db_path_str));
        let cancel_token = engine.cancel_token();

        let db = match Database::open(&db_path_str) {
            Ok(db) => db,
            Err(e) => {
                set_last_error(format!("Failed to open database: {}", e));
                return 0;
            }
        };

        // Preserve the legacy FFI selector semantics by selecting the latest completed run.
        let active_session_id = db.get_latest_completed_run_id().unwrap_or(None);

        let state = EngineState {
            engine,
            db: Some(db),
            db_path: db_path_str,
            root_paths: Vec::new(),
            ignore_patterns: Vec::new(),
            is_scanning: false,
            cancel_token,
            progress_bridge: None,
            active_session_id,
            async_scan: None,
        };

        allocate_handle(state)
    }
}

/// Destroy an engine instance and free its resources.
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sd_engine_set_scan_paths(
    handle: u64,
    paths: *const *const c_char,
    count: u32,
) -> SdResultCode {
    unsafe {
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
                ..Default::default()
            };
            state.engine = Arc::new(ScanEngine::new(config).with_db_path(&state.db_path));
            state.cancel_token = state.engine.cancel_token();
            SdResultCode::Ok
        });

        result.unwrap_or(SdResultCode::InvalidHandle)
    }
}

/// Set ignore patterns for file scanning.
///
/// # Safety
/// `patterns` must be a valid array of `count` null-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sd_engine_set_ignore_patterns(
    handle: u64,
    patterns: *const *const c_char,
    count: u32,
) -> SdResultCode {
    unsafe {
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
                ..Default::default()
            };
            state.engine = Arc::new(ScanEngine::new(config).with_db_path(&state.db_path));
            state.cancel_token = state.engine.cancel_token();
            SdResultCode::Ok
        });

        result.unwrap_or(SdResultCode::InvalidHandle)
    }
}

/// Set a progress callback for scan operations.
#[unsafe(no_mangle)]
pub extern "C" fn sd_set_progress_callback(
    handle: u64,
    callback: SdProgressCallback,
) -> SdResultCode {
    let result = with_handle(handle, |state| {
        state.progress_bridge = Some(Arc::new(FfiProgressBridge::new(callback)));
        SdResultCode::Ok
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Clear the progress callback.
#[unsafe(no_mangle)]
pub extern "C" fn sd_clear_progress_callback(handle: u64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        state.progress_bridge = None;
        SdResultCode::Ok
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Start a synchronous scan. Blocks until complete.
#[unsafe(no_mangle)]
pub extern "C" fn sd_scan_start(handle: u64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        if state.is_scanning {
            set_last_error("Scan already in progress".to_string());
            return SdResultCode::ScanInProgress;
        }

        state.is_scanning = true;
        let scan_result = match state.progress_bridge.as_deref() {
            Some(bridge) => state.engine.scan(bridge),
            None => state.engine.scan(&SilentReporter),
        };
        state.is_scanning = false;

        match scan_result {
            Ok(result) => {
                state.active_session_id = Some(result.run_id);
                SdResultCode::Ok
            }
            Err(e) => map_core_error(e),
        }
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Start a scan on a background thread and return immediately. Only one scan (sync or async) may
/// run per handle at a time; the usual `SdResultCode::ScanInProgress` applies. Poll with
/// `sd_scan_observe`, or block the calling thread (without holding the handle) with
/// `sd_scan_join`. Cancel with `sd_scan_cancel`, same as a synchronous scan.
///
/// # Thread safety
/// The scan itself, and the progress callback set by `sd_set_progress_callback` (if any), run on a
/// dedicated background thread — not the caller's. The callback must be safe to call from any
/// thread; it already must be, since it also fires from the scan thread `sd_scan_start` blocks on.
/// Every other FFI call for this handle, including `sd_scan_cancel` and every query, remains safe
/// to call concurrently while the scan runs. `sd_scan_observe`/`sd_scan_join` are not safe to call
/// concurrently with each other for the same handle; drive a given handle's scan from one thread.
///
/// If the handle is destroyed with `sd_engine_destroy` while the scan is still running, the scan
/// keeps running to completion in the background (its callback may keep firing) and its result is
/// then silently discarded, since there is no longer a handle to report it to.
#[unsafe(no_mangle)]
pub extern "C" fn sd_scan_start_async(handle: u64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        if state.is_scanning {
            set_last_error("Scan already in progress".to_string());
            return SdResultCode::ScanInProgress;
        }

        let engine = Arc::clone(&state.engine);
        let bridge = state.progress_bridge.clone();
        let outcome = Arc::new(std::sync::Mutex::new(None));
        let outcome_for_thread = Arc::clone(&outcome);
        let thread = std::thread::spawn(move || {
            let scan_result = match bridge.as_deref() {
                Some(bridge) => engine.scan(bridge),
                None => engine.scan(&SilentReporter),
            };
            *outcome_for_thread
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(scan_result);
        });

        state.is_scanning = true;
        state.async_scan = Some(crate::handle::AsyncScan {
            thread: Some(thread),
            outcome,
        });
        SdResultCode::Ok
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// If the async scan for this handle has produced an outcome, join its thread, clear
/// `async_scan`, update `is_scanning`/`active_session_id`, and map the outcome to an
/// `SdResultCode` — calling `set_last_error` on the CALLING thread for an error, matching
/// `sd_scan_start`. Returns `None` if there is no async scan, or it hasn't finished yet.
fn take_finished_async_scan(state: &mut EngineState) -> Option<SdResultCode> {
    let ready = state
        .async_scan
        .as_ref()?
        .outcome
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .is_some();
    if !ready {
        return None;
    }
    let mut async_scan = state.async_scan.take().expect("checked Some above");
    let outcome = async_scan
        .outcome
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
        .expect("checked ready above");
    if let Some(thread) = async_scan.thread.take() {
        let _ = thread.join();
    }
    state.is_scanning = false;
    Some(match outcome {
        Ok(result) => {
            state.active_session_id = Some(result.run_id);
            SdResultCode::Ok
        }
        Err(e) => map_core_error(e),
    })
}

/// Non-blocking check of a scan started with `sd_scan_start_async`. Always returns `Ok` itself
/// (unless `handle`/`out_status` are invalid); the scan's own outcome is reported once, through
/// `out_status` and this call's return code together, per `SdScanStatus`'s doc comment.
///
/// # Safety
/// `out_status` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sd_scan_observe(
    handle: u64,
    out_status: *mut SdScanStatus,
) -> SdResultCode {
    unsafe {
        if out_status.is_null() {
            set_last_error("out_status is null".to_string());
            return SdResultCode::InvalidArgument;
        }

        let result = with_handle(handle, |state| {
            if state.async_scan.is_none() {
                *out_status = SdScanStatus::Idle;
                return SdResultCode::Ok;
            }
            match take_finished_async_scan(state) {
                Some(code) => {
                    *out_status = SdScanStatus::Completed;
                    code
                }
                None => {
                    *out_status = SdScanStatus::Running;
                    SdResultCode::Ok
                }
            }
        });

        result.unwrap_or(SdResultCode::InvalidHandle)
    }
}

/// Block the calling thread until a scan started with `sd_scan_start_async` finishes, then
/// finalize and return its result exactly like the blocking `sd_scan_start`. Returns `Ok`
/// immediately if no async scan is running (including one already observed as finished). Unlike
/// `sd_scan_start`, this does not hold the handle for the wait itself, so `sd_scan_cancel` and
/// every query remain usable from another thread while this call blocks.
#[unsafe(no_mangle)]
pub extern "C" fn sd_scan_join(handle: u64) -> SdResultCode {
    let thread = with_handle(handle, |state| {
        state
            .async_scan
            .as_mut()
            .and_then(|scan| scan.thread.take())
    });
    let Some(maybe_thread) = thread else {
        return SdResultCode::InvalidHandle;
    };
    if let Some(thread) = maybe_thread {
        let _ = thread.join();
    }

    let result = with_handle(handle, |state| {
        take_finished_async_scan(state).unwrap_or(SdResultCode::Ok)
    });
    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Request cancellation of the current scan (synchronous or async). Safe to call more than once;
/// a second call is a no-op.
#[unsafe(no_mangle)]
pub extern "C" fn sd_scan_cancel(handle: u64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        state.cancel_token.store(true, Ordering::Relaxed);
        SdResultCode::Ok
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Check if a scan is currently running.
#[unsafe(no_mangle)]
pub extern "C" fn sd_scan_is_running(handle: u64) -> bool {
    with_handle(handle, |state| state.is_scanning).unwrap_or(false)
}

/// Mark a file for deletion.
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sd_deletion_plan_summary(
    handle: u64,
    out_count: *mut i64,
    out_bytes: *mut i64,
) -> SdResultCode {
    unsafe {
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
}

/// Mark all files of the active run that are in `directory_path` or its subdirectories for
/// deletion.
///
/// # Safety
/// `directory_path` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sd_mark_directory_for_deletion(
    handle: u64,
    directory_path: *const c_char,
) -> SdResultCode {
    unsafe {
        let path_str = match c_string_to_rust(directory_path) {
            Some(s) => s,
            None => {
                set_last_error("directory_path is null".to_string());
                return SdResultCode::InvalidArgument;
            }
        };

        let result = with_handle(handle, |state| {
            let run_id = match state.active_session_id {
                Some(id) => id,
                None => {
                    set_last_error("No active session — run a scan first".to_string());
                    return SdResultCode::DatabaseError;
                }
            };
            let db = match &state.db {
                Some(db) => db,
                None => {
                    set_last_error("No database open".to_string());
                    return SdResultCode::DatabaseError;
                }
            };
            match super_duper_core::analysis::deletion_plan::mark_directory_for_deletion(
                db, run_id, &path_str, None,
            ) {
                Ok(_) => SdResultCode::Ok,
                Err(e) => map_core_error(e),
            }
        });

        result.unwrap_or(SdResultCode::InvalidHandle)
    }
}

/// Auto-mark duplicate files for deletion using `strategy`. `preferred_path_prefix` is required
/// (non-null, non-empty) only when `strategy` is `PreferredPathPrefix`; it is ignored otherwise.
///
/// # Safety
/// `preferred_path_prefix` must be null or a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sd_auto_mark_for_deletion(
    handle: u64,
    strategy: SdAutoMarkStrategy,
    preferred_path_prefix: *const c_char,
) -> SdResultCode {
    use super_duper_core::analysis::deletion_plan::AutoMarkStrategy;

    let prefix = unsafe { c_string_to_rust(preferred_path_prefix) };
    let strategy = match strategy {
        SdAutoMarkStrategy::KeepFirst => AutoMarkStrategy::KeepFirst,
        SdAutoMarkStrategy::KeepNewest => AutoMarkStrategy::KeepNewest,
        SdAutoMarkStrategy::KeepOldest => AutoMarkStrategy::KeepOldest,
        SdAutoMarkStrategy::PreferredPathPrefix => match prefix {
            Some(prefix) if !prefix.trim().is_empty() => {
                AutoMarkStrategy::PreferredPathPrefix(prefix)
            }
            _ => {
                set_last_error(
                    "preferred_path_prefix strategy requires a non-empty prefix".to_string(),
                );
                return SdResultCode::InvalidArgument;
            }
        },
    };

    let result = with_handle(handle, |state| {
        let session_id = match state.active_session_id {
            Some(id) => id,
            None => {
                set_last_error("No active session — run a scan first".to_string());
                return SdResultCode::DatabaseError;
            }
        };
        let db = match &state.db {
            Some(db) => db,
            None => {
                set_last_error("No database open".to_string());
                return SdResultCode::DatabaseError;
            }
        };
        match super_duper_core::analysis::deletion_plan::auto_mark_duplicates(
            db, session_id, &strategy,
        ) {
            Ok(_) => SdResultCode::Ok,
            Err(e) => map_core_error(e),
        }
    });

    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Set the active session used by all query functions.
#[unsafe(no_mangle)]
pub extern "C" fn sd_set_active_session(handle: u64, session_id: i64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        state.active_session_id = Some(session_id);
        SdResultCode::Ok
    });
    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Delete a scan session and its duplicate groups.
/// scanned_file rows are preserved (they are the global file index).
/// If the deleted session was the active one, the active session is updated to the
/// most recent remaining completed session.
#[unsafe(no_mangle)]
pub extern "C" fn sd_delete_session(handle: u64, session_id: i64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        let db = match &state.db {
            Some(db) => db,
            None => {
                set_last_error("No database open".to_string());
                return SdResultCode::DatabaseError;
            }
        };
        match db.delete_run(session_id) {
            Ok(()) => {
                if state.active_session_id == Some(session_id) {
                    state.active_session_id = db.get_latest_completed_run_id().unwrap_or(None);
                }
                SdResultCode::Ok
            }
            Err(e) => {
                set_last_error(format!("Failed to delete session: {}", e));
                SdResultCode::DatabaseError
            }
        }
    });
    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Truncate all SQLite tables (sessions, files, groups, directory data, deletion plan).
/// The hash cache (RocksDB) is NOT touched.
/// Clears the engine's active_session_id.
#[unsafe(no_mangle)]
pub extern "C" fn sd_truncate_database(handle: u64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        if state.is_scanning {
            return SdResultCode::ScanInProgress;
        }
        match state.db.as_ref().map(|db| db.truncate_all()) {
            Some(Ok(())) => {
                state.active_session_id = None;
                SdResultCode::Ok
            }
            Some(Err(e)) => {
                set_last_error(e.to_string());
                SdResultCode::DatabaseError
            }
            None => SdResultCode::InternalError,
        }
    });
    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Delete all session history and derived analysis results.
/// The scanned_file global index and hash cache are preserved.
/// Clears the engine's active_session_id.
#[unsafe(no_mangle)]
pub extern "C" fn sd_delete_all_sessions(handle: u64) -> SdResultCode {
    let result = with_handle(handle, |state| {
        if state.is_scanning {
            return SdResultCode::ScanInProgress;
        }
        match state.db.as_ref().map(|db| db.delete_all_sessions()) {
            Some(Ok(())) => {
                state.active_session_id = None;
                SdResultCode::Ok
            }
            Some(Err(e)) => {
                set_last_error(e.to_string());
                SdResultCode::DatabaseError
            }
            None => SdResultCode::InternalError,
        }
    });
    result.unwrap_or(SdResultCode::InvalidHandle)
}

/// Clear all entries from the RocksDB hash cache at `HASH_CACHE_PATH` (default
/// `content_hash_cache.db`). Fails while a scan holds the cache open.
/// Does not affect the SQLite database.
#[unsafe(no_mangle)]
pub extern "C" fn sd_clear_hash_cache() -> SdResultCode {
    let cache = super_duper_core::hasher::cache::default_hash_cache_path();
    match super_duper_core::hasher::cache::clear_all(&cache) {
        Ok(()) => SdResultCode::Ok,
        Err(e) => {
            set_last_error(e.to_string());
            SdResultCode::InternalError
        }
    }
}

/// Remove hash-cache entries not confirmed unchanged, or created, within the last
/// `max_unseen_generations` scans (see `super_duper_core::hasher::cache::DEFAULT_TRIM_UNSEEN_GENERATIONS`
/// for the CLI's default). Fails while a scan holds the cache open. Does not affect the SQLite
/// database.
///
/// # Safety
/// `out_result` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sd_trim_hash_cache(
    max_unseen_generations: u64,
    out_result: *mut SdTrimHashCacheResult,
) -> SdResultCode {
    unsafe {
        if out_result.is_null() {
            set_last_error("out_result is null".to_string());
            return SdResultCode::InvalidArgument;
        }
        let cache = super_duper_core::hasher::cache::default_hash_cache_path();
        match super_duper_core::hasher::cache::trim(&cache, max_unseen_generations) {
            Ok(report) => {
                *out_result = SdTrimHashCacheResult {
                    live_entries_before: report.live_entries_before,
                    removed: report.removed,
                };
                SdResultCode::Ok
            }
            Err(e) => {
                set_last_error(e.to_string());
                SdResultCode::InternalError
            }
        }
    }
}

/// Execute the deletion plan. Returns success/error counts via out parameters.
///
/// Each file is re-validated against its scan snapshot (identity, size, modification time,
/// content hash) and is removed only while another member of each of its duplicate groups still
/// exists unchanged; entries that fail are skipped and counted in `error_count`.
///
/// When `use_trash` is non-zero, files are moved to the system Recycle Bin / Trash
/// instead of being permanently deleted.
///
/// # Safety
/// `out_result` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sd_deletion_execute(
    handle: u64,
    use_trash: u8,
    out_result: *mut SdDeletionResult,
) -> SdResultCode {
    unsafe {
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
            match super_duper_core::analysis::deletion_plan::execute_deletion_plan(
                db,
                use_trash != 0,
            ) {
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
}
