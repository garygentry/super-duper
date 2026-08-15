use std::io;

fn main() {
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
