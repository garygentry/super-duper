use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("{0}")]
    Other(String),
}
