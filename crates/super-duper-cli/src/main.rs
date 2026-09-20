mod commands;
mod logging;
mod progress;

use std::io::{self, Write};
use std::process;

use clap::{CommandFactory, Parser};
use colored::*;
use commands::{Cli, Commands, ExportKind, OutputFormat};
use dotenv::dotenv;
use progress::CliReporter;
use super_duper_core::ScanEngine;
use super_duper_core::analysis::deletion_plan::AutoMarkStrategy;
use super_duper_core::export;
use tracing::{error, info};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let _guard = logging::init_logger();

    let config = match super_duper_core::config::load_configuration() {
        Ok(config) => config,
        Err(err) => {
            error!("Error loading configuration: {}", err);
            process::exit(1);
        }
    };

    let args = Cli::parse();

    match args.command {
        Some(Commands::Process) => {
            if let Err(err) = run_process(&config) {
                error!("Error: {}", err);
            }
        }
        Some(Commands::AnalyzeDirectories { format }) => {
            if let Err(err) = run_analyze_directories(&config, format) {
                error!("Error: {}", err);
                process::exit(1);
            }
        }
        Some(Commands::CountHashCache { format }) => {
            let cache = super_duper_core::hasher::cache::default_hash_cache_path();
            match format {
                OutputFormat::Text => {
                    info!("Counting content cache hash...");
                    super_duper_core::hasher::cache::print_count(&cache);
                }
                OutputFormat::Json => {
                    match super_duper_core::hasher::cache::count_entries(&cache) {
                        Ok(count) => println!(
                            "{}",
                            serde_json::json!({"path": cache.to_string_lossy(), "entries": count})
                        ),
                        Err(err) => {
                            error!("Error counting hash cache entries: {}", err);
                            process::exit(1);
                        }
                    }
                }
            }
        }
        Some(Commands::TrimHashCache {
            unseen_scans,
            format,
        }) => {
            let cache = super_duper_core::hasher::cache::default_hash_cache_path();
            match super_duper_core::hasher::cache::trim(&cache, unseen_scans) {
                // `trim` already logs a human-readable summary at info level.
                Ok(_) if format == OutputFormat::Text => {}
                Ok(report) => println!(
                    "{}",
                    serde_json::json!({
                        "path": cache.to_string_lossy(),
                        "unseenScans": unseen_scans,
                        "liveEntriesBefore": report.live_entries_before,
                        "removed": report.removed,
                    })
                ),
                Err(err) => {
                    error!("Error trimming hash cache: {}", err);
                    process::exit(1);
                }
            }
        }
        Some(Commands::PrintConfig { format }) => match format {
            OutputFormat::Text => println!("Configuration: {:?}", config),
            OutputFormat::Json => match config.to_json_pretty() {
                Ok(json) => println!("{json}"),
                Err(err) => {
                    error!("Error serializing configuration: {}", err);
                    process::exit(1);
                }
            },
        },
        Some(Commands::Export { kind }) => {
            if let Err(err) = run_export(kind) {
                error!("Error: {}", err);
                process::exit(1);
            }
        }
        Some(Commands::AutoMark {
            run,
            strategy,
            prefix,
            format,
        }) => {
            if let Err(err) = run_auto_mark(run, strategy, prefix, format) {
                error!("Error: {}", err);
                process::exit(1);
            }
        }
        Some(Commands::TruncateDb) => {
            match prompt_confirm(
                "Are you SURE you want to COMPLETELY DELETE the Database?",
                Some(false),
            ) {
                Ok(true) => match super_duper_core::storage::Database::open("super_duper.db") {
                    Ok(db) => match db.truncate_all() {
                        Err(e) => {
                            error!("Error truncating database: {}", e);
                        }
                        _ => {
                            println!("All tables truncated");
                        }
                    },
                    Err(e) => error!("Error opening database: {}", e),
                },
                _ => {
                    process::exit(0);
                }
            }
        }
        None => {
            let _ = Cli::command().print_long_help();
        }
    }

    Ok(())
}

