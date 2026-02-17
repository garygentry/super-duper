pub mod analysis;
pub mod config;
pub mod engine;
pub mod error;
pub mod hasher;
pub mod platform;
pub mod progress;
pub mod scanner;
pub mod storage;

pub use config::AppConfig;
pub use engine::{ScanEngine, ScanResult, ScanStats};
pub use error::Error;
pub use progress::{ProgressReporter, SilentReporter};
