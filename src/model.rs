use std::fs::Metadata;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ScanFile {
    pub path_buf: PathBuf,
    pub file_size: i64,
    pub metadata: Metadata,
}
