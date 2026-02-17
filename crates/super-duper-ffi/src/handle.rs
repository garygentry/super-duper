use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use super_duper_core::ScanEngine;
use super_duper_core::storage::Database;

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

pub struct EngineState {
    pub engine: ScanEngine,
    pub db: Option<Database>,
    pub is_scanning: bool,
    pub cancel_token: Arc<AtomicBool>,
    pub progress_bridge: Option<crate::callbacks::FfiProgressBridge>,
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
