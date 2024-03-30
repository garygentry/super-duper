use std::env;
use tracing_appender::rolling;
use tracing_subscriber::{
    fmt::{self},
    layer::SubscriberExt,
    EnvFilter, Registry,
};

pub fn init_logger() -> impl Drop {
    let stdout_log_level = env::var("STDOUT_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let file_log_level = env::var("FILE_LOG_LEVEL").unwrap_or_else(|_| "error".to_string());
    let log_file_path = env::var("LOG_FILE_PATH").unwrap_or_else(|_| "app.log".to_string());

    // Setup for file writer
    let file_writer = rolling::never(".", log_file_path);
    let (non_blocking_writer, guard) = tracing_appender::non_blocking(file_writer);

    // Combined filter layer for both stdout and file, utilizing directive prefixes
    let filter_layer = EnvFilter::try_new(format!("{},{}", stdout_log_level, file_log_level))
        .unwrap_or_else(|_| EnvFilter::new("info,error"));

    // Formatter for stdout, allowing ANSI codes
    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .pretty()
        .with_file(false)
        .without_time(); // Use pretty printing for stdout

    // Custom formatter for file, omitting ANSI codes
    let file_layer = fmt::layer()
        .with_writer(non_blocking_writer)
        .with_ansi(false); // Disable ANSI codes for the file logger

    // Combine filter with both output layers into a single subscriber
    let subscriber = Registry::default()
        .with(filter_layer)
        .with(stdout_layer)
        .with(file_layer);

    // Set the combined subscriber as the global default
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default subscriber");

    guard // Return the guard to keep it alive
}

// fn init_logger() -> impl Drop {
//     let trace_log_level = env::var("TRACE_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
//     let file_log_level = env::var("FILE_LOG_LEVEL").unwrap_or_else(|_| "error".to_string());
//     let log_file_path = env::var("LOG_FILE_PATH").unwrap_or_else(|_| "app.log".to_string());
//     println!("trace_log_level {:?}", trace_log_level); // = debug

//     // Setup for file writer
//     let file_writer = rolling::never(".", log_file_path);
//     let (non_blocking_writer, guard) = tracing_appender::non_blocking(file_writer);

//     // Formatter for stdout, allowing ANSI codes
//     let stdout_layer = fmt::layer().with_writer(std::io::stdout);

//     // Custom formatter for file, omitting ANSI codes
//     let file_layer = fmt::layer()
//         .with_writer(non_blocking_writer)
//         .fmt_fields(fmt::format::DefaultFields::new()) // Use default field formatter
//         .event_format(Format::default().with_ansi(false)); // Disable ANSI codes for the file logger

//     // Filters
//     let filter_layer = EnvFilter::try_new(trace_log_level)
//         .or_else(|_| EnvFilter::try_new("info"))
//         .unwrap()
//         .add_directive(
//             file_log_level
//                 .parse()
//                 .expect("Failed to parse FILE_LOG_LEVEL"),
//         );

//     let subscriber = Registry::default()
//         .with(filter_layer)
//         .with(stdout_layer)
//         .with(file_layer);

//     subscriber.init();

//     guard // Return the guard to keep it alive
// }
