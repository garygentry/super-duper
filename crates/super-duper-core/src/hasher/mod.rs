pub mod cache;
#[cfg(test)]
mod hash_stability;
#[cfg(test)]
mod read_path;
pub(crate) mod repeat_cache;
#[cfg(all(test, target_os = "windows"))]
mod repeat_profile;
mod scheduler;
pub mod xxhash;

pub use xxhash::{HashOutcome, build_content_hash_map, build_content_hash_map_with_stats};
#[allow(unused_imports)]
pub(crate) use xxhash::{
    HashPipelineIo, HashProgressDelta, HashProgressSink, SystemHashPipelineIo,
    build_content_hash_map_with_progress,
};
