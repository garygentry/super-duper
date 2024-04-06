mod config;
use config::AppConfig;
mod cli;
mod debug;
mod file_cache;
mod file_proc;
mod logging;
mod model;
mod test1;
mod utils;

// use app_config::AppConfig;
use clap::{CommandFactory, Parser};
use cli::{Cli, Commands};
use colored::*;
use dotenv::dotenv;
use std::{env, process, time::Instant};
use tracing::{error, info};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let _guard = logging::init_logger();

    let config = match AppConfig::load() {
        Ok(config) => config,
        Err(err) => {
            error!("Error loading configuration: {}", err);
            process::exit(1);
        }
    };

    let args = Cli::parse();

    match args.command {
        Some(Commands::Process) => {
            info!("Processing...");
            // if let Err(err) = run_process(&config) {
            //     error!("Error: {}", err);
            // }
        }
        Some(Commands::BuildPathParts) => {
            info!("Building path_part (HASH)");
            // db::part_path::dupe_file_to_part_path()?;
        }
        Some(Commands::CountHashCache) => {
            info!("Counting content cache hash...");
            // crate::file_proc::hash::hash_cache::print_count();
        }
        Some(Commands::PrintConfig) => {
            println!("Configuration: {:?}", config);
            println!("Environment variables:");
            println!(
                "DATABASE_URL: {:?}",
                env::var("DATABASE_URL").unwrap_or_default()
            );
        }
        Some(Commands::Test1) => {
            info!("Test1...");
            //test1::test_cache();
            // test1::test1();
        }
        Some(Commands::TestCache) => {
            info!("Test Cache...");
            //test1::test_cache();
            // test1::test1();
        }
        Some(Commands::TestScan) => {
            info!("Test Scan...");
            test1::test_scan(&config);
        }
        Some(Commands::TestHash) => {
            info!("Test Hash...");
            test1::test_hash(&config);
        }
        Some(Commands::TestPrintCache) => {
            info!("Test Print Cache...");
            file_cache::print_all_cache_values();
        }
        Some(Commands::TruncateDb) => {
            info!("TruncateDb...");
            // println!(
            //     "This command will WIPE OUT all Data in the database\nDATABASE_URL: {:?}",
            //     env::var("DATABASE_URL").unwrap_or_default()
            // );
            // match utils::prompt_confirm(
            //     "Are you SURE you want to COMPLETELY DELETE the Database?",
            //     Some(false),
            // ) {
            //     Ok(true) => {
            //         db::truncate_tables();
            //         println!("All tables truncated");
            //     }
            //     _ => {
            //         process::exit(0);
            //     }
            // }
        }
        Some(Commands::PrintFileCacheLen) => {
            info!("Test1...");
            match file_cache::get_file_cache_len() {
                Ok(count) => info!("Total keys in file cache: {}", count),
                Err(e) => error!("Error: {}", e),
            }
        }
        Some(Commands::PrintOldFileCacheLen) => {
            info!("Print Old Hash Cache Count...");
            match file_cache::old_cache::get_file_cache_len() {
                Ok(count) => info!("Total keys in OLD file cache: {}", count),
                Err(e) => error!("Error: {}", e),
            }
        }
        Some(Commands::MigrateOldCacheVersion) => {
            info!("Migrate old file cache version...");
            let start = Instant::now();
            file_cache::old_cache::migrate_old_cache_version();
            let duration = start.elapsed();
            info!(
                "Migrated old cache version in {} seconds",
                format_args!("{}", format!("{:.2}", &duration.as_secs_f64()).green()),
            );
        }

        None => {
            // Default no subcommand
            let _ = Cli::command().print_long_help();
        }
    }

    Ok(())
}
