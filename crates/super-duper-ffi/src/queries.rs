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
        let session_id = match state.active_session_id {
            Some(id) => id,
            None => {
                // No session yet — return empty page
                *out_page = SdDuplicateGroupPage {
                    groups: std::ptr::null_mut(),
                    count: 0,
                    total_available: 0,
                };
                return SdResultCode::Ok;
            }
        };

        let db = match &state.db {
            Some(db) => db,
            None => {
                set_last_error("No database open".to_string());
                return SdResultCode::DatabaseError;
            }
        };

        match db.get_duplicate_groups(session_id, offset, limit) {
            Ok(groups) => {
                let total: i64 = db.get_duplicate_group_count(session_id).unwrap_or(0);
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
                    .map(|f| {
                        let marked = db
                            .is_file_marked_for_deletion(f.id)
                            .unwrap_or(false);
                        SdFileRecord {
                            id: f.id,
                            canonical_path: rust_string_to_c(&f.canonical_path),
                            file_name: rust_string_to_c(&f.file_name),
                            parent_dir: rust_string_to_c(&f.parent_dir),
                            file_size: f.file_size,
                            content_hash: f.content_hash.unwrap_or(0),
                            is_marked_for_deletion: if marked { 1 } else { 0 },
                        }
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

/// Query directory children. Pass parent_id = -1 for root directories.
///
/// # Safety
/// `out_page` must be a valid pointer. The returned page must be freed with `sd_free_directory_node_page`.
#[no_mangle]
pub unsafe extern "C" fn sd_query_directory_children(
    handle: u64,
    parent_id: i64,
    offset: i64,
    limit: i64,
    out_page: *mut SdDirectoryNodePage,
) -> SdResultCode {
    if out_page.is_null() {
        set_last_error("out_page is null".to_string());
        return SdResultCode::InvalidArgument;
    }

    let parent = if parent_id < 0 { None } else { Some(parent_id) };

    let result = with_handle(handle, |state| {
        let db = match &state.db {
            Some(db) => db,
            None => {
                set_last_error("No database open".to_string());
                return SdResultCode::DatabaseError;
            }
        };

        match db.get_directory_children(parent, offset, limit) {
            Ok(nodes) => {
                let count = nodes.len() as u32;
                let c_nodes: Vec<SdDirectoryNode> = nodes
                    .iter()
                    .map(|n| SdDirectoryNode {
                        id: n.id,
                        path: rust_string_to_c(&n.path),
                        name: rust_string_to_c(&n.name),
                        parent_id: n.parent_id.unwrap_or(-1),
                        total_size: n.total_size,
                        file_count: n.file_count,
                        depth: n.depth,
                    })
                    .collect();

                let boxed = c_nodes.into_boxed_slice();
                let ptr = Box::into_raw(boxed) as *mut SdDirectoryNode;

                *out_page = SdDirectoryNodePage {
                    nodes: ptr,
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

/// Free a directory node page allocated by `sd_query_directory_children`.
///
/// # Safety
/// `page` must have been returned by `sd_query_directory_children`.
#[no_mangle]
pub unsafe extern "C" fn sd_free_directory_node_page(page: *mut SdDirectoryNodePage) {
    if page.is_null() {
        return;
    }
    let page = &*page;
    if !page.nodes.is_null() && page.count > 0 {
        let slice = std::slice::from_raw_parts_mut(page.nodes, page.count as usize);
        for node in slice.iter() {
            sd_free_string(node.path);
            sd_free_string(node.name);
        }
        drop(Box::from_raw(slice as *mut [SdDirectoryNode]));
    }
}

/// Query similar directory pairs above a minimum score.
///
/// # Safety
/// `out_page` must be a valid pointer. The returned page must be freed with `sd_free_directory_similarity_page`.
#[no_mangle]
pub unsafe extern "C" fn sd_query_similar_directories(
    handle: u64,
    min_score: f64,
    offset: i64,
    limit: i64,
    out_page: *mut SdDirectorySimilarityPage,
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

        match db.get_similar_directories(min_score, offset, limit) {
            Ok(pairs) => {
                let count = pairs.len() as u32;
                let c_pairs: Vec<SdDirectorySimilarity> = pairs
                    .iter()
                    .map(|p| SdDirectorySimilarity {
                        id: p.id,
                        dir_a_id: p.dir_a_id,
                        dir_b_id: p.dir_b_id,
                        dir_a_path: rust_string_to_c(&p.dir_a_path),
                        dir_b_path: rust_string_to_c(&p.dir_b_path),
                        similarity_score: p.similarity_score,
                        shared_bytes: p.shared_bytes,
                        match_type: rust_string_to_c(&p.match_type),
                    })
                    .collect();

                let boxed = c_pairs.into_boxed_slice();
                let ptr = Box::into_raw(boxed) as *mut SdDirectorySimilarity;

                *out_page = SdDirectorySimilarityPage {
                    pairs: ptr,
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

/// Free a directory similarity page allocated by `sd_query_similar_directories`.
///
/// # Safety
/// `page` must have been returned by `sd_query_similar_directories`.
#[no_mangle]
pub unsafe extern "C" fn sd_free_directory_similarity_page(page: *mut SdDirectorySimilarityPage) {
    if page.is_null() {
        return;
    }
    let page = &*page;
    if !page.pairs.is_null() && page.count > 0 {
        let slice = std::slice::from_raw_parts_mut(page.pairs, page.count as usize);
        for pair in slice.iter() {
            sd_free_string(pair.dir_a_path);
            sd_free_string(pair.dir_b_path);
            sd_free_string(pair.match_type);
        }
        drop(Box::from_raw(slice as *mut [SdDirectorySimilarity]));
    }
}

/// List scan sessions with pagination, ordered newest-first.
/// `is_active` is set to 1 for the session matching the handle's active_session_id.
///
/// # Safety
/// `out_page` must be a valid pointer. The returned page must be freed with `sd_free_session_page`.
#[no_mangle]
pub unsafe extern "C" fn sd_list_sessions(
    handle: u64,
    offset: i64,
    limit: i64,
    out_page: *mut SdSessionPage,
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
        let active_id = state.active_session_id;

        match db.list_sessions(offset, limit) {
            Ok((sessions, total)) => {
                let count = sessions.len() as u32;

                let c_sessions: Vec<SdSessionInfo> = sessions
                    .iter()
                    .map(|(s, group_count)| SdSessionInfo {
                        id: s.id,
                        started_at: rust_string_to_c(&s.started_at),
                        completed_at: s
                            .completed_at
                            .as_deref()
                            .map(rust_string_to_c)
                            .unwrap_or(std::ptr::null_mut()),
                        status: rust_string_to_c(&s.status),
                        root_paths: rust_string_to_c(&s.root_paths),
                        files_scanned: s.files_scanned,
                        total_bytes: s.total_bytes,
                        group_count: *group_count,
                        is_active: if active_id == Some(s.id) { 1 } else { 0 },
                    })
                    .collect();

                let boxed = c_sessions.into_boxed_slice();
                let ptr = Box::into_raw(boxed) as *mut SdSessionInfo;

                *out_page = SdSessionPage {
                    sessions: ptr,
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

/// Free a session page allocated by `sd_list_sessions`.
///
/// # Safety
/// `page` must have been returned by `sd_list_sessions`.
#[no_mangle]
pub unsafe extern "C" fn sd_free_session_page(page: *mut SdSessionPage) {
    if page.is_null() {
        return;
    }
    let page = &*page;
    if !page.sessions.is_null() && page.count > 0 {
        let slice = std::slice::from_raw_parts_mut(page.sessions, page.count as usize);
        for session in slice.iter() {
            sd_free_string(session.started_at);
            if !session.completed_at.is_null() {
                sd_free_string(session.completed_at);
            }
            sd_free_string(session.status);
            sd_free_string(session.root_paths);
        }
        drop(Box::from_raw(slice as *mut [SdSessionInfo]));
    }
}

// Re-export sd_free_string so it's accessible from this module
use crate::error::sd_free_string;
