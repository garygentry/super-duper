use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "super-duper")]
#[command(about = "A super duper deduper", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// The file format for an `export` document (`docs/export-format-v1.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExportFileFormat {
    Csv,
    Json,
}

impl From<ExportFileFormat> for super_duper_core::export::ExportFormat {
    fn from(format: ExportFileFormat) -> Self {
        match format {
            ExportFileFormat::Csv => super_duper_core::export::ExportFormat::Csv,
            ExportFileFormat::Json => super_duper_core::export::ExportFormat::Json,
        }
    }
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
    /// Export duplicate groups or session data (docs/export-format-v1.md)
    Export {
        #[command(subcommand)]
        kind: ExportKind,
    },
}

#[derive(Debug, Subcommand)]
pub enum ExportKind {
    /// Every duplicate file group and its members for one run
    DuplicateGroups {
        /// Run to export; defaults to the latest completed run
        #[arg(long)]
        run: Option<i64>,
        #[arg(long, value_enum, default_value_t = ExportFileFormat::Csv)]
        format: ExportFileFormat,
    },
    /// Session definitions and their run history
    Sessions {
        /// Session to export; defaults to every session
        #[arg(long)]
        session: Option<i64>,
        #[arg(long, value_enum, default_value_t = ExportFileFormat::Csv)]
        format: ExportFileFormat,
    },
}
