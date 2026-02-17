use std::env;
use tracing::info;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

pub fn init_logger() -> impl Drop {
    let filter = env::var("TRACING_LEVEL").unwrap_or_else(|_| "info".to_string());
    let filter_layer = EnvFilter::new(filter);

    let log_file_path =
        env::var("LOG_FILE_PATH").unwrap_or_else(|_| "./logs/sd.log".to_string());

    let file_appender = tracing_appender::rolling::never("./", log_file_path);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(std::io::stdout)
                .pretty()
                .with_file(false)
                .without_time()
                .with_ansi(true),
        )
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
        .with(filter_layer)
        .init();

    info!("Tracing is configured for stdout and file logging.");

    guard
}
