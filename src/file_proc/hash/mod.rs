use dashmap::DashMap;
use std::io;
use std::path::PathBuf;

pub mod builders;

pub fn build_content_hash_map(
    size_to_file_map: DashMap<u64, Vec<PathBuf>>,
) -> io::Result<DashMap<u64, Vec<PathBuf>>> {
    build_content_hash_map_custom(size_to_file_map, builders::default::build_content_hash_map)
}

pub fn build_content_hash_map_custom<F>(
    size_to_file_map: DashMap<u64, Vec<PathBuf>>,
    builder: F,
) -> io::Result<DashMap<u64, Vec<PathBuf>>>
where
    F: FnOnce(DashMap<u64, Vec<PathBuf>>) -> io::Result<DashMap<u64, Vec<PathBuf>>>,
{
    builder(size_to_file_map)
}
