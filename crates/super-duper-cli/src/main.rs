mod commands;
mod logging;
mod progress;

use std::io::{self, Write};
use std::process;

use clap::{CommandFactory, Parser};
use colored::*;
use commands::{Cli, Commands};
use dotenv::dotenv;
use progress::CliReporter;
use super_duper_core::ScanEngine;
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
        Some(Commands::AnalyzeDirectories) => {
            if let Err(err) = run_analyze_directories() {
                error!("Error: {}", err);
            }
        }
        Some(Commands::CountHashCache) => {
            info!("Counting content cache hash...");
            super_duper_core::hasher::cache::print_count();
        }
        Some(Commands::PrintConfig) => {
            println!("Configuration: {:?}", config);
        }
        Some(Commands::TruncateDb) => {
            match prompt_confirm(
                "Are you SURE you want to COMPLETELY DELETE the Database?",
                Some(false),
            ) {
                Ok(true) => {
                    match super_duper_core::storage::Database::open("super_duper.db") {
                        Ok(db) => {
                            if let Err(e) = db.truncate_all() {
                                error!("Error truncating database: {}", e);
                            } else {
                                println!("All tables truncated");
                            }
                        }
                        Err(e) => error!("Error opening database: {}", e),
                    }
                }
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

fn run_process(
    config: &super_duper_core::AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let engine = ScanEngine::new(config.clone());
    let reporter = CliReporter::new();
    let result = engine.scan(&reporter)?;

    println!();
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
        "{} directory fingerprints, {} similar directory pairs",
        format!("{}", result.dir_fingerprints).cyan(),
        format!("{}", result.dir_similarity_pairs).cyan(),
    );

    Ok(())
}

fn run_analyze_directories() -> Result<(), Box<dyn std::error::Error>> {
    let db = super_duper_core::storage::Database::open("super_duper.db")?;

    info!("Building directory fingerprints...");
    let fingerprint_count =
        super_duper_core::analysis::dir_fingerprint::build_directory_fingerprints(&db)?;
    info!("{} directory fingerprints computed", fingerprint_count);

    info!("Computing directory similarity...");
    let similarity_count =
        super_duper_core::analysis::dir_similarity::compute_directory_similarity(&db, 0.5)?;
    info!("{} similar directory pairs found", similarity_count);

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
