use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use crate::types::kam_toml::dependency::{Dependency, DependencySection};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub enum ModuleType {
    Normal,
    Template,
    Library,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct VariableDefinition {
    pub var_type: String,
    pub required: bool,
    pub default: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct TmplSection {
    pub used_template: Option<String>,
    pub variables: BTreeMap<String, VariableDefinition>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct LibSection {
    pub dependencies: Option<Vec<Dependency>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct KamSection {
    pub min_api: Option<u32>,
    pub max_api: Option<u32>,
    pub supported_arch: Option<Vec<String>>,
    pub conflicts: Option<Vec<String>>,
    pub dependency: Option<DependencySection>,
    pub build: Option<BuildSection>,
    pub module_type: ModuleType,
    pub tmpl: Option<TmplSection>,
    pub lib: Option<LibSection>,
}

impl Default for KamSection {
    fn default() -> Self {
        KamSection {
            min_api: None,
            max_api: None,
            supported_arch: None,
            conflicts: None,
            dependency: None,
            build: None,
            module_type: ModuleType::Normal,
            tmpl: None,
            lib: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct BuildSection {
    pub target_dir: Option<String>,
    pub output_file: Option<String>,
    pub pre_build: Option<String>,
    pub post_build: Option<String>,
}
