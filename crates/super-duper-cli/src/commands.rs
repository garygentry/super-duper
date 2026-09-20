use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "super-duper")]
#[command(about = "A super duper deduper", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Output for a machine-readable command. `Text` (the default) keeps the existing human-readable
/// output; `Json` prints one JSON value to stdout instead, for scripting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
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
    AnalyzeDirectories {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    /// Display the number of keys in the hash cache
    CountHashCache {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    /// Print configuration values
    PrintConfig {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    /// Truncate all database tables
    TruncateDb,
    /// Export duplicate groups or session data (docs/export-format-v1.md)
    Export {
        #[command(subcommand)]
        kind: ExportKind,
    },
    /// Mark duplicate files for deletion, keeping one survivor per group by `strategy`
    AutoMark {
        /// Run to mark; defaults to the latest completed run
        #[arg(long)]
        run: Option<i64>,
        #[arg(long, value_enum, default_value_t = AutoMarkStrategyArg::KeepFirst)]
        strategy: AutoMarkStrategyArg,
        /// Required (and only used) when --strategy=preferred-path-prefix: the file kept per
        /// group is the one whose canonical path starts with this prefix, falling back to
        /// keep-first when no member matches
        #[arg(long)]
        prefix: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}

/// CLI-facing survivor-selection rule for `auto-mark` (`analysis::deletion_plan::AutoMarkStrategy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AutoMarkStrategyArg {
    KeepFirst,
    KeepNewest,
    KeepOldest,
    PreferredPathPrefix,
}

impl AutoMarkStrategyArg {
    /// The name `AutoMarkStrategy::parse` expects.
    pub fn as_str(&self) -> &'static str {
        match self {
            AutoMarkStrategyArg::KeepFirst => "keep_first",
            AutoMarkStrategyArg::KeepNewest => "keep_newest",
            AutoMarkStrategyArg::KeepOldest => "keep_oldest",
            AutoMarkStrategyArg::PreferredPathPrefix => "preferred_path_prefix",
        }
    }
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
