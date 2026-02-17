use crate::error::set_last_error;
use crate::handle::with_handle;
use crate::types::*;

/// Query duplicate groups with pagination.
///
/// # Safety
/// `out_page` must be a valid pointer. The returned page must be freed with `sd_free_duplicate_group_page`.
#[no_mangle]
pub unsafe extern "C" fn sd_query_duplicate_groups(
    handle: u64,
    offset: i64,
    limit: i64,
    out_page: *mut SdDuplicateGroupPage,
) -> SdResultCode {
    if out_page.is_null() {
        set_last_error("out_page is null".to_string());
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

        match db.get_duplicate_groups(offset, limit) {
            Ok(groups) => {
                let total: i64 = db.get_duplicate_group_count().unwrap_or(0);
                let count = groups.len() as u32;

                let c_groups: Vec<SdDuplicateGroup> = groups
                    .iter()
                    .map(|g| SdDuplicateGroup {
                        id: g.id,
                        content_hash: g.content_hash,
                        file_size: g.file_size,
                        file_count: g.file_count,
                        wasted_bytes: g.wasted_bytes,
                    })
                    .collect();

                let boxed = c_groups.into_boxed_slice();
                let ptr = Box::into_raw(boxed) as *mut SdDuplicateGroup;

                *out_page = SdDuplicateGroupPage {
                    groups: ptr,
                    count,
                    total_available: total as u32,
                };

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

/// Free a duplicate group page allocated by `sd_query_duplicate_groups`.
///
/// # Safety
/// `page` must have been returned by `sd_query_duplicate_groups`.
#[no_mangle]
pub unsafe extern "C" fn sd_free_duplicate_group_page(page: *mut SdDuplicateGroupPage) {
    if page.is_null() {
        return;
    }
    let page = &*page;
    if !page.groups.is_null() && page.count > 0 {
        let slice = std::slice::from_raw_parts_mut(page.groups, page.count as usize);
        drop(Box::from_raw(slice as *mut [SdDuplicateGroup]));
    }
}

/// Query files in a duplicate group.
///
/// # Safety
/// `out_page` must be a valid pointer. The returned page must be freed with `sd_free_file_record_page`.
#[no_mangle]
pub unsafe extern "C" fn sd_query_files_in_group(
    handle: u64,
    group_id: i64,
    out_page: *mut SdFileRecordPage,
) -> SdResultCode {
    if out_page.is_null() {
        set_last_error("out_page is null".to_string());
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

        match db.get_files_in_group(group_id) {
            Ok(files) => {
                let count = files.len() as u32;
                let c_files: Vec<SdFileRecord> = files
                    .iter()
                    .map(|f| SdFileRecord {
                        id: f.id,
                        canonical_path: rust_string_to_c(&f.canonical_path),
                        file_name: rust_string_to_c(&f.file_name),
                        parent_dir: rust_string_to_c(&f.parent_dir),
                        file_size: f.file_size,
                        content_hash: f.content_hash.unwrap_or(0),
                    })
                    .collect();

                let boxed = c_files.into_boxed_slice();
                let ptr = Box::into_raw(boxed) as *mut SdFileRecord;

                *out_page = SdFileRecordPage {
                    files: ptr,
                    count,
                };

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

/// Free a file record page allocated by `sd_query_files_in_group`.
///
/// # Safety
/// `page` must have been returned by `sd_query_files_in_group`.
#[no_mangle]
pub unsafe extern "C" fn sd_free_file_record_page(page: *mut SdFileRecordPage) {
    if page.is_null() {
        return;
    }
    let page = &*page;
    if !page.files.is_null() && page.count > 0 {
        let slice = std::slice::from_raw_parts_mut(page.files, page.count as usize);
        for file in slice.iter() {
            sd_free_string(file.canonical_path);
            sd_free_string(file.file_name);
            sd_free_string(file.parent_dir);
        }
        drop(Box::from_raw(slice as *mut [SdFileRecord]));
    }
}

// Re-export sd_free_string so it's accessible from this module
use crate::error::sd_free_string;
