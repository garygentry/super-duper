mod config;
use config::AppConfig;
mod cli;
mod db;
mod debug;
mod file_cache;
mod file_proc;
mod logging;
mod model;
mod test1;
mod utils;
mod scan_and_store;

// use app_config::AppConfig;
use clap::{ CommandFactory, Parser };
use cli::{ Cli, Commands };
use dotenv::dotenv;
use std::{ env, process };
use tracing::{ error, info };

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
            scan_and_store
                ::scan_and_store_dupes(config.root_paths, config.ignore_patterns)
                .map_err(|err| format!("Error processing files: {}", err))?;

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
            file_cache::print_count();
            // crate::file_proc::hash::hash_cache::print_count();
        }
        Some(Commands::PrintConfig) => {
            println!("Configuration: {:?}", config);
            println!("Environment variables:");
            println!("DATABASE_URL: {:?}", env::var("DATABASE_URL").unwrap_or_default());
        }
        Some(Commands::Test1) => {
            info!("Test1...");
            let spinner = file_proc::status::progress_bars::load_spinner_frames("dots".to_string());
            println!("Spinner: {:?}", spinner);
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
            // test1::test_scan(&config);
        }
        Some(Commands::TestHash) => {
            info!("Test Hash...");
            // test1::test_hash(&config);
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
        None => {
            // Default no subcommand
            let _ = Cli::command().print_long_help();
        }
    }

    Ok(())
}
