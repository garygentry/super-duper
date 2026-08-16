use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSession {
    pub id: i64,
    pub name: String,
    pub roots_json: String,
    pub ignore_patterns_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunParameters {
    pub roots: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub directory_similarity_threshold_millis: u16,
}

impl RunParameters {
    pub fn from_json(value: &str) -> Option<Self> {
        serde_json::from_str(value).ok()
    }

    pub fn roots_json(&self) -> String {
        serde_json::to_string(&self.roots).unwrap_or_else(|_| "[]".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRun {
    pub id: i64,
    pub session_id: i64,
    pub parameters_json: String,
    pub status: String,
    pub phase: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub files_discovered: i64,
    pub bytes_discovered: i64,
    pub files_hashed: i64,
    pub duplicate_file_groups: i64,
    pub duplicate_folder_groups: i64,
    pub wasted_bytes: i64,
    pub warning_count: i64,
    pub error_message: Option<String>,
    pub engine_version: String,
}

#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub id: i64,
    pub run_id: i64,
    pub root_path: String,
    pub canonical_path: String,
    pub relative_path: String,
    pub file_name: String,
    pub parent_dir: String,
    pub drive_letter: String,
    pub file_size: i64,
    pub last_modified: i64,
    pub partial_hash: Option<i64>,
    pub content_hash: Option<i64>,
    pub file_identity: Option<String>,
    pub warning_message: Option<String>,
    pub marked_deleted: bool,
}

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub id: i64,
    pub run_id: i64,
    pub content_hash: i64,
    pub file_size: i64,
    pub file_count: i64,
    pub wasted_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct DuplicateGroupMember {
    pub id: i64,
    pub group_id: i64,
    pub file_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateFileGroupSortField {
    RecoverableBytes,
    GroupSize,
    CopyCount,
    RepresentativeName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateFileMemberSortField {
    Path,
    ModifiedTime,
    Size,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageCursorValue {
    Integer(i64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageCursor {
    pub value: PageCursorValue,
    pub id: i64,
    pub before: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileGroupFilter {
    pub search: Option<String>,
    pub minimum_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileGroupPageQuery {
    pub run_id: i64,
    pub limit: i64,
    pub sort_field: DuplicateFileGroupSortField,
    pub sort_direction: SortDirection,
    pub filter: DuplicateFileGroupFilter,
    pub cursor: Option<PageCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileGroupResult {
    pub id: i64,
    pub run_id: i64,
    pub file_size: i64,
    pub file_count: i64,
    pub recoverable_bytes: i64,
    pub representative_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileGroupPage {
    pub groups: Vec<DuplicateFileGroupResult>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileMemberFilter {
    pub search: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileMemberPageQuery {
    pub run_id: i64,
    pub group_id: i64,
    pub limit: i64,
    pub sort_field: DuplicateFileMemberSortField,
    pub sort_direction: SortDirection,
    pub filter: DuplicateFileMemberFilter,
    pub cursor: Option<PageCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileMemberResult {
    pub id: i64,
    pub group_id: i64,
    pub canonical_path: String,
    pub file_name: String,
    pub parent_dir: String,
    pub file_size: i64,
    pub last_modified: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFileMemberPage {
    pub members: Vec<DuplicateFileMemberResult>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateFolderGroupSortField {
    TotalBytes,
    CopyCount,
    FileCount,
    RepresentativePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateFolderMemberSortField {
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderGroupFilter {
    pub search: Option<String>,
    pub minimum_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderGroupPageQuery {
    pub run_id: i64,
    pub limit: i64,
    pub sort_field: DuplicateFolderGroupSortField,
    pub sort_direction: SortDirection,
    pub filter: DuplicateFolderGroupFilter,
    pub cursor: Option<PageCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderGroupResult {
    pub id: i64,
    pub run_id: i64,
    pub total_size: i64,
    pub file_count: i64,
    pub folder_count: i64,
    pub representative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderGroupPage {
    pub groups: Vec<DuplicateFolderGroupResult>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderMemberFilter {
    pub search: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderMemberPageQuery {
    pub run_id: i64,
    pub group_id: i64,
    pub limit: i64,
    pub sort_field: DuplicateFolderMemberSortField,
    pub sort_direction: SortDirection,
    pub filter: DuplicateFolderMemberFilter,
    pub cursor: Option<PageCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderMemberResult {
    pub id: i64,
    pub group_id: i64,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFolderMemberPage {
    pub members: Vec<DuplicateFolderMemberResult>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct ExactFolderGroupInsert {
    pub structural_fingerprint: String,
    pub verified_fingerprint: String,
    pub total_size: i64,
    pub file_count: i64,
    pub directory_ids: Vec<i64>,
    pub is_suppressed: bool,
}

#[derive(Debug, Clone)]
pub struct DirectoryNode {
    pub id: i64,
    pub run_id: i64,
    pub path: String,
    pub name: String,
    pub parent_id: Option<i64>,
    pub total_size: i64,
    pub file_count: i64,
    pub depth: i64,
}

#[derive(Debug, Clone)]
pub struct DirectoryFingerprint {
    pub id: i64,
    pub directory_id: i64,
    pub content_fingerprint: String,
    pub file_hash_set: String,
}

#[derive(Debug, Clone)]
pub struct DirectorySimilarity {
    pub id: i64,
    pub run_id: i64,
    pub dir_a_id: i64,
    pub dir_b_id: i64,
    pub dir_a_path: String,
    pub dir_b_path: String,
    pub similarity_score: f64,
    pub shared_bytes: i64,
    pub match_type: String,
}

#[derive(Debug, Clone)]
pub struct DeletionPlanEntry {
    pub id: i64,
    pub file_id: i64,
    pub marked_at: String,
    pub strategy: Option<String>,
    pub executed_at: Option<String>,
    pub execution_result: Option<String>,
}
