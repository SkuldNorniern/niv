use std::fmt;

/// Configuration-related errors
#[derive(Debug)]
pub enum ConfigError {
    /// File I/O errors
    Io(std::io::Error),
    /// TOML parsing errors
    Toml(String),
    /// Configuration validation errors
    Validation(String),
    /// Path resolution errors
    Path(String),
    /// Permission errors
    Permission(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "I/O error: {}", e),
            ConfigError::Toml(msg) => write!(f, "TOML parsing error: {}", msg),
            ConfigError::Validation(msg) => write!(f, "Configuration validation error: {}", msg),
            ConfigError::Path(msg) => write!(f, "Path error: {}", msg),
            ConfigError::Permission(msg) => write!(f, "Permission error: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(error: std::io::Error) -> Self {
        ConfigError::Io(error)
    }
}

pub type ConfigResult<T> = Result<T, ConfigError>;
