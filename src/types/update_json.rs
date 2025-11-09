use serde::{Deserialize, Serialize};
use regex::Regex;
use std::borrow::Cow;
use std::path::Path;

/// UpdateJson: Subset of KamToml for update.json fields
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct UpdateJson<'a> {
    pub version: Cow<'a, str>,
    pub versionCode: u64,
    pub zipUrl: Option<Cow<'a, str>>,
    pub changelog: Option<Cow<'a, str>>,
}

impl<'a> crate::types::traits::KamConvertible<'a> for UpdateJson<'a> {
    fn from_kam(kam: &'a crate::types::kam_toml::KamToml) -> Self {
        Self::from_kam_toml(kam)
    }

    fn to_kam(&self) -> crate::types::kam_toml::KamToml {
        // Create a minimal KamToml with mmrl section
        let mut name = std::collections::HashMap::new();
        name.insert("en".to_string(), "Generated Module".to_string());
        let mut description = std::collections::HashMap::new();
        description.insert("en".to_string(), "Generated from UpdateJson".to_string());

        let mut kt = crate::types::kam_toml::KamToml::new(
            "generated_module".to_string(),
            name,
            self.version.to_string(),
            self.versionCode,
            "Unknown".to_string(),
            description,
            None,
        );

        kt.mmrl = Some(crate::types::kam_toml::MmrlSection {
            zip_url: self.zipUrl.as_ref().map(|s| s.to_string()),
            changelog: self.changelog.as_ref().map(|s| s.to_string()),
            license: None,
            homepage: None,
            readme: None,
            screenshots: None,
            categories: None,
            keywords: None,
            maintainers: None,
            repository: None,
            documentation: None,
            issues: None,
            funding: None,
            support: None,
            donate: None,
            cover: None,
            icon: None,
            devices: None,
            arch: None,
            require: None,
            note: None,
            manager: None,
            antifeatures: None,
            options: None,
            max_num: None,
            min_api: None,
            max_api: None,
            verified: None,
            features: None,
        });

        kt
    }
}

#[allow(non_snake_case)]
impl<'a> UpdateJson<'a> {
    /// Create a new UpdateJson instance
    pub fn new(version: &'a str, versionCode: u64, zipUrl: Option<&'a str>, changelog: Option<&'a str>) -> UpdateJson<'a> {
        UpdateJson {
            version: Cow::Borrowed(version),
            versionCode,
            zipUrl: zipUrl.map(Cow::Borrowed),
            changelog: changelog.map(Cow::Borrowed),
        }
    }

    /// Create from KamToml (extract mmrl fields)
    pub fn from_kam_toml(kt: &'a crate::types::kam_toml::KamToml) -> UpdateJson<'a> {
        UpdateJson {
            version: Cow::Borrowed(&kt.prop.version),
            versionCode: kt.prop.versionCode,
            zipUrl: kt.mmrl.as_ref().and_then(|m| m.zip_url.as_deref()).map(Cow::Borrowed),
            changelog: kt.mmrl.as_ref().and_then(|m| m.changelog.as_deref()).map(Cow::Borrowed),
        }
    }

    /// Validate version format (semver-like)
    pub fn validate_version(version: &str) -> bool {
        let re = Regex::new(r"^\d+\.\d+\.\d+$").unwrap();
        re.is_match(version)
    }

    /// Load from JSON string
    pub fn from_json(content: &str) -> Result<UpdateJson<'static>, serde_json::Error> {
        serde_json::from_str(content)
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Write to file
    pub fn write_to_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let content = self.to_json()?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Load from file
    pub fn load_from_file(path: &Path) -> Result<UpdateJson<'static>, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&content)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::kam_toml::{KamToml, MmrlSection};
    use crate::types::traits::KamConvertible;

    #[test]
    fn test_new() {
        let uj = UpdateJson::new("1.0.0", 100, Some("url"), Some("log"));
        assert_eq!(uj.version, "1.0.0");
        assert_eq!(uj.versionCode, 100);
    }

    #[test]
    fn test_validate_version() {
        assert!(UpdateJson::validate_version("1.0.0"));
        assert!(!UpdateJson::validate_version("1.0"));
    }

    #[test]
    fn test_from_kam_toml() {
        let mut name = std::collections::HashMap::new();
        name.insert("en".to_string(), "name".to_string());
        let mut description = std::collections::HashMap::new();
        description.insert("en".to_string(), "desc".to_string());
        let kt = KamToml::new("id".to_string(), name, "1.0.0".to_string(), 100, "author".to_string(), description, None);
        let uj = UpdateJson::from_kam_toml(&kt);
        assert_eq!(uj.version, "1.0.0");
        assert_eq!(uj.versionCode, 100);
    }

    #[test]
    fn test_serialization() {
        let uj = UpdateJson::new("1.0.0", 100, Some("url"), None);
        let json = uj.to_json().unwrap();
        let de: UpdateJson<'static> = UpdateJson::from_json(&json).unwrap();
        assert_eq!(uj.version, de.version);
    }

    #[test]
    fn test_from_kam() {
        let mut name = std::collections::HashMap::new();
        name.insert("en".to_string(), "Test Module".to_string());
        let mut description = std::collections::HashMap::new();
        description.insert("en".to_string(), "Test Description".to_string());
        let mut kt = KamToml::new("test_id".to_string(), name, "1.0.0".to_string(), 123, "author".to_string(), description, None);
        kt.mmrl = Some(MmrlSection {
            zip_url: Some("https://example.com/zip".to_string()),
            changelog: Some("https://example.com/log".to_string()),
            license: None,
            homepage: None,
            readme: None,
            screenshots: None,
            categories: None,
            keywords: None,
            maintainers: None,
            repository: None,
            documentation: None,
            issues: None,
            funding: None,
            support: None,
            donate: None,
            cover: None,
            icon: None,
            devices: None,
            arch: None,
            require: None,
            note: None,
            manager: None,
            antifeatures: None,
            options: None,
            max_num: None,
            min_api: None,
            max_api: None,
            verified: None,
            features: None,
        });
        let uj = UpdateJson::from_kam(&kt);
        assert_eq!(uj.version, "1.0.0");
        assert_eq!(uj.versionCode, 123);
        assert_eq!(uj.zipUrl.as_deref(), Some("https://example.com/zip"));
        assert_eq!(uj.changelog.as_deref(), Some("https://example.com/log"));
    }

    #[test]
    fn test_to_kam() {
        let uj = UpdateJson {
            version: Cow::Borrowed("2.0.0"),
            versionCode: 200,
            zipUrl: Some(Cow::Borrowed("zip_url")),
            changelog: Some(Cow::Borrowed("changelog_url")),
        };
        let kt = uj.to_kam();
        assert_eq!(kt.prop.version, "2.0.0");
        assert_eq!(kt.prop.versionCode, 200);
        assert_eq!(kt.mmrl.as_ref().unwrap().zip_url, Some("zip_url".to_string()));
        assert_eq!(kt.mmrl.as_ref().unwrap().changelog, Some("changelog_url".to_string()));
    }
}
