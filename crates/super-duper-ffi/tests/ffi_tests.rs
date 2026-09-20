use std::ffi::{CStr, CString, c_char};
use std::fs;
use std::ptr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use tempfile::tempdir;

use super_duper_ffi::actions::*;
use super_duper_ffi::error::*;
use super_duper_ffi::queries::*;
use super_duper_ffi::types::*;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn c_str(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn create_engine(db_path: &str) -> u64 {
    let path = c_str(db_path);
    unsafe { sd_engine_create(path.as_ptr()) }
}

/// Create a temp directory tree with known duplicates for scanning.
fn create_test_tree(root: &std::path::Path) {
    let folder_a = root.join("folder_a");
    let folder_b = root.join("folder_b");
    fs::create_dir_all(&folder_a).unwrap();
    fs::create_dir_all(&folder_b).unwrap();

    fs::write(folder_a.join("unique_a.txt"), "unique content a").unwrap();
    fs::write(folder_b.join("unique_b.txt"), "unique content b").unwrap();

    // Duplicate across folders
    fs::write(folder_a.join("shared.txt"), "shared content xyz").unwrap();
    fs::write(folder_b.join("shared.txt"), "shared content xyz").unwrap();

    // Duplicate within folder_b
    let big = vec![0xAAu8; 4096];
    fs::write(folder_a.join("large_dup.bin"), &big).unwrap();
    fs::write(folder_b.join("large_dup.bin"), &big).unwrap();
}

// ── Handle lifecycle ─────────────────────────────────────────────────────────

#[test]
fn test_handle_create_and_destroy() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());
    assert_ne!(handle, 0, "handle should be non-zero");

    let result = sd_engine_destroy(handle);
    assert_eq!(result, SdResultCode::Ok);
}

#[test]
fn test_destroy_invalid_handle() {
    let result = sd_engine_destroy(999999);
    assert_eq!(result, SdResultCode::InvalidHandle);
}

#[test]
fn test_double_destroy() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    assert_eq!(sd_engine_destroy(handle), SdResultCode::Ok);
    assert_eq!(sd_engine_destroy(handle), SdResultCode::InvalidHandle);
}

#[test]
fn test_create_with_null_path_uses_default() {
    let handle = unsafe { sd_engine_create(ptr::null()) };
    assert_ne!(handle, 0);
    sd_engine_destroy(handle);
    // Clean up the default db file
    let _ = fs::remove_file("super_duper.db");
}

// ── Set scan paths ───────────────────────────────────────────────────────────

#[test]
fn test_set_scan_paths() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let path1 = c_str("/tmp/path1");
    let path2 = c_str("/tmp/path2");
    let paths = [path1.as_ptr(), path2.as_ptr()];

    let result = unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 2) };
    assert_eq!(result, SdResultCode::Ok);

    sd_engine_destroy(handle);
}

#[test]
fn test_set_scan_paths_null_array() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let result = unsafe { sd_engine_set_scan_paths(handle, ptr::null(), 1) };
    assert_eq!(result, SdResultCode::InvalidArgument);

    sd_engine_destroy(handle);
}

#[test]
fn test_set_scan_paths_invalid_handle() {
    let path1 = c_str("/tmp/path1");
    let paths = [path1.as_ptr()];

    let result = unsafe { sd_engine_set_scan_paths(999999, paths.as_ptr(), 1) };
    assert_eq!(result, SdResultCode::InvalidHandle);
}

// ── Set ignore patterns ──────────────────────────────────────────────────────

#[test]
fn test_set_ignore_patterns() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let pat = c_str("**/.git/**");
    let patterns = [pat.as_ptr()];

    let result = unsafe { sd_engine_set_ignore_patterns(handle, patterns.as_ptr(), 1) };
    assert_eq!(result, SdResultCode::Ok);

    sd_engine_destroy(handle);
}

#[test]
fn test_set_ignore_patterns_null_with_nonzero_count() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let result = unsafe { sd_engine_set_ignore_patterns(handle, ptr::null(), 3) };
    assert_eq!(result, SdResultCode::InvalidArgument);

    sd_engine_destroy(handle);
}

