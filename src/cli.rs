use clap::{Args, Parser, Subcommand, ValueEnum};

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
    /// A subcommand for doing bar which requires an argument 'blah'
    Bar(BarArgs),
    /// A subcommand for doing baz which has an optional argument 'blat'
    Baz(BazArgs),
}

#[derive(Debug, Parser)]
pub struct BarArgs {
    /// An argument required for the bar subcommand
    #[clap(value_parser)]
    pub blah: String,
}

#[derive(Debug, Parser)]
pub struct BazArgs {
    /// An optional argument for the baz subcommand
    #[clap(value_parser)]
    pub blat: Option<String>,
}
