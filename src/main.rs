mod app_config;
mod cli;
mod db;
mod file_proc;
mod logging;
mod model;
mod utils;

use std::{env, process};

use app_config::AppConfig;
use clap::{CommandFactory, Parser};
use cli::{Cli, Commands};
use dotenv::dotenv;
use tracing::{error, info};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let _guard = logging::init_logger();

    let config = match app_config::load_configuration() {
        Ok(config) => config,
        Err(err) => {
            error!("Error loading configuration: {}", err);
            process::exit(1);
        }
    };

    utils::hide_cursor();

    let args = Cli::parse();

    match args.command {
        Some(Commands::Process) => {
            if let Err(err) = run_process(&config) {
                error!("Error: {}", err);
            }
        }
        Some(Commands::BuildPathParts) => {
            info!("Building path_part");
            // let _ = db::part_path::dupe_file_to_part_path();
        }
        Some(Commands::BuildPathPartsHash) => {
            info!("Building path_part (HASH)");
            db::part_path_hash::dupe_file_to_part_path()?;
        }
        Some(Commands::CountHashCache) => {
            info!("Counting content cache hash...");
            crate::file_proc::hash::hash_cache::print_count();
        }
        Some(Commands::PrintConfig) => {
            println!("Configuration: {:?}", config);
            println!("Environment variables:");
            println!(
                "DATABASE_URL: {:?}",
                env::var("DATABASE_URL").unwrap_or_default()
            );
        }
        Some(Commands::Baz(args)) => match args.blat {
            Some(blat) => info!("Handling 'baz' subcommand with blat = {}", blat),
            None => info!("Handling 'baz' subcommand without blat"),
        },
        None => {
            // Default no subcommand
            let _ = Cli::command().print_long_help();
        }
    }

    utils::show_cursor();

    Ok(())
}

fn run_process(config: &AppConfig) -> Result<(), String> {
    let non_overlapping = utils::non_overlapping_directories(config.root_paths.clone());
    info!("Processing directories: {:?}", non_overlapping);
    file_proc::process(&non_overlapping, &config.ignore_patterns)
        .map_err(|err| format!("Error processing files: {}", err))?;

    Ok(())
}