// ── Scan operations ──────────────────────────────────────────────────────────

#[test]
fn test_scan_and_query_duplicates() {
    let dir = tempdir().unwrap();
    let scan_dir = dir.path().join("data");
    let db_path = dir.path().join("test.db");
    create_test_tree(&scan_dir);

    let handle = create_engine(db_path.to_str().unwrap());

    // Set scan path
    let scan_path_str = c_str(scan_dir.to_str().unwrap());
    let paths = [scan_path_str.as_ptr()];
    unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 1) };

    // Run scan
    let result = sd_scan_start(handle);
    assert_eq!(result, SdResultCode::Ok);

    // Query duplicate groups
    let mut page = SdDuplicateGroupPage {
        groups: ptr::null_mut(),
        count: 0,
        total_available: 0,
    };
    let result = unsafe { sd_query_duplicate_groups(handle, 0, 100, &mut page) };
    assert_eq!(result, SdResultCode::Ok);
    assert!(page.count > 0, "should find duplicate groups");
    assert!(page.total_available > 0);

    // Query files in first group
    let first_group_id = unsafe { (*page.groups).id };
    let mut file_page = SdFileRecordPage {
        files: ptr::null_mut(),
        count: 0,
    };
    let result = unsafe { sd_query_files_in_group(handle, first_group_id, &mut file_page) };
    assert_eq!(result, SdResultCode::Ok);
    assert!(
        file_page.count >= 2,
        "duplicate group should have at least 2 files"
    );

    // Verify file records have valid strings
    for i in 0..file_page.count as usize {
        let file = unsafe { &*file_page.files.add(i) };
        assert!(!file.canonical_path.is_null());
        assert!(!file.file_name.is_null());
        let path = unsafe { CStr::from_ptr(file.canonical_path) }
            .to_str()
            .unwrap();
        assert!(!path.is_empty());
    }

    // Free pages
    unsafe {
        sd_free_file_record_page(&mut file_page);
        sd_free_duplicate_group_page(&mut page);
    }

    sd_engine_destroy(handle);
}

#[test]
fn test_query_empty_database() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let mut page = SdDuplicateGroupPage {
        groups: ptr::null_mut(),
        count: 0,
        total_available: 0,
    };
    let result = unsafe { sd_query_duplicate_groups(handle, 0, 100, &mut page) };
    assert_eq!(result, SdResultCode::Ok);
    assert_eq!(page.count, 0);
    assert_eq!(page.total_available, 0);

    sd_engine_destroy(handle);
}

#[test]
fn test_scan_is_running() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    // Not scanning yet
    assert!(!sd_scan_is_running(handle));

    sd_engine_destroy(handle);
}

// ── Progress callback ────────────────────────────────────────────────────────

static PROGRESS_CALL_COUNT: AtomicU32 = AtomicU32::new(0);

extern "C" fn test_progress_callback(
    _phase: u32,
    _current: u64,
    _total: u64,
    _message: *const c_char,
) {
    PROGRESS_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn test_progress_callback_fires() {
    let dir = tempdir().unwrap();
    let scan_dir = dir.path().join("data");
    let db_path = dir.path().join("test.db");
    create_test_tree(&scan_dir);

    let handle = create_engine(db_path.to_str().unwrap());

    // Set callback
    PROGRESS_CALL_COUNT.store(0, Ordering::SeqCst);
    let result = sd_set_progress_callback(handle, test_progress_callback);
    assert_eq!(result, SdResultCode::Ok);

    // Set scan path and scan
    let scan_path_str = c_str(scan_dir.to_str().unwrap());
    let paths = [scan_path_str.as_ptr()];
    unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 1) };
    sd_scan_start(handle);

    // Callback should have fired
    assert!(
        PROGRESS_CALL_COUNT.load(Ordering::SeqCst) > 0,
        "progress callback should have been called"
    );

    // Clear callback
    let result = sd_clear_progress_callback(handle);
    assert_eq!(result, SdResultCode::Ok);

    sd_engine_destroy(handle);
}

