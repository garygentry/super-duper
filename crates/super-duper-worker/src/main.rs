use std::io;
use tracing_subscriber::EnvFilter;

fn main() {
    let filter = EnvFilter::try_from_env("SUPER_DUPER_LOG")
        .unwrap_or_else(|_| EnvFilter::new("super_duper_core=info,super_duper_worker=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(io::stderr)
        .init();

    let stdin = io::stdin();
    let stdout = io::stdout();

    match super_duper_worker::run(stdin.lock(), stdout) {
        Ok(()) => eprintln!("worker input ended; shutting down"),
        Err(error) => {
            eprintln!("worker fatal error: {error}");
            std::process::exit(1);
        }
    }
}
