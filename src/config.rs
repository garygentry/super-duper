use config::{ Config, ConfigError, File as ConfigFile };
use serde::Deserialize;
use crate::utils;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub root_paths: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub spinner_key: Option<String>,
}

impl AppConfig {
    pub fn load() -> Result<AppConfig, ConfigError> {
        // Start by creating a ConfigBuilder
        let builder = Config::builder()
            // Add configuration values from a file named 'Config.toml', if present
            .add_source(ConfigFile::with_name("Config").required(false))
            // (Optional) You can add more configuration sources here, like environment variables
            // .add_source(Environment::default().separator("__"))
            // Build the configuration
            .build()?;

        // Try to deserialize the configuration into our AppConfig struct
        let config = builder.try_deserialize::<AppConfig>()?;

        let config = Self::sanitize_config(config);

        Ok(config)
    }

    fn sanitize_config(mut config: AppConfig) -> AppConfig {
        config.root_paths = utils::to_non_overlapping_directories(
            &config.root_paths
        );
        config
    }
}