// ── Async scan ───────────────────────────────────────────────────────────────
// `sd_scan_start_async`/`sd_scan_observe`/`sd_scan_join` run the scan on a background thread so
// `sd_scan_cancel` and every query stay usable while it runs (unlike `sd_scan_start`, which holds
// the handle, and therefore every other call for every handle, for its whole duration).

static CANCEL_ON_PROGRESS_HANDLE: AtomicU64 = AtomicU64::new(0);

/// Cancels the scan the very first time it fires (`on_scan_start`), before any real traversal
/// work, so cancellation mid-scan is deterministic instead of racing a real file scan's duration.
extern "C" fn cancel_on_first_progress_callback(
    _phase: u32,
    _current: u64,
    _total: u64,
    _message: *const c_char,
) {
    let handle = CANCEL_ON_PROGRESS_HANDLE.swap(0, Ordering::SeqCst);
    if handle != 0 {
        sd_scan_cancel(handle);
    }
}

#[test]
fn test_async_scan_completes_and_populates_query_results() {
    let dir = tempdir().unwrap();
    let scan_dir = dir.path().join("data");
    let db_path = dir.path().join("test.db");
    create_test_tree(&scan_dir);

    let handle = create_engine(db_path.to_str().unwrap());
    let scan_path_str = c_str(scan_dir.to_str().unwrap());
    let paths = [scan_path_str.as_ptr()];
    unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 1) };

    assert_eq!(sd_scan_start_async(handle), SdResultCode::Ok);
    assert!(sd_scan_is_running(handle));
    // Only one scan (sync or async) may run per handle at a time, same as `sd_scan_start`.
    assert_eq!(sd_scan_start_async(handle), SdResultCode::ScanInProgress);

    assert_eq!(sd_scan_join(handle), SdResultCode::Ok);
    assert!(!sd_scan_is_running(handle));
    // Joining again reports idle rather than re-running or blocking.
    assert_eq!(sd_scan_join(handle), SdResultCode::Ok);

    let mut page = SdDuplicateGroupPage {
        groups: ptr::null_mut(),
        count: 0,
        total_available: 0,
    };
    let result = unsafe { sd_query_duplicate_groups(handle, 0, 100, &mut page) };
    assert_eq!(result, SdResultCode::Ok);
    assert!(
        page.count > 0,
        "the async scan's results should be queryable once joined"
    );
    unsafe { sd_free_duplicate_group_page(&mut page) };

    sd_engine_destroy(handle);
}

#[test]
fn test_scan_observe_reports_idle_then_completed_exactly_once() {
    let dir = tempdir().unwrap();
    let scan_dir = dir.path().join("data");
    let db_path = dir.path().join("test.db");
    create_test_tree(&scan_dir);

    let handle = create_engine(db_path.to_str().unwrap());
    let scan_path_str = c_str(scan_dir.to_str().unwrap());
    let paths = [scan_path_str.as_ptr()];
    unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 1) };

    let mut status = SdScanStatus::Running;
    assert_eq!(
        unsafe { sd_scan_observe(handle, &mut status) },
        SdResultCode::Ok
    );
    assert_eq!(status, SdScanStatus::Idle, "no scan has started yet");

    assert_eq!(sd_scan_start_async(handle), SdResultCode::Ok);

    let mut code = SdResultCode::InvalidHandle;
    for _ in 0..2000 {
        code = unsafe { sd_scan_observe(handle, &mut status) };
        if status == SdScanStatus::Completed {
            break;
        }
        assert_eq!(status, SdScanStatus::Running);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        status,
        SdScanStatus::Completed,
        "scan did not finish in time"
    );
    assert_eq!(code, SdResultCode::Ok);
    assert!(!sd_scan_is_running(handle));

    // `Completed` is reported once; a later observation is idle rather than repeating it.
    assert_eq!(
        unsafe { sd_scan_observe(handle, &mut status) },
        SdResultCode::Ok
    );
    assert_eq!(status, SdScanStatus::Idle);

    sd_engine_destroy(handle);
}

