mod app_config;
mod db;
mod file_proc;
mod model;

fn main() {
    match app_config::load_configuration() {
        Ok(config) => {
            println!("root_files: {:?}", config.root_paths);
            if let Err(err) = file_proc::process(&config.root_paths) {
                eprintln!("Error processing files: {}", err);
            }
        }
        Err(err) => {
            eprintln!("Error loading configuration: {}", err);
        }
    }
}
