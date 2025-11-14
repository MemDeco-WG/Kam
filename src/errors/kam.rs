use reqwest;
use thiserror::Error;
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

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Walkdir error: {0}")]
    Walkdir(#[from] walkdir::Error),

    #[error("Strip prefix error: {0}")]
    StripPrefix(#[from] std::path::StripPrefixError),

    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

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

    #[error("Invalid config: {0}")]
    InvalidConfig(String),

    #[error("Unsupported archive format: {0}")]
    UnsupportedArchive(String),

    #[error("Template not found: {0}")]
    TemplateNotFound(String),

    #[error("Library not found: {0}")]
    LibraryNotFound(String),

    #[error("Extract failed: {0}")]
    ExtractFailed(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("JSON error: {0}")]
    JsonError(String),

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

    #[error("Invalid module structure: {0}")]
    InvalidModuleStructure(String),

    #[error("Template render error: {0}")]
    TemplateRenderError(String),
}
