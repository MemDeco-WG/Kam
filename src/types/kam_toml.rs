use chrono;
use serde::{Deserialize, Serialize};
use toml;

pub mod sections;
use sections::*;

pub mod enums;

// KamToml：module.prop、update.json和其他元数据的超集
// 设计灵感来自pyproject.toml，使用分层section结构
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct KamToml {
    pub prop: PropSection,
    pub mmrl: Option<MmrlSection>,
    pub kam: KamSection,

    pub tmpl: Option<TmplSection>,
    pub tool: Option<ToolSection>,
    // lib字段在kam.lib!
    #[serde(skip)]
    pub raw: String,
}
impl Default for KamToml {
    fn default() -> Self {
        // Use defaults from section Default impls where appropriate.
        let mut default = Self::from_prop(PropSection::default());
        default.mmrl = Some(MmrlSection::default());
        default.kam = KamSection::default();
        default.raw = "".to_string();
        default
    }
}

impl KamToml {
    pub fn generate_description(module_type: &enums::ModuleType) -> String {
        match module_type {
            enums::ModuleType::Kam => "A kam module",
            enums::ModuleType::Template => "A template module",
        }
        .to_string()
    }

    // 从PropSection构造KamToml（用于默认值组合）
    // 这个辅助函数保持和其他构造函数相同的签名
    pub fn from_prop(prop: PropSection) -> Self {
        Self {
            prop,
            mmrl: Some(MmrlSection::default()),
            kam: KamSection::default(),
            tmpl: Some(TmplSection::default()),
            tool: Some(ToolSection::default()),
            raw: String::new(),
        }
    }

    /// Create a new KamToml with current timestamp for versionCode
    pub fn new_with_current_timestamp(
        id: String,
        name: String,
        version: String,
        author: Option<String>,
        description: String,
        update_json: Option<String>,
        module_type: Option<enums::ModuleType>,
    ) -> Self {
        let mut kt = Self::from_prop(PropSection {
            id,
            name,
            version,
            versionCode: chrono::Utc::now().timestamp_millis(),
            author,
            description,
            updateJson: update_json,
            metamodule: false,
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
        let path_buf = path.as_ref().to_path_buf();
        let content = std::fs::read_to_string(&path_buf).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                crate::errors::KamTomlError::NotFound(path_buf.display().to_string()).into()
            } else {
                crate::errors::KamError::Io(e)
            }
        })?;
        let mut kt: Self = toml::from_str(&content)?;
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
        // If `raw` is empty, fall back to the current struct by serializing it to TOML
        let mut value: toml::Value = if self.raw.trim().is_empty() {
            let s = toml::to_string_pretty(self)?;
            toml::from_str(&s)?
        } else {
            toml::from_str(&self.raw)?
        };
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
        for (_i, &part) in parts.iter().enumerate().take(parts.len() - 1) {
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
}
