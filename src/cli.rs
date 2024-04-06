use clap::{Parser, Subcommand};

#[derive(Debug, Parser)] // requires `derive` feature
#[command(name = "super-duper")]
#[command(about = "A super duper deduper", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Perform full process on input paths
    #[command()]
    Process,
    /// Create part_part from dupe_file in db
    BuildPathParts,
    /// Display the number of keys in the cache hash
    CountHashCache,
    /// Print configuration values
    PrintConfig,
    /// Truncate Database tables
    TruncateDb,
    /// Test1
    Test1,
    /// Test Cache
    TestCache,
    /// Test Cache
    TestScan,
    /// Test Hash
    TestHash,
    /// Test Print Cache
    TestPrintCache,
    /// Print number of keys in file cache
    PrintFileCacheLen,
    /// Count old hash cache
    PrintOldFileCacheLen,
    /// Migrate old cache version to new cache
    MigrateOldCacheVersion,
}
