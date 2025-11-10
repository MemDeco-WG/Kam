use thiserror::Error;

#[derive(Debug, Error)]
pub enum KamTomlError {
	#[error("TOML syntax error: {0}")]
	TomlSyntax(#[from] toml_edit::TomlError),
	#[error("TOML schema error: {0}")]
	TomlSchema(#[from] toml_edit::de::Error),
	#[error("TOML deserialization error: {0}")]
	TomlDe(#[from] toml::de::Error),
	#[error("TOML serialization error: {0}")]
	TomlSer(#[from] toml::ser::Error),
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
	#[error("kam.toml not found")]
	NotFound,
	#[error("kam.toml is empty")]
	EmptyFile,
	#[error("Missing required field: id")]
	MissingId,
	#[error("Missing required field: name")]
	MissingName,
	#[error("Missing required field: version")]
	MissingVersion,
	#[error("Missing required field: author")]
	MissingAuthor,
	#[error("Missing required field: description")]
	MissingDescription,
	#[error("Missing required section: [mmrl]")]
	MissingMmrl,
	#[error("Missing required field: zip_url")]
	MissingZipUrl,
	#[error("Missing required field: changelog")]
	MissingChangelog,
	#[error("Invalid id: {0}")]
	InvalidId(String),
	#[error("Version must be in format x.y.z")]
	InvalidVersionFormat,
	#[error("License file not found: {0}")]
	LicenseNotFound(String),
	#[error("License file is empty: {0}")]
	LicenseEmpty(String),
	#[error("Readme file not found: {0}")]
	ReadmeNotFound(String),
	#[error("Readme file is empty: {0}")]
	ReadmeEmpty(String),
	#[error("Unsupported architecture: {0}, supported: {1:?}")]
	UnsupportedArch(String, Vec<String>),
	#[error("Template module missing [kam.tmpl] section")]
	TemplateMissingTmpl,
	#[error("Library module missing [kam.lib] section")]
	LibraryMissingLib,
	#[error("Duplicate key in unique map: {0}")]
	DuplicateKey(String),
}

#[derive(Debug, PartialEq)]
pub enum ValidationResult {
	Valid,
	Invalid(String),
}
