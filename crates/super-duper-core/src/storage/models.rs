/// Represents a scan session — one invocation of the scan pipeline.
#[derive(Debug, Clone)]
pub struct ScanSession {
    pub id: i64,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: String,
    pub root_paths: String,
    pub files_scanned: i64,
    pub total_bytes: i64,
}

/// A file discovered during scanning.
#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub id: i64,
    pub canonical_path: String,
    pub file_name: String,
    pub parent_dir: String,
    pub drive_letter: String,
    pub file_size: i64,
    pub last_modified: i64,
    pub partial_hash: Option<i64>,
    pub content_hash: Option<i64>,
    pub last_seen_session_id: Option<i64>,
    pub marked_deleted: bool,
}

/// A group of files sharing the same content hash and size, scoped to a session.
#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub id: i64,
    pub session_id: i64,
    pub content_hash: i64,
    pub file_size: i64,
    pub file_count: i64,
    pub wasted_bytes: i64,
}

/// Links a scanned file to its duplicate group.
#[derive(Debug, Clone)]
pub struct DuplicateGroupMember {
    pub id: i64,
    pub group_id: i64,
    pub file_id: i64,
}

/// A node in the directory hierarchy tree.
#[derive(Debug, Clone)]
pub struct DirectoryNode {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub parent_id: Option<i64>,
    pub total_size: i64,
    pub file_count: i64,
    pub depth: i64,
}

/// Directory fingerprint for exact and similarity matching.
#[derive(Debug, Clone)]
pub struct DirectoryFingerprint {
    pub id: i64,
    pub directory_id: i64,
    pub content_fingerprint: String,
    pub file_hash_set: String,
}

/// Pre-computed similarity between two directories.
#[derive(Debug, Clone)]
pub struct DirectorySimilarity {
    pub id: i64,
    pub dir_a_id: i64,
    pub dir_b_id: i64,
    pub dir_a_path: String,
    pub dir_b_path: String,
    pub similarity_score: f64,
    pub shared_bytes: i64,
    pub match_type: String,
}

/// A file marked for deletion.
#[derive(Debug, Clone)]
pub struct DeletionPlanEntry {
    pub id: i64,
    pub file_id: i64,
    pub marked_at: String,
    pub strategy: Option<String>,
    pub executed_at: Option<String>,
    pub execution_result: Option<String>,
}
