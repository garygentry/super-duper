use config::{Config, ConfigError, File as ConfigFile};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub root_paths: Vec<String>,
    pub ignore_patterns: Vec<String>,
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
        builder.try_deserialize::<AppConfig>()
    }
}
