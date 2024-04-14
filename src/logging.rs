use std::env;
use tracing::info;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

const DEFAULT_LOG_PATH: &str = "sd.log";

pub fn init_logger() -> impl Drop {
    // Attempt to read the tracing level from the `TRACING_LEVEL` environment variable.
    // Default to `info` if not specified.
    let default_filter = "info";
    let filter = env::var("TRACING_LEVEL").unwrap_or_else(|_| default_filter.to_string());
    let filter_layer = EnvFilter::new(filter);

    // Attempt to read the log file path from the `LOG_FILE_PATH` environment variable.
    // Default to `./logs/my_app.log` if not specified.
    let default_log_path = DEFAULT_LOG_PATH;
    let log_file_path = env::var("LOG_FILE_PATH").unwrap_or_else(|_| default_log_path.to_string());

    // Set up file logging
    let file_appender = tracing_appender::rolling::never("./", log_file_path);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Combine everything together
    tracing_subscriber::registry()
        // .with(
        //     fmt::layer()
        //         .with_writer(std::io::stdout) // Log to stdout
        //         // .pretty()
        //         .with_file(false)
        //         .without_time()
        //         .with_ansi(true),
        // ) // Enable ANSI escape codes for colors in the terminal
        .with(
            fmt::layer()
                .with_writer(non_blocking) // Log to file
                .with_ansi(false),
        ) // Disable ANSI escape codes for the file logger
        .with(filter_layer)
        .init();

    // Your application logic here
    info!("Tracing is configured for stdout and file logging.");

    guard // Return the guard to keep it alive
}
