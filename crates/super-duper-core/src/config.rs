use config::{Config, ConfigError, File as ConfigFile};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Default Jaccard similarity threshold for `analysis::dir_similarity` (see `AppConfig`).
pub fn default_directory_similarity_threshold() -> f64 {
    0.5
}

/// Default noise cutoff for `analysis::dir_similarity` (see `AppConfig`).
pub fn default_directory_similarity_noise_cutoff() -> u32 {
    50
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub root_paths: Vec<String>,
    pub ignore_patterns: Vec<String>,
    /// Minimum Jaccard similarity (0.0 to 1.0) for two directories to be reported as similar.
    #[serde(default = "default_directory_similarity_threshold")]
    pub directory_similarity_threshold: f64,
    /// A content hash appearing in more than this many directories is treated as noise (common
    /// files like `README` or `.gitkeep`) and skipped when building candidate directory pairs.
    #[serde(default = "default_directory_similarity_noise_cutoff")]
    pub directory_similarity_noise_cutoff: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            root_paths: Vec::new(),
            ignore_patterns: Vec::new(),
            directory_similarity_threshold: default_directory_similarity_threshold(),
            directory_similarity_noise_cutoff: default_directory_similarity_noise_cutoff(),
        }
    }
}

impl AppConfig {
    pub fn to_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Checks the ranges of the fields not already enforced by their type.
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.directory_similarity_threshold) {
            return Err(format!(
                "directory_similarity_threshold must be between 0.0 and 1.0, got {}",
                self.directory_similarity_threshold
            ));
        }
        if self.directory_similarity_noise_cutoff == 0 {
            return Err("directory_similarity_noise_cutoff must be at least 1, got 0".to_string());
        }
        Ok(())
    }
}

pub fn load_configuration() -> Result<AppConfig, ConfigError> {
    let builder = Config::builder()
        .add_source(ConfigFile::with_name("Config").required(false))
        .build()?;
    let config: AppConfig = builder.try_deserialize()?;
    config.validate().map_err(ConfigError::Message)?;
    Ok(config)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_overlapping_no_overlap() {
        let dirs = vec![
            "/home/user/photos".to_string(),
            "/home/user/docs".to_string(),
            "/var/data".to_string(),
        ];
        let result = non_overlapping_directories(dirs);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"/home/user/photos".to_string()));
        assert!(result.contains(&"/home/user/docs".to_string()));
        assert!(result.contains(&"/var/data".to_string()));
    }

    #[test]
    fn test_non_overlapping_with_subdirectory() {
        let dirs = vec![
            "/home/user".to_string(),
            "/home/user/docs".to_string(),
            "/var/data".to_string(),
        ];
        let result = non_overlapping_directories(dirs);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"/home/user".to_string()));
        assert!(result.contains(&"/var/data".to_string()));
        // /home/user/docs should be removed as it's under /home/user
        assert!(!result.contains(&"/home/user/docs".to_string()));
    }

    #[test]
    fn default_similarity_settings_match_previous_hardcoded_values() {
        let config = AppConfig::default();
        assert_eq!(config.directory_similarity_threshold, 0.5);
        assert_eq!(config.directory_similarity_noise_cutoff, 50);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_rejects_out_of_range_threshold() {
        let mut config = AppConfig {
            directory_similarity_threshold: 1.5,
            ..Default::default()
        };
        assert!(config.validate().is_err());
        config.directory_similarity_threshold = -0.1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_noise_cutoff() {
        let config = AppConfig {
            directory_similarity_noise_cutoff: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn toml_without_new_fields_still_deserializes_with_defaults() {
        let toml = r#"
            root_paths = ["C:/data"]
            ignore_patterns = []
        "#;
        let config: AppConfig = Config::builder()
            .add_source(ConfigFile::from_str(toml, config::FileFormat::Toml))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();
        assert_eq!(config.directory_similarity_threshold, 0.5);
        assert_eq!(config.directory_similarity_noise_cutoff, 50);
    }

    #[test]
    fn toml_can_override_similarity_settings() {
        let toml = r#"
            root_paths = []
            ignore_patterns = []
            directory_similarity_threshold = 0.75
            directory_similarity_noise_cutoff = 10
        "#;
        let config: AppConfig = Config::builder()
            .add_source(ConfigFile::from_str(toml, config::FileFormat::Toml))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();
        assert_eq!(config.directory_similarity_threshold, 0.75);
        assert_eq!(config.directory_similarity_noise_cutoff, 10);
    }
}
