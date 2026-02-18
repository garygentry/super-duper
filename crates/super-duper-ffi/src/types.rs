use std::ffi::{c_char, CStr, CString};
use std::ptr;

/// Result codes returned by all FFI functions.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdResultCode {
    Ok = 0,
    InvalidHandle = 1,
    InvalidArgument = 2,
    IoError = 3,
    DatabaseError = 4,
    ScanInProgress = 5,
    ScanNotRunning = 6,
    Cancelled = 7,
    InternalError = 99,
}

/// A page of duplicate groups returned by paginated queries.
#[repr(C)]
pub struct SdDuplicateGroupPage {
    pub groups: *mut SdDuplicateGroup,
    pub count: u32,
    pub total_available: u32,
}

/// A single duplicate group.
#[repr(C)]
pub struct SdDuplicateGroup {
    pub id: i64,
    pub content_hash: i64,
    pub file_size: i64,
    pub file_count: i64,
    pub wasted_bytes: i64,
}

/// A page of file records.
#[repr(C)]
pub struct SdFileRecordPage {
    pub files: *mut SdFileRecord,
    pub count: u32,
}

/// A single file record.
#[repr(C)]
pub struct SdFileRecord {
    pub id: i64,
    pub canonical_path: *mut c_char,
    pub file_name: *mut c_char,
    pub parent_dir: *mut c_char,
    pub file_size: i64,
    pub content_hash: i64,
    pub is_marked_for_deletion: u8,
}

/// A page of directory nodes.
#[repr(C)]
pub struct SdDirectoryNodePage {
    pub nodes: *mut SdDirectoryNode,
    pub count: u32,
}

/// A single directory node.
#[repr(C)]
pub struct SdDirectoryNode {
    pub id: i64,
    pub path: *mut c_char,
    pub name: *mut c_char,
    pub parent_id: i64,
    pub total_size: i64,
    pub file_count: i64,
    pub depth: i64,
}

/// A page of directory similarity pairs.
#[repr(C)]
pub struct SdDirectorySimilarityPage {
    pub pairs: *mut SdDirectorySimilarity,
    pub count: u32,
}

/// A single directory similarity pair.
#[repr(C)]
pub struct SdDirectorySimilarity {
    pub id: i64,
    pub dir_a_id: i64,
    pub dir_b_id: i64,
    pub dir_a_path: *mut c_char,
    pub dir_b_path: *mut c_char,
    pub similarity_score: f64,
    pub shared_bytes: i64,
    pub match_type: *mut c_char,
}

/// A page of scan session records.
#[repr(C)]
pub struct SdSessionPage {
    pub sessions: *mut SdSessionInfo,
    pub count: u32,
    pub total_available: u32,
}

/// A single scan session record.
#[repr(C)]
pub struct SdSessionInfo {
    pub id: i64,
    pub started_at: *mut c_char,
    pub completed_at: *mut c_char, // null if still running
    pub status: *mut c_char,
    pub root_paths: *mut c_char,
    pub files_scanned: i64,
    pub total_bytes: i64,
    pub group_count: i64,
    pub is_active: u8,
}

/// Deletion execution result.
#[repr(C)]
pub struct SdDeletionResult {
    pub success_count: u32,
    pub error_count: u32,
}

/// Progress callback signature.
pub type SdProgressCallback = extern "C" fn(
    phase: u32,           // 0=scan, 1=hash, 2=db_write
    current: u64,
    total: u64,
    message: *const c_char,
);

/// Helper to convert a Rust string to a C string on the heap.
pub fn rust_string_to_c(s: &str) -> *mut c_char {
    CString::new(s)
        .map(|cs| cs.into_raw())
        .unwrap_or(ptr::null_mut())
}

/// Helper to convert a C string to a Rust string.
///
/// # Safety
/// The caller must ensure `ptr` is a valid null-terminated C string.
pub unsafe fn c_string_to_rust(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string())
}