#[test]
fn test_async_scan_cancelled_mid_scan_reports_cancelled_on_join() {
    let dir = tempdir().unwrap();
    let scan_dir = dir.path().join("data");
    let db_path = dir.path().join("test.db");
    create_test_tree(&scan_dir);

    let handle = create_engine(db_path.to_str().unwrap());
    let scan_path_str = c_str(scan_dir.to_str().unwrap());
    let paths = [scan_path_str.as_ptr()];
    unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 1) };

    CANCEL_ON_PROGRESS_HANDLE.store(handle, Ordering::SeqCst);
    assert_eq!(
        sd_set_progress_callback(handle, cancel_on_first_progress_callback),
        SdResultCode::Ok
    );

    assert_eq!(sd_scan_start_async(handle), SdResultCode::Ok);
    assert_eq!(sd_scan_join(handle), SdResultCode::Cancelled);
    assert!(!sd_scan_is_running(handle));

    sd_engine_destroy(handle);
}

#[test]
fn test_async_scan_double_cancel_during_scan_is_safe() {
    let dir = tempdir().unwrap();
    let scan_dir = dir.path().join("data");
    let db_path = dir.path().join("test.db");
    create_test_tree(&scan_dir);

    let handle = create_engine(db_path.to_str().unwrap());
    let scan_path_str = c_str(scan_dir.to_str().unwrap());
    let paths = [scan_path_str.as_ptr()];
    unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 1) };

    assert_eq!(sd_scan_start_async(handle), SdResultCode::Ok);
    assert_eq!(sd_scan_cancel(handle), SdResultCode::Ok);
    assert_eq!(
        sd_scan_cancel(handle),
        SdResultCode::Ok,
        "a second cancel is a no-op"
    );

    // The scan may have finished before either cancel took effect (the fixture tree is tiny), so
    // either outcome is a correct, safe result; the point is that neither call crashed or hung.
    let join_result = sd_scan_join(handle);
    assert!(matches!(
        join_result,
        SdResultCode::Ok | SdResultCode::Cancelled
    ));
    assert!(!sd_scan_is_running(handle));

    sd_engine_destroy(handle);
}

#[test]
fn test_engine_destroy_while_async_scan_is_running_does_not_block_or_crash() {
    let dir = tempdir().unwrap();
    let scan_dir = dir.path().join("data");
    let db_path = dir.path().join("test.db");
    create_test_tree(&scan_dir);

    let handle = create_engine(db_path.to_str().unwrap());
    let scan_path_str = c_str(scan_dir.to_str().unwrap());
    let paths = [scan_path_str.as_ptr()];
    unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 1) };

    assert_eq!(sd_scan_start_async(handle), SdResultCode::Ok);
    // Must return promptly (it does not join the background thread) and must not crash even
    // though the scan may still be running; the orphaned thread keeps its own `Arc` clones alive
    // and simply discards its result once it finishes.
    assert_eq!(sd_engine_destroy(handle), SdResultCode::Ok);

    assert_eq!(sd_scan_cancel(handle), SdResultCode::InvalidHandle);
    assert_eq!(sd_scan_join(handle), SdResultCode::InvalidHandle);
    let mut status = SdScanStatus::Idle;
    assert_eq!(
        unsafe { sd_scan_observe(handle, &mut status) },
        SdResultCode::InvalidHandle
    );
}

#[test]
fn test_scan_join_invalid_handle() {
    assert_eq!(sd_scan_join(999999), SdResultCode::InvalidHandle);
}

#[test]
fn test_scan_observe_invalid_handle() {
    let mut status = SdScanStatus::Idle;
    let result = unsafe { sd_scan_observe(999999, &mut status) };
    assert_eq!(result, SdResultCode::InvalidHandle);
}

#[test]
fn test_scan_observe_null_out_status() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());
    let result = unsafe { sd_scan_observe(handle, ptr::null_mut()) };
    assert_eq!(result, SdResultCode::InvalidArgument);
    sd_engine_destroy(handle);
}

// ── Deletion operations ──────────────────────────────────────────────────────

