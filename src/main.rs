use std::{env, panic};
mod app_config;
mod db;
mod file_proc;
mod model;
mod utils;
use dotenv::dotenv;
use tracing::{debug, error, info, warn};
use tracing_appender::rolling;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    fmt::{self, format::Format},
    layer::SubscriberExt,
    EnvFilter, Registry,
};

fn init_logger() -> impl Drop {
    let trace_log_level = env::var("TRACE_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let file_log_level = env::var("FILE_LOG_LEVEL").unwrap_or_else(|_| "error".to_string());
    let log_file_path = env::var("LOG_FILE_PATH").unwrap_or_else(|_| "app.log".to_string());

    // Setup for file writer
    let file_writer = rolling::never(".", log_file_path);
    let (non_blocking_writer, guard) = tracing_appender::non_blocking(file_writer);

    // Formatter for stdout, allowing ANSI codes
    let stdout_layer = fmt::layer().with_writer(std::io::stdout);

    // Custom formatter for file, omitting ANSI codes
    let file_layer = fmt::layer()
        .with_writer(non_blocking_writer)
        .fmt_fields(fmt::format::DefaultFields::new()) // Use default field formatter
        .event_format(Format::default().with_ansi(false)); // Disable ANSI codes for the file logger

    // Filters
    let filter_layer = EnvFilter::try_new(trace_log_level)
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap()
        .add_directive(
            file_log_level
                .parse()
                .expect("Failed to parse FILE_LOG_LEVEL"),
        );

    let subscriber = Registry::default()
        .with(filter_layer)
        .with(stdout_layer)
        .with(file_layer);

    subscriber.init();

    guard // Return the guard to keep it alive
}

fn main() {
    dotenv().ok();

    let _guard = init_logger();

    utils::hide_cursor();
    panic::set_hook(Box::new(|panic_info| {
        utils::show_cursor();
        eprintln!("Panic occurred: {:?}", panic_info);
    }));

    if let Err(err) = run_app() {
        eprintln!("Error: {}", err);
    }

    utils::show_cursor();
}

fn run_app() -> Result<(), String> {
    let config = app_config::load_configuration()
        .map_err(|err| format!("Error loading configuration: {}", err))?;
    debug!("config.root_paths: {:?}", config.root_paths);
    let non_overlapping = utils::non_overlapping_directories(config.root_paths);
    info!("Processing directories: {:?}", non_overlapping);
    file_proc::process(&non_overlapping)
        .map_err(|err| format!("Error processing files: {}", err))?;

    // file_proc::process_dir(&non_overlapping)
    //     .map_err(|err| format!("Error processing files: {}", err))?;

    Ok(())
}
