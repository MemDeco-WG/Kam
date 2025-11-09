use serde::{Deserialize, Deserializer, Serialize};
use serde::de::IntoDeserializer;
use regex::Regex;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

/// Default source for dependencies if not specified
const DEFAULT_DEPENDENCY_SOURCE: &str = "https://github.com/MemDeco-WG/Kam-Index/";

/// Errors that can occur when parsing or validating kam.toml
#[derive(Error, Debug)]
pub enum KamTomlError {
    #[error(transparent)]
    TomlSyntax(#[from] toml_edit::TomlError),
    #[error(transparent)]
    TomlSchema(#[from] toml_edit::de::Error),
    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),
    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("kam.toml not found")]
    NotFound,
    #[error("kam.toml is empty")]
    EmptyFile,
    #[error("prop.id is required and cannot be empty")]
    MissingId,
    #[error("prop.name is required and cannot be empty")]
    MissingName,
    #[error("prop.version is required and cannot be empty")]
    MissingVersion,
    #[error("prop.author is required and cannot be empty")]
    MissingAuthor,
    #[error("prop.description is required and cannot be empty")]
    MissingDescription,
    #[error("mmrl section is required")]
    MissingMmrl,
    #[error("mmrl.zip_url is required for MMRL and cannot be empty")]
    MissingZipUrl,
    #[error("mmrl.changelog is required for MMRL and cannot be empty")]
    MissingChangelog,
    #[error("ID '{0}' must start with a letter and contain only letters, digits, '.', '_', or '-'")]
    InvalidId(String),
    #[error("prop.version must be in format x.y.z (e.g., 1.0.0)")]
    InvalidVersionFormat,
    #[error("license file '{0}' not found")]
    LicenseNotFound(String),
    #[error("license file '{0}' is empty")]
    LicenseEmpty(String),
    #[error("readme file '{0}' not found")]
    ReadmeNotFound(String),
    #[error("readme file '{0}' is empty")]
    ReadmeEmpty(String),
    #[error("unsupported arch '{0}', valid: {1:?}")]
    UnsupportedArch(String, Vec<&'static str>),
    #[error("Template module must have tmpl section")]
    TemplateMissingTmpl,
    #[error("Library module must have lib section")]
    LibraryMissingLib,
    #[error("duplicate key '{0}' found")]
    DuplicateKey(String),
}

/// Validation result for metadata checks
#[derive(Debug, Clone)]
pub enum ValidationResult {
    Valid,
    Invalid(String),  // Contains error message
}

/// Helper function to deserialize a map while ensuring all keys are unique.
fn deserialize_unique_map<'de, D, K, V, F>(
    deserializer: D,
    error_msg: F,
) -> Result<BTreeMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord + std::fmt::Display,
    V: Deserialize<'de>,
    F: FnOnce(&K) -> String,
{
    struct Visitor<K, V, F>(F, std::marker::PhantomData<(K, V)>);

    impl<'de, K, V, F> serde::de::Visitor<'de> for Visitor<K, V, F>
    where
        K: Deserialize<'de> + Ord + std::fmt::Display,
        V: Deserialize<'de>,
        F: FnOnce(&K) -> String,
    {
        type Value = BTreeMap<K, V>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a map with unique keys")
        }

        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            use std::collections::btree_map::Entry;

            let mut map = BTreeMap::new();
            while let Some((key, value)) = access.next_entry::<K, V>()? {
                match map.entry(key) {
                    Entry::Occupied(entry) => {
                        return Err(serde::de::Error::custom((self.0)(entry.key())));
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(value);
                    }
                }
            }
            Ok(map)
        }
    }

    deserializer.deserialize_map(Visitor(error_msg, std::marker::PhantomData))
}


/// Core module.prop fields
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[allow(non_snake_case)]
pub struct PropSection {
    pub id: String,
    pub name: std::collections::HashMap<String, String>,  // Multi-language support
    pub version: String,
    pub versionCode: u64,
    pub author: String,
    pub description: std::collections::HashMap<String, String>,  // Multi-language support
    pub updateJson: Option<String>,
}