#[test]
fn test_mark_unmark_deletion() {
    let dir = tempdir().unwrap();
    let scan_dir = dir.path().join("data");
    let db_path = dir.path().join("test.db");
    create_test_tree(&scan_dir);

    let handle = create_engine(db_path.to_str().unwrap());

    // Scan first
    let scan_path_str = c_str(scan_dir.to_str().unwrap());
    let paths = [scan_path_str.as_ptr()];
    unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 1) };
    sd_scan_start(handle);

    // Get a file ID from the first group
    let mut page = SdDuplicateGroupPage {
        groups: ptr::null_mut(),
        count: 0,
        total_available: 0,
    };
    unsafe { sd_query_duplicate_groups(handle, 0, 100, &mut page) };
    assert!(page.count > 0);

    let group_id = unsafe { (*page.groups).id };
    let mut file_page = SdFileRecordPage {
        files: ptr::null_mut(),
        count: 0,
    };
    unsafe { sd_query_files_in_group(handle, group_id, &mut file_page) };
    assert!(file_page.count >= 2);

    let file_id = unsafe { (*file_page.files).id };

    // Initially not marked
    assert_eq!(unsafe { (*file_page.files).is_marked_for_deletion }, 0);

    // Mark
    let result = sd_mark_file_for_deletion(handle, file_id);
    assert_eq!(result, SdResultCode::Ok);

    // Verify via re-query
    unsafe { sd_free_file_record_page(&mut file_page) };
    let mut file_page2 = SdFileRecordPage {
        files: ptr::null_mut(),
        count: 0,
    };
    unsafe { sd_query_files_in_group(handle, group_id, &mut file_page2) };
    let marked_file = (0..file_page2.count as usize)
        .find(|&i| unsafe { (*file_page2.files.add(i)).id } == file_id)
        .map(|i| unsafe { (*file_page2.files.add(i)).is_marked_for_deletion });
    assert_eq!(marked_file, Some(1), "file should be marked for deletion");

    // Unmark
    let result = sd_unmark_file_for_deletion(handle, file_id);
    assert_eq!(result, SdResultCode::Ok);

    // Verify unmarked
    unsafe { sd_free_file_record_page(&mut file_page2) };
    let mut file_page3 = SdFileRecordPage {
        files: ptr::null_mut(),
        count: 0,
    };
    unsafe { sd_query_files_in_group(handle, group_id, &mut file_page3) };
    let unmarked_file = (0..file_page3.count as usize)
        .find(|&i| unsafe { (*file_page3.files.add(i)).id } == file_id)
        .map(|i| unsafe { (*file_page3.files.add(i)).is_marked_for_deletion });
    assert_eq!(unmarked_file, Some(0), "file should be unmarked");

    // Cleanup
    unsafe {
        sd_free_file_record_page(&mut file_page3);
        sd_free_duplicate_group_page(&mut page);
    }
    sd_engine_destroy(handle);
}

#[test]
fn test_deletion_plan_summary() {
    let dir = tempdir().unwrap();
    let scan_dir = dir.path().join("data");
    let db_path = dir.path().join("test.db");
    create_test_tree(&scan_dir);

    let handle = create_engine(db_path.to_str().unwrap());

    // Scan
    let scan_path_str = c_str(scan_dir.to_str().unwrap());
    let paths = [scan_path_str.as_ptr()];
    unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 1) };
    sd_scan_start(handle);

    // Initially empty
    let mut count: i64 = -1;
    let mut bytes: i64 = -1;
    let result = unsafe { sd_deletion_plan_summary(handle, &mut count, &mut bytes) };
    assert_eq!(result, SdResultCode::Ok);
    assert_eq!(count, 0);
    assert_eq!(bytes, 0);

    // Auto-mark
    let result =
        unsafe { sd_auto_mark_for_deletion(handle, SdAutoMarkStrategy::KeepFirst, ptr::null()) };
    assert_eq!(result, SdResultCode::Ok);

    // Now should have entries
    let result = unsafe { sd_deletion_plan_summary(handle, &mut count, &mut bytes) };
    assert_eq!(result, SdResultCode::Ok);
    assert!(count > 0, "should have files marked for deletion");
    assert!(bytes > 0, "should have bytes marked for deletion");

    sd_engine_destroy(handle);
}

