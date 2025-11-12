// Central error aggregation module. This file defines the global `KamError`
// and re-exports commonly used error types under `crate::errors::*`.
pub mod cache;
pub mod kam_toml;

pub use cache::CacheError;
pub use kam_toml::KamTomlError;
pub use kam_toml::ValidationResult;

use thiserror::Error;
use reqwest;
use toml;

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

    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("KamToml error: {0}")]
    KamToml(#[from] crate::errors::KamTomlError),

    #[error("Cache error: {0}")]
    Cache(#[from] crate::errors::CacheError),

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Invalid directory: {0}")]
    InvalidDirectory(String),

    #[error("Package not found: {0}")]
    PackageNotFound(String),

    #[error("Invalid filename: {0}")]
    InvalidFilename(String),

    #[error("Upload failed: {0}")]
    UploadFailed(String),

    #[error("Fetch failed: {0}")]
    FetchFailed(String),

    #[error("Virtual environment already exists: {0}")]
    VenvExists(String),

    #[error("Virtual environment not found: {0}")]
    VenvNotFound(String),

    #[error("Required template variable not provided: {0}")]
    TemplateVarRequired(String),

    #[error("Unsupported archive format: {0}")]
    UnsupportedArchive(String),

    #[error("Template not found: {0}")]
    TemplateNotFound(String),

    #[error("Implementation requires template variables: {0}")]
    ImplRequiresVars(String),

    #[error("Invalid template variable format: {0}")]
    InvalidVarFormat(String),

    #[error("Repo template not found: {0}")]
    RepoTemplateNotFound(String),

    #[error("Unknown template type: {0}")]
    UnknownTemplateType(String),

    #[error("Failed to create table: {0}")]
    TableCreationFailed(String),

    #[error("TOML parse error: {0}")]
    TomlParseError(String),

    #[error("TOML serialize error: {0}")]
    TomlSerializeError(String),

    #[error("Invalid module type: {0}")]
    InvalidModuleType(String),

    #[error("Strip prefix failed: {0}")]
    StripPrefixFailed(String),

    #[error("Parse source spec failed: {0}")]
    ParseSourceFailed(String),

    #[error("Venv create failed: {0}")]
    VenvCreateFailed(String),

    #[error("Dependency resolution failed: {0}")]
    DependencyResolutionFailed(String),
}





pub type Result<T> = std::result::Result<T, KamError>;
