use crate::types::{rust_string_to_c, SdResultCode};
use std::cell::RefCell;
use std::ffi::c_char;

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = RefCell::new(None);
}

pub fn set_last_error(msg: String) {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = Some(msg);
    });
}

pub fn map_core_error(e: super_duper_core::Error) -> SdResultCode {
    let msg = e.to_string();
    set_last_error(msg);
    match e {
        super_duper_core::Error::Io(_) => SdResultCode::IoError,
        super_duper_core::Error::Database(_) => SdResultCode::DatabaseError,
        super_duper_core::Error::Config(_) => SdResultCode::InvalidArgument,
        super_duper_core::Error::Cancelled => SdResultCode::Cancelled,
        _ => SdResultCode::InternalError,
    }
}

/// Get the last error message. Returns a C string that must be freed with `sd_free_string`.
///
/// # Safety
/// Caller must free the returned string with `sd_free_string`.
#[no_mangle]
pub extern "C" fn sd_last_error_message() -> *mut c_char {
    LAST_ERROR.with(|e| {
        let msg = e.borrow();
        match msg.as_ref() {
            Some(s) => rust_string_to_c(s),
            None => rust_string_to_c(""),
        }
    })
}

/// Free a string allocated by the FFI layer.
///
/// # Safety
/// `ptr` must have been allocated by this library (e.g., from `sd_last_error_message`).
#[no_mangle]
pub unsafe extern "C" fn sd_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(std::ffi::CString::from_raw(ptr));
    }
}