#[test]
fn test_deletion_plan_summary_null_pointers() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let mut count: i64 = 0;
    let result = unsafe { sd_deletion_plan_summary(handle, &mut count, ptr::null_mut()) };
    assert_eq!(result, SdResultCode::InvalidArgument);

    let result = unsafe { sd_deletion_plan_summary(handle, ptr::null_mut(), &mut count) };
    assert_eq!(result, SdResultCode::InvalidArgument);

    sd_engine_destroy(handle);
}

// ── Hash cache maintenance ───────────────────────────────────────────────────
// Trim mechanics (which entries age out, legacy stores, failing while a scan holds the store) are
// covered by super-duper-core's hasher::repeat_cache and hasher::cache unit tests; these only cover
// the FFI wiring. `HASH_CACHE_PATH` is process-global, so only this one test touches it.

#[test]
fn test_trim_hash_cache_reports_zero_for_a_fresh_cache_path() {
    let dir = tempdir().unwrap();
    let cache_path = dir.path().join("content_hash_cache.db");
    // SAFETY: no other test reads or writes `HASH_CACHE_PATH`.
    unsafe { std::env::set_var("HASH_CACHE_PATH", &cache_path) };

    let mut result = SdTrimHashCacheResult {
        live_entries_before: u64::MAX,
        removed: u64::MAX,
    };
    let code = unsafe { sd_trim_hash_cache(10, &mut result) };

    unsafe { std::env::remove_var("HASH_CACHE_PATH") };

    assert_eq!(code, SdResultCode::Ok);
    assert_eq!(result.live_entries_before, 0);
    assert_eq!(result.removed, 0);
}

#[test]
fn test_trim_hash_cache_null_out_result() {
    let result = unsafe { sd_trim_hash_cache(10, ptr::null_mut()) };
    assert_eq!(result, SdResultCode::InvalidArgument);
}

// ── Error message API ────────────────────────────────────────────────────────

#[test]
fn test_last_error_message_after_invalid_handle() {
    // Trigger an error
    sd_engine_destroy(999999);

    let msg_ptr = sd_last_error_message();
    assert!(!msg_ptr.is_null());
    let msg = unsafe { CStr::from_ptr(msg_ptr) }.to_str().unwrap();
    assert!(
        msg.contains("Invalid handle"),
        "error message should mention invalid handle, got: {msg}"
    );

    unsafe { sd_free_string(msg_ptr) };
}

#[test]
fn test_free_null_string() {
    // Should not panic
    unsafe { sd_free_string(ptr::null_mut()) };
}

// ── Query null pointer checks ────────────────────────────────────────────────

#[test]
fn test_query_duplicate_groups_null_out() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let result = unsafe { sd_query_duplicate_groups(handle, 0, 100, ptr::null_mut()) };
    assert_eq!(result, SdResultCode::InvalidArgument);

    sd_engine_destroy(handle);
}

#[test]
fn test_query_files_in_group_null_out() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let result = unsafe { sd_query_files_in_group(handle, 1, ptr::null_mut()) };
    assert_eq!(result, SdResultCode::InvalidArgument);

    sd_engine_destroy(handle);
}

#[test]
fn test_query_directory_children_null_out() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let result = unsafe { sd_query_directory_children(handle, -1, 0, 100, ptr::null_mut()) };
    assert_eq!(result, SdResultCode::InvalidArgument);

    sd_engine_destroy(handle);
}

#[test]
fn test_query_similar_directories_null_out() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let result = unsafe { sd_query_similar_directories(handle, 0.5, 0, 100, ptr::null_mut()) };
    assert_eq!(result, SdResultCode::InvalidArgument);

    sd_engine_destroy(handle);
}

// ── Query with invalid handle ────────────────────────────────────────────────

