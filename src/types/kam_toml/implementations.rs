
use crate::errors::KamTomlError;
use crate::errors::ValidationResult;
use super::*;
use chrono;
use regex::Regex;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;
use toml;
use toml_edit;
use super::sections::dependency::Dependency;
use super::sections::module::{ModuleType, TmplSection};

impl KamToml {
    pub fn new(
        id: String,
        name: BTreeMap<String, String>,
        version: String,
        version_code: u64,
        author: String,
        description: BTreeMap<String, String>,
        update_json: Option<String>,
    ) -> Self {
        let mut kt = Self::default();
        kt.prop.id = id;
        kt.prop.name = name;
        kt.prop.version = version;
        kt.prop.versionCode = version_code;
        kt.prop.author = author;
        kt.prop.description = description;
        kt.prop.updateJson = update_json;
        kt
    }

    pub fn new_with_current_timestamp(
        id: String,
        name: BTreeMap<String, String>,
        version: String,
        author: String,
        description: BTreeMap<String, String>,
        update_json: Option<String>,
    ) -> Self {
        let mut kt = Self::new(id, name, version, 1, author, description, update_json);
        kt.prop.versionCode = chrono::Utc::now().timestamp_millis() as u64;
        kt
    }

    pub fn new_template(
        id: String,
        name: BTreeMap<String, String>,
        version: String,
        author: String,
        description: BTreeMap<String, String>,
        update_json: Option<String>,
    ) -> Self {
        let mut kt = Self::new(id, name, version, 1, author, description, update_json);
        kt.kam.module_type = ModuleType::Template;
        kt.kam.tmpl = Some(TmplSection {
            used_template: None,
            variables: BTreeMap::new(),
        });
        kt
    }

    pub fn from_string(raw: String) -> Result<Self, KamTomlError> {
        let kamtoml = toml_edit::DocumentMut::from_str(&raw)
            .map_err(KamTomlError::TomlSyntax)?;
        let kamtoml = toml_edit::de::from_document(kamtoml)
            .map_err(KamTomlError::TomlSchema)?;
        Ok(Self { raw, ..kamtoml })
    }

    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }

    pub fn get_effective_source(dep: &Dependency) -> &str {
        dep.source.as_deref().unwrap_or(DEFAULT_DEPENDENCY_SOURCE)
    }

    pub fn resolve_dependencies(&self) -> Result<crate::dependency_resolver::FlatDependencyGroups, crate::dependency_resolver::DependencyResolutionError> {
        if let Some(dependency_section) = &self.kam.dependency {
            let resolver = crate::dependency_resolver::DependencyResolver::new(dependency_section);
            resolver.resolve()
        } else {
            Ok(crate::dependency_resolver::FlatDependencyGroups::new())
        }
    }

    pub fn validate_dependencies(&self) -> Result<(), crate::dependency_resolver::DependencyResolutionError> {
        if let Some(dependency_section) = &self.kam.dependency {
            let resolver = crate::dependency_resolver::DependencyResolver::new(dependency_section);
            resolver.validate()
        } else {
            Ok(())
        }
    }

    pub fn validate_id(id: &str) -> ValidationResult {
        let re = Regex::new(r"^[a-zA-Z][a-zA-Z0-9._-]+$",).unwrap();
        if re.is_match(id) {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid(format!("ID '{}' must start with a letter and contain only letters, digits, '.', '_', or '-'.", id))
        }
    }

    pub fn write_to_dir(&self, dir_path: &Path) -> Result<(), KamTomlError> {
        std::fs::create_dir_all(dir_path)?;
        let file_path = dir_path.join("kam.toml");
        let content = self.to_toml()?;
        std::fs::write(file_path, content)?;
        Ok(())
    }

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
            // Ensure repo section exists under [mmrl]
            if mmrl.repo.is_none() {
                return Err(KamTomlError::MissingMmrl);
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
            // Check license file if specified
            if let Some(license) = mmrl.repo.as_ref().and_then(|r| r.license.as_ref()) {
                let license_path = dir_path.join(license);
                if !license_path.exists() {
                    return Err(KamTomlError::LicenseNotFound(license.clone()));
                }
                if license_path.metadata()?.len() == 0 {
                    return Err(KamTomlError::LicenseEmpty(license.clone()));
                }
            }

            // Check readme file if specified
            if let Some(readme) = mmrl.repo.as_ref().and_then(|r| r.readme.as_ref()) {
                let readme_path = dir_path.join(readme);
                if !readme_path.exists() {
                    return Err(KamTomlError::ReadmeNotFound(readme.clone()));
                }
                if readme_path.metadata()?.len() == 0 {
                    return Err(KamTomlError::ReadmeEmpty(readme.clone()));
                }
            }
        }

        // Validate supported_arch (compare canonical string forms)
        if let Some(archs) = &kt.kam.supported_arch {
            let valid_archs = vec!["arm64-v8a".to_string(), "armeabi-v7a".to_string(), "x86".to_string(), "x86_64".to_string()];
            for arch in archs {
                let arch_s = arch.to_string();
                if !valid_archs.contains(&arch_s) {
                    return Err(KamTomlError::UnsupportedArch(arch_s, valid_archs));
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
            ModuleType::Kam => {}
        }

        Ok(kt)
    }

    pub fn write_to_file(&self, path: &Path) -> Result<(), KamTomlError> {
        let content = self.to_toml()?;
        std::fs::write(path, content)?;
        Ok(())
    }

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