impl PropSection {
    /// Get the name in the default language (English)
    pub fn get_name(&self) -> &str {
        self.name.get("en").map(|s| s.as_str()).unwrap_or("Unknown Module")
    }

    /// Get the description in the default language (English)
    pub fn get_description(&self) -> &str {
        self.description.get("en").map(|s| s.as_str()).unwrap_or("No description available")
    }
}

/// MMRL-related metadata
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MmrlSection {
    pub zip_url: Option<String>,  // Required for MMRL
    pub changelog: Option<String>,  // Required for MMRL
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub readme: Option<String>,
    pub screenshots: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub keywords: Option<Vec<String>>,
    pub maintainers: Option<Vec<String>>,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub issues: Option<String>,
    pub funding: Option<String>,
    pub support: Option<String>,
    pub donate: Option<String>,
    pub cover: Option<String>,
    pub icon: Option<String>,
    pub devices: Option<Vec<String>>,
    pub arch: Option<Vec<String>>,
    pub require: Option<Vec<String>>,
    pub note: Option<NoteSection>,
    pub manager: Option<ManagerSection>,
    pub antifeatures: Option<Vec<String>>,
    pub options: Option<OptionsSection>,
    pub max_num: Option<u64>,
    pub min_api: Option<u32>,
    pub max_api: Option<u32>,
    pub verified: Option<bool>,
    pub features: Option<Vec<String>>,
}

/// Note section for MMRL
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NoteSection {
    pub title: Option<String>,
    pub message: String,
    pub color: Option<String>,
}