#[test]
fn test_query_duplicate_groups_invalid_handle() {
    let mut page = SdDuplicateGroupPage {
        groups: ptr::null_mut(),
        count: 0,
        total_available: 0,
    };
    let result = unsafe { sd_query_duplicate_groups(999999, 0, 100, &mut page) };
    assert_eq!(result, SdResultCode::InvalidHandle);
}

#[test]
fn test_mark_file_invalid_handle() {
    let result = sd_mark_file_for_deletion(999999, 1);
    assert_eq!(result, SdResultCode::InvalidHandle);
}

#[test]
fn test_scan_start_invalid_handle() {
    let result = sd_scan_start(999999);
    assert_eq!(result, SdResultCode::InvalidHandle);
}

#[test]
fn test_scan_cancel_invalid_handle() {
    let result = sd_scan_cancel(999999);
    assert_eq!(result, SdResultCode::InvalidHandle);
}

// ── Free null page safety ────────────────────────────────────────────────────

#[test]
fn test_free_null_pages() {
    // All free functions should handle null gracefully
    unsafe {
        sd_free_duplicate_group_page(ptr::null_mut());
        sd_free_file_record_page(ptr::null_mut());
        sd_free_directory_node_page(ptr::null_mut());
        sd_free_directory_similarity_page(ptr::null_mut());
    }
}

// ── Deletion execute ─────────────────────────────────────────────────────────

#[test]
fn test_deletion_execute_null_result() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let result = unsafe { sd_deletion_execute(handle, 1, ptr::null_mut()) };
    assert_eq!(result, SdResultCode::InvalidArgument);

    sd_engine_destroy(handle);
}

#[test]
fn test_auto_mark_and_execute_deletion() {
    let dir = tempdir().unwrap();
    let scan_dir = dir.path().join("data");
    let db_path = dir.path().join("test.db");
    create_test_tree(&scan_dir);

    let handle = create_engine(db_path.to_str().unwrap());

    // Scan
    let scan_path_str = c_str(scan_dir.to_str().unwrap());
    let paths = [scan_path_str.as_ptr()];
    unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 1) };
    sd_scan_start(handle);

    // Auto-mark
    unsafe { sd_auto_mark_for_deletion(handle, SdAutoMarkStrategy::KeepFirst, ptr::null()) };

    // Get count before
    let mut count: i64 = 0;
    let mut bytes: i64 = 0;
    unsafe { sd_deletion_plan_summary(handle, &mut count, &mut bytes) };
    assert!(count > 0);

    // Execute
    let mut result_out = SdDeletionResult {
        success_count: 0,
        error_count: 0,
    };
    let result = unsafe { sd_deletion_execute(handle, 1, &mut result_out) };
    assert_eq!(result, SdResultCode::Ok);
    assert!(result_out.success_count > 0, "should have deleted files");

    // After execution, plan should be empty
    unsafe { sd_deletion_plan_summary(handle, &mut count, &mut bytes) };
    assert_eq!(count, 0, "deletion plan should be empty after execution");

    sd_engine_destroy(handle);
}

