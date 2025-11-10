pub mod sections;
pub mod implementations;
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
        // Use defaults from section Default impls where appropriate.
        let mut default = KamToml::from_prop(PropSection::default());
        default.mmrl = Some(crate::types::kam_toml::sections::mmrl::MmrlSection::default());
        default.kam = crate::types::kam_toml::sections::module::KamSection::default();
        default.raw = "".to_string();
        default
    }
}

impl KamToml {
    /// Construct a KamToml starting from a PropSection (useful for default
    /// composition). This helper keeps the same signature shape as other
    /// constructors in this module.
    pub fn from_prop(prop: PropSection) -> Self {
        KamToml {
            prop,
            mmrl: Some(crate::types::kam_toml::sections::mmrl::MmrlSection::default()),
            kam: crate::types::kam_toml::sections::module::KamSection::default(),
            tool: None,
            tmpl: None,
            lib: None,
            raw: String::new(),
        }
    }
}
