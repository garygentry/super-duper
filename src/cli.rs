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
    /// A subcommand for doing baz which has an optional argument 'blat'
    // Baz(BazArgs),
    Test1,
}

#[derive(Debug, Parser)]
pub struct BazArgs {
    /// An optional argument for the baz subcommand
    #[arg(long, short)]
    pub truncate: bool,
}
