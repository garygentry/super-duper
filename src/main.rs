mod app_config;
mod cli;
mod db;
mod file_proc;
mod logging;
mod model;
mod status;
mod utils;

use clap::Parser;
use cli::{Cli, Commands};
use dotenv::dotenv;
use tracing::{debug, error, info};

fn main() {
    dotenv().ok();

    let _guard = logging::init_logger();

    utils::hide_cursor();

    let args = Cli::parse();

    match args.command {
        Some(Commands::Process) => {
            if let Err(err) = run_process() {
                error!("Error: {}", err);
            }
        }
        Some(Commands::Bar(args)) => {
            // Use args.blah as needed
            println!("Handling 'bar' subcommand with blah = {}", args.blah);
        }
        Some(Commands::Baz(args)) => {
            // Check if 'blat' was provided and use accordingly
            match args.blat {
                Some(blat) => println!("Handling 'baz' subcommand with blat = {}", blat),
                None => println!("Handling 'baz' subcommand without blat"),
            }
        }
        None => {
            // Call your default function here
            //println!("No subcommand was used, calling the default function");
            // let _ = Cli::command().print_long_help();
            // let _ = db::test1::test1();
            let _ = db::part_path::dupe_file_to_part_path();
        }
    }

    // crate::file_proc::hash::hash_cache::print_hash_cache_count();

    // if args.foo {
    //     println!("Foo");
    // } else if let Err(err) = run_app() {
    //     error!("Error: {}", err);
    // }

    // crate::file_proc::hash::hash_cache::print_hash_cache_count();

    utils::show_cursor();
}

fn run_process() -> Result<(), String> {
    let config = app_config::load_configuration()
        .map_err(|err| format!("Error loading configuration: {}", err))?;
    debug!("config.root_paths: {:?}", config.root_paths);
    debug!("config.ignore_patterns: {:?}", config.ignore_patterns);
    let non_overlapping = utils::non_overlapping_directories(config.root_paths);
    info!("Processing directories: {:?}", non_overlapping);
    file_proc::process(&non_overlapping, &config.ignore_patterns)
        .map_err(|err| format!("Error processing files: {}", err))?;

    // file_proc::process_dir(&non_overlapping)
    //     .map_err(|err| format!("Error processing files: {}", err))?;

    Ok(())
}