fn run_process(config: &super_duper_core::AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    let engine = ScanEngine::new(config.clone());
    let reporter = CliReporter::new();
    let result = engine.scan(&reporter)?;

    println!();
    info!(
        "Session {}, immutable run {}",
        result.session_id, result.run_id
    );
    info!(
        "Scan: {}, Hash: {}, DB: {}, Dir: {}",
        format!("{:.2}s", result.scan_duration.as_secs_f64()).green(),
        format!("{:.2}s", result.hash_duration.as_secs_f64()).green(),
        format!("{:.2}s", result.db_write_duration.as_secs_f64()).green(),
        format!("{:.2}s", result.dir_analysis_duration.as_secs_f64()).green(),
    );
    info!(
        "{} duplicate groups, {} files with duplicates, {} bytes wasted",
        format!("{}", result.duplicate_groups).red(),
        format!("{}", result.duplicate_files).red(),
        format!("{}", result.wasted_bytes).red(),
    );
    info!(
        "{} files / {} bytes discovered, {} files hashed, {} warnings",
        result.total_files_scanned,
        result.total_bytes_discovered,
        result.files_hashed,
        result.warning_count,
    );
    info!(
        "{} directory fingerprints, {} similar directory pairs",
        format!("{}", result.dir_fingerprints).cyan(),
        format!("{}", result.dir_similarity_pairs).cyan(),
    );

    Ok(())
}

fn run_analyze_directories(
    config: &super_duper_core::AppConfig,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = super_duper_core::storage::Database::open("super_duper.db")?;
    let run_id = db
        .get_latest_completed_run_id()?
        .ok_or("No completed run is available for directory analysis")?;

    let text = format == OutputFormat::Text;
    if text {
        info!("Building directory fingerprints...");
    }
    let fingerprint_count =
        super_duper_core::analysis::dir_fingerprint::build_directory_fingerprints(&db, run_id)?;
    if text {
        info!("{} directory fingerprints computed", fingerprint_count);
        info!("Computing directory similarity...");
    }
    let similarity_count =
        super_duper_core::analysis::dir_similarity::compute_directory_similarity(
            &db,
            run_id,
            config.directory_similarity_threshold,
            config.directory_similarity_noise_cutoff,
        )?;
    if text {
        info!("{} similar directory pairs found", similarity_count);
    } else {
        println!(
            "{}",
            serde_json::json!({
                "runId": run_id,
                "directoryFingerprints": fingerprint_count,
                "similarDirectoryPairs": similarity_count,
            })
        );
    }

    Ok(())
}

fn run_export(kind: ExportKind) -> Result<(), Box<dyn std::error::Error>> {
    let db = super_duper_core::storage::Database::open("super_duper.db")?;
    let output = match kind {
        ExportKind::DuplicateGroups { run, format } => {
            export::export_duplicate_groups(&db, run, format.into())?
        }
        ExportKind::Sessions { session, format } => {
            export::export_sessions(&db, session, format.into())?
        }
    };
    // The CSV form already ends in a line terminator; JSON does not. Emit exactly one either way.
    print!("{output}");
    if !output.ends_with('\n') {
        println!();
    }
    Ok(())
}

fn run_auto_mark(
    run: Option<i64>,
    strategy: commands::AutoMarkStrategyArg,
    prefix: Option<String>,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = super_duper_core::storage::Database::open("super_duper.db")?;
    let run_id = match run {
        Some(run_id) => {
            db.get_scan_run(run_id)
                .map_err(|_| format!("run {run_id} not found"))?;
            run_id
        }
        None => db
            .get_latest_completed_run_id()?
            .ok_or("No completed run is available for auto-mark")?,
    };
    let strategy = AutoMarkStrategy::parse(strategy.as_str(), prefix.as_deref())?;

    let marked =
        super_duper_core::analysis::deletion_plan::auto_mark_duplicates(&db, run_id, &strategy)?;

    match format {
        OutputFormat::Text => info!("Marked {} files for deletion (run {})", marked, run_id),
        OutputFormat::Json => println!(
            "{}",
            serde_json::json!({"runId": run_id, "markedCount": marked})
        ),
    }

    Ok(())
}

fn prompt_confirm(prompt: &str, default: Option<bool>) -> io::Result<bool> {
    let mut input = String::new();

    loop {
        input.clear();

        match default {
            Some(true) => print!("{} (Y/n): ", prompt),
            Some(false) | None => print!("{} (y/N): ", prompt),
        }
        io::stdout().flush()?;

        io::stdin().read_line(&mut input)?;

        match input.trim().to_uppercase().as_str() {
            "Y" => return Ok(true),
            "N" => return Ok(false),
            "" => match default {
                Some(default) => return Ok(default),
                None => continue,
            },
            _ => continue,
        }
    }
}
