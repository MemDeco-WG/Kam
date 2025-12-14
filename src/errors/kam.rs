use thiserror::Error;
use toml;
// Kam的错误类型，避免在公共API里用Box<dyn Error>
// 用thiserror自动生成Error trait实现
#[derive(Error, Debug)]
pub enum KamError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("TOML error: {0}")]
    Toml(String),

    #[error("JSON error: {0}")]
    Json(String),

    #[error("Walkdir error: {0}")]
    Walkdir(#[from] walkdir::Error),

    #[error("KamToml error: {0}")]
    KamToml(#[from] crate::errors::KamTomlError),

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

    #[error("Invalid module type: {0}")]
    InvalidModuleType(String),

    #[error("Parse source spec failed: {0}")]
    ParseSourceFailed(String),

    #[error("Venv create failed: {0}")]
    VenvCreateFailed(String),

    #[error("Invalid module structure: {0}")]
    InvalidModuleStructure(String),

    #[error("Template render error: {0}")]
    TemplateRenderError(String),
}

impl From<toml_edit::TomlError> for KamError {
    fn from(e: toml_edit::TomlError) -> Self {
        KamError::Toml(format!("TOML edit error: {}", e))
    }
}

impl From<toml_edit::de::Error> for KamError {
    fn from(e: toml_edit::de::Error) -> Self {
        KamError::Toml(format!("TOML schema error: {}", e))
    }
}

impl From<toml::de::Error> for KamError {
    fn from(e: toml::de::Error) -> Self {
        KamError::Toml(format!("TOML deserialization error: {}", e))
    }
}

impl From<toml::ser::Error> for KamError {
    fn from(e: toml::ser::Error) -> Self {
        KamError::Toml(format!("TOML serialization error: {}", e))
    }
}

impl From<serde_json::Error> for KamError {
    fn from(e: serde_json::Error) -> Self {
        KamError::Json(format!("JSON error: {}", e))
    }
}

impl From<std::path::StripPrefixError> for KamError {
    fn from(e: std::path::StripPrefixError) -> Self {
        KamError::InvalidDirectory(format!("strip_prefix failed: {}", e))
    }
}
