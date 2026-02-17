use config::{Config, ConfigError, File as ConfigFile};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub root_paths: Vec<String>,
    pub ignore_patterns: Vec<String>,
}

pub fn load_configuration() -> Result<AppConfig, ConfigError> {
    let builder = Config::builder()
        .add_source(ConfigFile::with_name("Config").required(false))
        .build()?;
    builder.try_deserialize::<AppConfig>()
}

/// Remove directories that are subdirectories of other directories in the list.
pub fn non_overlapping_directories(dirs: Vec<String>) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();

    for dir in dirs {
        let dir_path = Path::new(&dir);
        let mut should_add = true;
        let result_clone = result.clone();

        for res_dir in &result_clone {
            let res_dir_path = Path::new(res_dir);

            if dir_path.starts_with(res_dir_path) {
                should_add = false;
                break;
            }

            if res_dir_path.starts_with(dir_path) {
                result.retain(|x| x != res_dir);
                break;
            }
        }

        if should_add {
            result.push(dir);
        }
    }

    result
}
