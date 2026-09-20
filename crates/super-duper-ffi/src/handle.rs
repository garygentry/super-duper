use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use super_duper_core::ScanEngine;
use super_duper_core::storage::Database;

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

/// A scan started by `sd_scan_start_async`, running on its own thread.
///
/// The thread writes its outcome into `outcome` and exits without touching `EngineState` or any
/// FFI thread-local (`sd_last_error_message` is thread-local, so the error must be translated on
/// the observing thread, not the scan thread — see `sd_scan_observe`/`sd_scan_join`).
pub struct AsyncScan {
    pub thread: Option<JoinHandle<()>>,
    #[allow(clippy::type_complexity)]
    pub outcome: Arc<Mutex<Option<Result<super_duper_core::ScanResult, super_duper_core::Error>>>>,
}

pub struct EngineState {
    pub engine: Arc<ScanEngine>,
    pub db: Option<Database>,
    pub db_path: String,
    pub root_paths: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub is_scanning: bool,
    pub cancel_token: Arc<AtomicBool>,
    pub progress_bridge: Option<Arc<crate::callbacks::FfiProgressBridge>>,
    /// The session whose results are returned by query functions.
    /// Set to the most recent completed session on engine create,
    /// and updated after each successful scan.
    pub active_session_id: Option<i64>,
    /// Set by `sd_scan_start_async`; cleared once `sd_scan_observe` or `sd_scan_join` reports the
    /// outcome. `None` while no async scan has been started, or its outcome already reported.
    pub async_scan: Option<AsyncScan>,
}

lazy_static! {
    static ref HANDLES: Mutex<HashMap<u64, Box<EngineState>>> = Mutex::new(HashMap::new());
}

pub fn allocate_handle(state: EngineState) -> u64 {
    let handle = NEXT_HANDLE.fetch_add(1, Ordering::SeqCst);
    let mut handles = HANDLES.lock().unwrap();
    handles.insert(handle, Box::new(state));
    handle
}

pub fn with_handle<F, R>(handle: u64, f: F) -> Option<R>
where
    F: FnOnce(&mut EngineState) -> R,
{
    let mut handles = HANDLES.lock().unwrap();
    handles.get_mut(&handle).map(|state| f(state))
}

pub fn destroy_handle(handle: u64) -> bool {
    let mut handles = HANDLES.lock().unwrap();
    handles.remove(&handle).is_some()
}