#[test]
fn test_auto_mark_preferred_path_prefix_keeps_matching_file() {
    let dir = tempdir().unwrap();
    let scan_dir = dir.path().join("data");
    let db_path = dir.path().join("test.db");
    create_test_tree(&scan_dir);

    let handle = create_engine(db_path.to_str().unwrap());
    let scan_path_str = c_str(scan_dir.to_str().unwrap());
    let paths = [scan_path_str.as_ptr()];
    unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 1) };
    sd_scan_start(handle);

    let mut page = SdDuplicateGroupPage {
        groups: ptr::null_mut(),
        count: 0,
        total_available: 0,
    };
    unsafe { sd_query_duplicate_groups(handle, 0, 100, &mut page) };
    assert!(page.count > 0);
    let group_id = unsafe { (*page.groups).id };

    let mut file_page = SdFileRecordPage {
        files: ptr::null_mut(),
        count: 0,
    };
    unsafe { sd_query_files_in_group(handle, group_id, &mut file_page) };
    assert!(file_page.count >= 2);

    // Prefer the directory of whichever member happens to be first; the other member(s) of the
    // group live in a sibling directory in `create_test_tree`, so the prefix picks exactly one.
    let preferred_path = unsafe { CStr::from_ptr((*file_page.files).canonical_path) }
        .to_str()
        .unwrap()
        .to_string();
    let preferred_dir = preferred_path
        .rsplit_once(['\\', '/'])
        .expect("canonical path has a parent")
        .0
        .to_string();
    let prefix = c_str(&preferred_dir);

    let result = unsafe {
        sd_auto_mark_for_deletion(
            handle,
            SdAutoMarkStrategy::PreferredPathPrefix,
            prefix.as_ptr(),
        )
    };
    assert_eq!(result, SdResultCode::Ok);

    unsafe { sd_free_file_record_page(&mut file_page) };
    let mut file_page2 = SdFileRecordPage {
        files: ptr::null_mut(),
        count: 0,
    };
    unsafe { sd_query_files_in_group(handle, group_id, &mut file_page2) };
    let preferred_marked = (0..file_page2.count as usize).find_map(|i| unsafe {
        let record = &*file_page2.files.add(i);
        let path = CStr::from_ptr(record.canonical_path).to_str().unwrap();
        if path == preferred_path {
            Some(record.is_marked_for_deletion)
        } else {
            None
        }
    });
    assert_eq!(
        preferred_marked,
        Some(0),
        "the file under the preferred prefix must survive"
    );

    unsafe {
        sd_free_file_record_page(&mut file_page2);
        sd_free_duplicate_group_page(&mut page);
    }
    sd_engine_destroy(handle);
}

#[test]
fn test_auto_mark_preferred_path_prefix_requires_prefix() {
    let dir = tempdir().unwrap();
    let scan_dir = dir.path().join("data");
    let db_path = dir.path().join("test.db");
    create_test_tree(&scan_dir);

    let handle = create_engine(db_path.to_str().unwrap());
    let scan_path_str = c_str(scan_dir.to_str().unwrap());
    let paths = [scan_path_str.as_ptr()];
    unsafe { sd_engine_set_scan_paths(handle, paths.as_ptr(), 1) };
    sd_scan_start(handle);

    let result = unsafe {
        sd_auto_mark_for_deletion(handle, SdAutoMarkStrategy::PreferredPathPrefix, ptr::null())
    };
    assert_eq!(result, SdResultCode::InvalidArgument);

    sd_engine_destroy(handle);
}

// ── Directory queries ────────────────────────────────────────────────────────

#[test]
fn test_query_directory_children_empty() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let mut page = SdDirectoryNodePage {
        nodes: ptr::null_mut(),
        count: 0,
    };
    let result = unsafe { sd_query_directory_children(handle, -1, 0, 100, &mut page) };
    assert_eq!(result, SdResultCode::Ok);
    assert_eq!(page.count, 0);

    sd_engine_destroy(handle);
}

#[test]
fn test_query_similar_directories_empty() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let handle = create_engine(db_path.to_str().unwrap());

    let mut page = SdDirectorySimilarityPage {
        pairs: ptr::null_mut(),
        count: 0,
    };
    let result = unsafe { sd_query_similar_directories(handle, 0.0, 0, 100, &mut page) };
    assert_eq!(result, SdResultCode::Ok);
    assert_eq!(page.count, 0);

    sd_engine_destroy(handle);
}

// ── Multiple handles ─────────────────────────────────────────────────────────

#[test]
fn test_multiple_handles_independent() {
    let dir = tempdir().unwrap();
    let db1 = dir.path().join("db1.db");
    let db2 = dir.path().join("db2.db");

    let h1 = create_engine(db1.to_str().unwrap());
    let h2 = create_engine(db2.to_str().unwrap());

    assert_ne!(h1, h2, "handles should be unique");
    assert_ne!(h1, 0);
    assert_ne!(h2, 0);

    // Destroy one shouldn't affect the other
    assert_eq!(sd_engine_destroy(h1), SdResultCode::Ok);
    assert!(!sd_scan_is_running(h2)); // h2 still works

    assert_eq!(sd_engine_destroy(h2), SdResultCode::Ok);
}
