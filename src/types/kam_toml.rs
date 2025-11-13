use chrono;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use toml;

pub mod sections;
use sections::*;

use crate::types::modules::DEFAULT_DEPENDENCY_SOURCE;

pub mod enums;

/// Workspace section for Kam workspace management, similar to Cargo workspaces
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct WorkspaceSection {
    /// List of workspace members (paths relative to the workspace root)
    pub members: Option<Vec<String>>,
    /// List of paths to exclude from the workspace
    pub exclude: Option<Vec<String>>,
}

/// KamToml: A superset of module.prop, update.json, and other metadata,
/// inspired by pyproject.toml format with hierarchical sections.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct KamToml {
    pub prop: PropSection,
    pub mmrl: Option<MmrlSection>,
    pub kam: KamSection,

    pub tool: Option<ToolSection>,
    pub tmpl: Option<TmplSection>,
    // lib字段在kam.lib!
    pub raw: String,
}
impl Default for KamToml {
    fn default() -> Self {
        // Use defaults from section Default impls where appropriate.
        let mut default = KamToml::from_prop(PropSection::default());
        default.mmrl = Some(MmrlSection::default());
        default.kam = KamSection::default();
        default.raw = "".to_string();
        default
    }
}

impl KamToml {
    /// Construct a KamToml starting from a PropSection (useful for default
    /// composition). This helper keeps the same signature as other
    /// constructors in this module.
    pub fn from_prop(prop: PropSection) -> Self {
        KamToml {
            prop,
            mmrl: Some(MmrlSection::default()),
            kam: KamSection::default(),
            tool: Some(ToolSection::default()),
            tmpl: Some(TmplSection::default()),
            lib: Some(LibSection::default()),
            raw: String::new(),
        }
    }

    /// Create a new KamToml with current timestamp for versionCode
    pub fn new_with_current_timestamp(
        id: String,
        name: BTreeMap<String, String>,
        version: String,
        author: String,
        description: BTreeMap<String, String>,
        update_json: Option<String>,
        module_type: Option<ModuleType>,
    ) -> Self {
        let mut kt = KamToml::from_prop(PropSection {
            id,
            name,
            version,
            versionCode: chrono::Utc::now().timestamp_millis(),
            author,
            description,
            updateJson: update_json,
        });
        if let Some(mt) = module_type {
            kt.kam.module_type = mt;
        }
        kt
    }

    /// Load KamToml from a directory (looks for kam.toml)
    pub fn load_from_dir<P: AsRef<std::path::Path>>(dir: P) -> crate::errors::Result<Self> {
        let path = dir.as_ref().join("kam.toml");
        Self::load_from_file(path)
    }

    /// Load KamToml from a file
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> crate::errors::Result<Self> {
        println!("DEBUG: current_dir: {:?}", std::env::current_dir());
        let content = std::fs::read_to_string(path)?;
        let mut kt: KamToml = toml::from_str(&content)?;
        kt.raw = content;
        Ok(kt)
    }

    /// Write KamToml to a directory as kam.toml
    pub fn write_to_dir<P: AsRef<std::path::Path>>(&self, dir: P) -> crate::errors::Result<()> {
        let path = dir.as_ref().join("kam.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Apply template variables to the KamToml structure
    pub fn apply_vars(&mut self, kam_vars: Vec<(String, String)>) -> crate::errors::Result<()> {
        let mut value: toml::Value = toml::from_str(&self.raw)?;
        for (key, val) in kam_vars {
            let key = key.strip_prefix('#').unwrap_or(&key);
            Self::set_value_by_path(&mut value, key, &val);
        }
        self.raw = toml::to_string_pretty(&value)?;
        *self = toml::from_str(&self.raw)?;
        Ok(())
    }

    fn set_value_by_path(value: &mut toml::Value, path: &str, new_value: &str) {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value.as_table_mut().unwrap();
        for &part in &parts[..parts.len() - 1] {
            if !current.contains_key(part) {
                current.insert(part.to_string(), toml::Value::Table(Default::default()));
            }
            current = current[part].as_table_mut().unwrap();
        }
        let last = &parts[parts.len() - 1];
        if *last == "versionCode" {
            if let Ok(num) = new_value.parse::<i64>() {
                current.insert(last.to_string(), toml::Value::Integer(num));
            }
        } else {
            current.insert(last.to_string(), toml::Value::String(new_value.to_string()));
        }
    }

    /// Get effective source URL for dependencies
    pub fn get_effective_source(dep: &Dependency) -> String {
        dep.source
            .clone()
            .unwrap_or_else(|| DEFAULT_DEPENDENCY_SOURCE.to_string())
    }

    /// Resolve dependencies into flattened groups
    pub fn resolve_dependencies(&self) -> crate::errors::Result<sections::FlatDependencyGroups> {
        self.kam
            .dependency
            .as_ref()
            .unwrap_or(&DependencySection::default())
            .resolve()
    }
}