/// Manager-specific configurations
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ManagerConfig {
    pub min: Option<u64>,
    pub devices: Option<Vec<String>>,
    pub arch: Option<Vec<String>>,
    pub require: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ManagerSection {
    pub magisk: Option<ManagerConfig>,
    pub kernelsu: Option<ManagerConfig>,
    pub apatch: Option<ManagerConfig>,
}

/// Options section
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct OptionsSection {
    pub archive: Option<ArchiveOptions>,
    pub disable_remote_metadata: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ArchiveOptions {
    pub compression: Option<String>,
}

/// Individual dependency with version spec (supports ranges like ">=1.0.0, <2.0.0")
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub id: String,
    pub version: Option<String>,
    pub source: Option<String>,  // Custom source URL or repository
}

/// Dependency section
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DependencySection {
    pub normal: Option<Vec<Dependency>>,
    pub dev: Option<Vec<Dependency>>,
}

/// Kam custom fields
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ModuleType {
    Normal,
    Template,
    Library,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct VariableDefinition {
    pub var_type: String,  // e.g., "string", "int", "bool"
    pub required: bool,
    pub default: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TmplSection {
    pub used_template: Option<String>,
    pub variables: std::collections::HashMap<String, VariableDefinition>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LibSection {
    pub dependencies: std::collections::HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct KamSection {
    pub min_api: Option<u32>,
    pub max_api: Option<u32>,
    pub supported_arch: Option<Vec<String>>,
    pub conflicts: Option<Vec<String>>,
    pub dependency: Option<DependencySection>,
    pub build: Option<BuildSection>,
    pub proxy: Option<Vec<String>>,
    pub module_type: ModuleType,
    pub tmpl: Option<TmplSection>,
    pub lib: Option<LibSection>,
}

impl Default for KamSection {
    fn default() -> Self {
        Self {
            min_api: None,
            max_api: None,
            supported_arch: None,
            conflicts: None,
            dependency: None,
            build: None,
            proxy: None,
            module_type: ModuleType::Normal,
            tmpl: None,
            lib: None,
        }
    }
}

/// Build-related fields
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct BuildSection {
    pub target_dir: Option<String>,
    pub output_file: Option<String>,
    pub pre_build: Option<String>,
    pub post_build: Option<String>,
}



/// KamToml: A superset of module.prop, update.json, and other metadata,
/// inspired by pyproject.toml format with hierarchical sections.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KamToml {
    pub prop: PropSection,
    pub mmrl: Option<MmrlSection>,
    pub kam: KamSection,
    pub tool: Option<serde_json::Value>,
    pub tmpl: Option<TmplSection>,
    pub lib: Option<LibSection>,
    /// The raw unserialized document.
    #[serde(skip)]
    pub raw: String,
}

impl Default for KamToml {
    fn default() -> Self {
        let mut name = std::collections::HashMap::new();
        name.insert("en".to_string(), "Default Module".to_string());
        let mut description = std::collections::HashMap::new();
        description.insert("en".to_string(), "A default module description".to_string());

        Self {
            prop: PropSection {
                id: String::new(),
                name,
                version: "0.1.0".to_string(),
                versionCode: 1,
                author: String::new(),
                description,
                updateJson: Some("https://example.com/update.json".to_string()),
            },
            mmrl: Some(MmrlSection {
                zip_url: Some("https://example.com/module.zip".to_string()),
                changelog: Some("https://example.com/changelog.md".to_string()),
                license: Some("MIT".to_string()),
                homepage: Some("https://example.com".to_string()),
                readme: Some("README.md".to_string()),
                screenshots: Some(vec![]),
                categories: Some(vec!["Kam".to_string()]),
                keywords: Some(vec!["android".to_string(), "root".to_string(), "module".to_string()]),
                maintainers: Some(vec![]),
                repository: Some("https://github.com/user/repo".to_string()),
                documentation: Some("https://example.com/docs".to_string()),
                issues: Some("https://github.com/user/repo/issues".to_string()),
                funding: Some("https://example.com/donate".to_string()),
                support: Some("https://example.com/support".to_string()),
                donate: Some("https://example.com/donate".to_string()),
                cover: Some("https://example.com/cover.png".to_string()),
                icon: Some("https://example.com/icon.png".to_string()),
                devices: Some(vec![]),
                arch: Some(vec!["arm64-v8a".to_string(), "armeabi-v7a".to_string()]),
                require: Some(vec![]),
                note: Some(NoteSection {
                    title: Some("Note".to_string()),
                    message: "Additional info".to_string(),
                    color: None,
                }),
                manager: Some(ManagerSection {
                    magisk: Some(ManagerConfig {
                        min: Some(1),
                        devices: None,
                        arch: None,
                        require: None,
                    }),
                    kernelsu: None,
                    apatch: None,
                }),
                antifeatures: Some(vec![]),
                options: Some(OptionsSection {
                    archive: Some(ArchiveOptions {
                        compression: Some("gzip".to_string()),
                    }),
                    disable_remote_metadata: Some(false),
                }),
                max_num: Some(1),
                min_api: Some(29),
                max_api: Some(35),
                verified: Some(false),
                features: Some(vec!["feature1".to_string()]),
            }),
            kam: KamSection::default(),
            tool: None,
            tmpl: None,
            lib: None,
            raw: String::new(),
        }
    }
}
#[allow(non_snake_case)]
impl KamToml {

    /// Create a new KamToml with required fields
    pub fn new(
        id: String,
        name: std::collections::HashMap<String, String>,
        version: String,
        versionCode: u64,
        author: String,
        description: std::collections::HashMap<String, String>,
        updateJson: Option<String>,
    ) -> Self {
        Self {
            prop: PropSection {
                id,
                name,
                version,
                versionCode,
                author,
                description,
                updateJson,
            },
            mmrl: Default::default(),
            kam: KamSection {
                min_api: None,
                max_api: None,
                supported_arch: None,
                conflicts: None,
                dependency: None,
                build: None,
                proxy: None,
                module_type: ModuleType::Normal,
                tmpl: None,
                lib: None,
            },
            tool: None,
            tmpl: None,
            lib: None,
            raw: String::new(),
        }
    }

    pub fn new_with_current_timestamp(id: String, name: std::collections::HashMap<String, String>, version: String, author: String, description: std::collections::HashMap<String, String>, updateJson: Option<String>) -> Self {
        use chrono::Utc;
        let versionCode = Utc::now().timestamp_millis() as u64;
        Self::new(id, name, version, versionCode, author, description, updateJson)
    }

    pub fn new_template(id: String, name: std::collections::HashMap<String, String>, version: String, versionCode: u64, author: String, description: std::collections::HashMap<String, String>, updateJson: Option<String>) -> Self {
        let mut kt = Self::new(id, name, version, versionCode, author, description, updateJson);
        kt.kam.module_type = ModuleType::Template;
        kt.kam.tmpl = Some(TmplSection { used_template: None, variables: std::collections::HashMap::new() });
        kt
    }

    /// Parse a `KamToml` from a raw TOML string.
    pub fn from_string(raw: String) -> Result<Self, KamTomlError> {
        let kamtoml = toml_edit::DocumentMut::from_str(&raw)
            .map_err(KamTomlError::TomlSyntax)?;
        let kamtoml = Self::deserialize(kamtoml.into_deserializer())
            .map_err(KamTomlError::TomlSchema)?;
        Ok(Self { raw, ..kamtoml })
    }

    /// Load from TOML string
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    /// Serialize to TOML string
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }

    /// Get the effective source for a dependency (uses fallback if None)
    pub fn get_effective_source(dep: &Dependency) -> &str {
        dep.source.as_deref().unwrap_or(DEFAULT_DEPENDENCY_SOURCE)
    }

    /// Validate ID against the regex: ^[a-zA-Z][a-zA-Z0-9._-]+$
    pub fn validate_id(id: &str) -> ValidationResult {
        let re = Regex::new(r"^[a-zA-Z][a-zA-Z0-9._-]+$").unwrap();
        if re.is_match(id) {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid(format!("ID '{}' must start with a letter and contain only letters, digits, '.', '_', or '-'.", id))
        }
    }

    /// Write the KamToml to a file named "kam.toml" in the given directory
    pub fn write_to_dir(&self, dir_path: &Path) -> Result<(), KamTomlError> {
        std::fs::create_dir_all(dir_path)?;
        let file_path = dir_path.join("kam.toml");
        let content = self.to_toml()?;
        std::fs::write(file_path, content)?;
        Ok(())
    }

    /// Load KamToml from "kam.toml" in the given directory, with validation
    pub fn load_from_dir(dir_path: &Path) -> Result<Self, KamTomlError> {
        let file_path = dir_path.join("kam.toml");
        if !file_path.exists() {
            return Err(KamTomlError::NotFound);
        }
        let content = std::fs::read_to_string(&file_path)?;
        if content.trim().is_empty() {
            return Err(KamTomlError::EmptyFile);
        }
        let kt: KamToml = KamToml::from_string(content)?;

        // Check required fields
        if kt.prop.id.is_empty() {
            return Err(KamTomlError::MissingId);
        }
        if kt.prop.name.is_empty() {
            return Err(KamTomlError::MissingName);
        }
        if kt.prop.version.is_empty() {
            return Err(KamTomlError::MissingVersion);
        }
        if kt.prop.author.is_empty() {
            return Err(KamTomlError::MissingAuthor);
        }
        if kt.prop.description.is_empty() {
            return Err(KamTomlError::MissingDescription);
        }
        if let Some(mmrl) = &kt.mmrl {
            if mmrl.zip_url.as_ref().map_or(true, |s| s.is_empty()) {
                return Err(KamTomlError::MissingZipUrl);
            }
            if mmrl.changelog.as_ref().map_or(true, |s| s.is_empty()) {
                return Err(KamTomlError::MissingChangelog);
            }
        } else {
            return Err(KamTomlError::MissingMmrl);
        }

        // Validate id
        if let ValidationResult::Invalid(_) = KamToml::validate_id(&kt.prop.id) {
            return Err(KamTomlError::InvalidId(kt.prop.id.clone()));
        }

        // Validate version format (semver-like: x.y.z)
        let version_re = Regex::new(r"^\d+\.\d+\.\d+$").unwrap();
        if !version_re.is_match(&kt.prop.version) {
            return Err(KamTomlError::InvalidVersionFormat);
        }

        // Check license file if specified
        if let Some(mmrl) = &kt.mmrl {
            if let Some(license) = &mmrl.license {
                let license_path = dir_path.join(license);
                if !license_path.exists() {
                    return Err(KamTomlError::LicenseNotFound(license.clone()));
                }
                if license_path.metadata()?.len() == 0 {
                    return Err(KamTomlError::LicenseEmpty(license.clone()));
                }
            }

            // Check readme file if specified
            if let Some(readme) = &mmrl.readme {
                let readme_path = dir_path.join(readme);
                if !readme_path.exists() {
                    return Err(KamTomlError::ReadmeNotFound(readme.clone()));
                }
                if readme_path.metadata()?.len() == 0 {
                    return Err(KamTomlError::ReadmeEmpty(readme.clone()));
                }
            }
        }

        // Validate supported_arch
        if let Some(archs) = &kt.kam.supported_arch {
            let valid_archs = vec!["arm64-v8a", "armeabi-v7a", "x86", "x86_64"];
            for arch in archs {
                if !valid_archs.contains(&arch.as_str()) {
                    return Err(KamTomlError::UnsupportedArch(arch.clone(), valid_archs));
                }
            }
        }

        // Validate module_type
        match kt.kam.module_type {
            ModuleType::Template => {
                if kt.kam.tmpl.is_none() {
                    return Err(KamTomlError::TemplateMissingTmpl);
                }
            }
            ModuleType::Library => {
                if kt.kam.lib.is_none() {
                    return Err(KamTomlError::LibraryMissingLib);
                }
            }
            ModuleType::Normal => {}
        }

        Ok(kt)
    }

    /// Write the KamToml to a specific file path
    pub fn write_to_file(&self, path: &Path) -> Result<(), KamTomlError> {
        let content = self.to_toml()?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Load KamToml from a specific file path
    pub fn load_from_file(path: &Path) -> Result<Self, KamTomlError> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_string(content)?)
    }
}

// Ignore raw document in comparison.
impl PartialEq for KamToml {
    fn eq(&self, other: &Self) -> bool {
        self.prop == other.prop
            && self.mmrl == other.mmrl
            && self.kam == other.kam
            && self.tool == other.tool
            && self.tmpl == other.tmpl
            && self.lib == other.lib
    }
}

impl Eq for KamToml {}

impl AsRef<[u8]> for KamToml {
    fn as_ref(&self) -> &[u8] {
        self.raw.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_id_valid() {
        assert!(matches!(KamToml::validate_id("a_module"), ValidationResult::Valid));
        assert!(matches!(KamToml::validate_id("a.module"), ValidationResult::Valid));
        assert!(matches!(KamToml::validate_id("module-101"), ValidationResult::Valid));
    }

    #[test]
    fn test_validate_id_invalid() {
        assert!(matches!(KamToml::validate_id("a module"), ValidationResult::Invalid(_)));
        assert!(matches!(KamToml::validate_id("1_module"), ValidationResult::Invalid(_)));
        assert!(matches!(KamToml::validate_id("-a-module"), ValidationResult::Invalid(_)));
    }

    #[test]
    fn test_default() {
        let kt = KamToml::default();
        assert_eq!(kt.prop.version, "0.1.0");
        assert_eq!(kt.prop.versionCode, 1);
        if let Some(mmrl) = &kt.mmrl {
            assert_eq!(mmrl.categories, Some(vec!["Kam".to_string()]));
            assert_eq!(mmrl.keywords, Some(vec!["android".to_string(), "root".to_string(), "module".to_string()]));
        }
    }

    #[test]
    fn test_new() {
        let mut name = std::collections::HashMap::new();
        name.insert("en".to_string(), "Test Module".to_string());
        let mut description = std::collections::HashMap::new();
        description.insert("en".to_string(), "Test Description".to_string());
        let kt = KamToml::new(
            "test_id".to_string(),
            name.clone(),
            "1.0.0".to_string(),
            100,
            "Test Author".to_string(),
            description.clone(),
            Some("https://example.com/update.json".to_string()),
        );
        assert_eq!(kt.prop.id, "test_id");
        assert_eq!(kt.prop.name, name);
        assert_eq!(kt.prop.updateJson, Some("https://example.com/update.json".to_string()));
    }

    #[test]
    fn test_serialization() {
        let mut name = std::collections::HashMap::new();
        name.insert("en".to_string(), "Example Module".to_string());
        let mut description = std::collections::HashMap::new();
        description.insert("en".to_string(), "Description".to_string());
        let kt = KamToml::new(
            "example".to_string(),
            name,
            "1.0.0".to_string(),
            123,
            "Author".to_string(),
            description,
            None,
        );
        let toml_str = kt.to_toml().unwrap();
        let deserialized: KamToml = KamToml::from_toml(&toml_str).unwrap();
        assert_eq!(kt.prop.id, deserialized.prop.id);
    }

    #[test]
    fn test_write_to_dir() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join("kam_test_write");
        fs::create_dir_all(&temp_dir).unwrap();

        let kt = KamToml::default();
        kt.write_to_dir(&temp_dir).unwrap();

        let file_path = temp_dir.join("kam.toml");
        assert!(file_path.exists());
        assert!(fs::read_to_string(file_path).unwrap().contains("[prop]"));

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_load_from_dir() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join("kam_test_load");
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a sample kam.toml
        let sample_toml = r#"
[prop]
id = "test_module"
name = { en = "Test Module" }
version = "1.0.0"
versionCode = 100
author = "Test Author"
description = { en = "Test Description" }

[mmrl]
zip_url = "https://example.com/module.zip"
changelog = "https://example.com/changelog.md"
license = "LICENSE"
readme = "README.md"

[kam]
min_api = 29
supported_arch = ["arm64-v8a"]
module_type = "Normal"
"#;
        fs::write(temp_dir.join("kam.toml"), sample_toml).unwrap();

        // Create LICENSE and README.md files
        fs::write(temp_dir.join("LICENSE"), "MIT License").unwrap();
        fs::write(temp_dir.join("README.md"), "# README").unwrap();

        let loaded = KamToml::load_from_dir(&temp_dir).unwrap();
        assert_eq!(loaded.prop.id, "test_module");
        assert_eq!(loaded.prop.version, "1.0.0");
        assert_eq!(loaded.prop.get_name(), "Test Module");
        assert_eq!(loaded.prop.get_description(), "Test Description");

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_load_from_dir_missing_file() {
        let temp_dir = std::env::temp_dir().join("kam_test_missing");
        let result = KamToml::load_from_dir(&temp_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_load_from_dir_invalid_version() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join("kam_test_invalid");
        fs::create_dir_all(&temp_dir).unwrap();

        let invalid_toml = r#"
[prop]
id = "test"
name = { en = "Test" }
version = "1.0"
versionCode = 1
author = "Author"
description = { en = "Desc" }

[mmrl]
zip_url = "https://example.com/module.zip"
changelog = "https://example.com/changelog.md"

[kam]
min_api = 29
module_type = "Normal"
"#;
        fs::write(temp_dir.join("kam.toml"), invalid_toml).unwrap();

        let result = KamToml::load_from_dir(&temp_dir);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("format") || error.to_string().contains("x.y.z"));
        fs::remove_dir_all(temp_dir).unwrap();
    }
}
