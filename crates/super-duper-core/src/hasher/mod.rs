pub mod cache;
#[cfg(test)]
mod hash_stability;
pub(crate) mod repeat_cache;
mod scheduler;
pub mod xxhash;

pub use xxhash::{HashOutcome, build_content_hash_map, build_content_hash_map_with_stats};
#[allow(unused_imports)]
pub(crate) use xxhash::{
    HashPipelineIo, HashProgressDelta, HashProgressSink, SystemHashPipelineIo,
    build_content_hash_map_with_progress,
};
