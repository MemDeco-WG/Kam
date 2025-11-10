// Central error aggregation module. This file defines the global `KamError`
// and re-exports commonly used error types under `crate::errors::*`.
pub mod cache;
pub mod kam_toml;

pub use cache::CacheError;
pub use kam_toml::KamTomlError;
pub use kam_toml::ValidationResult;

use thiserror::Error;

/// Kam-wide error type to avoid `Box<dyn Error>` in public APIs.
#[derive(Error, Debug)]
pub enum KamError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("TOML edit error: {0}")]
    TomlEdit(#[from] toml_edit::TomlError),

    #[error("TOML schema error: {0}")]
    TomlSchema(#[from] toml_edit::de::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("KamToml error: {0}")]
    KamToml(#[from] crate::errors::KamTomlError),

    #[error("Cache error: {0}")]
    Cache(#[from] crate::errors::CacheError),

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<String> for KamError {
    fn from(s: String) -> Self { KamError::Other(s) }
}

impl From<&str> for KamError {
    fn from(s: &str) -> Self { KamError::Other(s.to_string()) }
}

pub type Result<T> = std::result::Result<T, KamError>;
