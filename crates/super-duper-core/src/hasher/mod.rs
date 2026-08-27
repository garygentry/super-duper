pub mod cache;
#[cfg(test)]
mod read_path;
pub(crate) mod repeat_cache;
#[cfg(all(test, target_os = "windows"))]
mod repeat_profile;
mod scheduler;
pub mod xxhash;

pub use xxhash::{build_content_hash_map, build_content_hash_map_with_stats, HashOutcome};
#[allow(unused_imports)]
pub(crate) use xxhash::{
    build_content_hash_map_with_progress, HashPipelineIo, HashProgressDelta, HashProgressSink,
    SystemHashPipelineIo,
};
