pub mod error;
pub mod sections;
pub mod implementations;
#[cfg(test)]
pub mod tests;

pub use error::*;
pub use sections::*;


use serde::{Deserialize, Serialize};
use self::sections::prop::PropSection;
use self::sections::mmrl::MmrlSection;
use self::sections::module::KamSection;

const DEFAULT_DEPENDENCY_SOURCE: &str = "https://github.com/MemDeco-WG/Kam-Index";

/// KamToml: A superset of module.prop, update.json, and other metadata,
/// inspired by pyproject.toml format with hierarchical sections.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct KamToml {
    pub prop: PropSection,
    pub mmrl: Option<MmrlSection>,
    pub kam: KamSection,
    pub tool: Option<serde_json::Value>,
    pub tmpl: Option<serde_json::Value>,
    pub lib: Option<serde_json::Value>,
    #[serde(skip)]
    pub raw: String,
}

impl Default for KamToml {
    fn default() -> Self {
        let mut name = std::collections::BTreeMap::new();
        name.insert("en".to_string(), "My Module".to_string());
        let mut description = std::collections::BTreeMap::new();
        description.insert("en".to_string(), "A module description".to_string());
        let keywords = vec![
            "android".to_string(),
            "root".to_string(),
            "module".to_string(),
        ];
        let categories = vec!["Kam".to_string()];

        KamToml {
            prop: PropSection {
                id: "my_module".to_string(),
                name,
                version: "0.1.0".to_string(),
                versionCode: 1,
                author: "Author".to_string(),
                description,
                updateJson: Some("https://example.com/update.json".to_string()),
            },
            mmrl: Some(MmrlSection {
                repo: Some(crate::types::kam_toml::sections::mmrl::RepoSection {
                    license: Some("MIT".to_string()),
                    homepage: Some("https://github.com/example/repo".to_string()),
                    readme: Some("https://github.com/example/repo#readme".to_string()),
                    screenshots: None,
                    categories: Some(categories),
                    keywords: Some(keywords),
                    maintainers: None,
                    repository: None,
                    documentation: None,
                    issues: None,
                    funding: None,
                    support: Some("https://github.com/example/repo/issues".to_string()),
                    donate: Some("https://paypal.me/example".to_string()),
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
                }),
            }),
            kam: KamSection {
                min_api: None,
                max_api: None,
                supported_arch: None,
                conflicts: None,
                dependency: None,
                build: None,
                module_type: crate::types::kam_toml::sections::module::ModuleType::Normal,
                tmpl: None,
                lib: None,
            },
            tool: None,
            tmpl: None,
            lib: None,
            raw: "".to_string(),
        }
    }
}
