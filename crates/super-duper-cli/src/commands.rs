use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "super-duper")]
#[command(about = "A super duper deduper", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Perform full duplicate detection process on configured paths
    Process,
    /// Build directory fingerprints and compute similarity
    AnalyzeDirectories,
    /// Display the number of keys in the hash cache
    CountHashCache,
    /// Print configuration values
    PrintConfig,
    /// Truncate all database tables
    TruncateDb,
}
