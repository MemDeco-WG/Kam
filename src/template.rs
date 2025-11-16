use crate::assets::tmpl::TmplAssets;
use crate::cache::KamCache;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tera::{Context, Tera};
use walkdir::WalkDir;

/// Template cache manager - handles template availability
pub struct TemplateCacheManager;

impl TemplateCacheManager {
    /// Ensure a specific template archive is available in the cache
    pub fn ensure_template(template: &str) -> Result<(), KamError> {
        let cache = KamCache::new()?;
        let tmpl_dir = cache.tmpl_dir();
        let archive_path = tmpl_dir.join(format!("{}.tar.gz", template));

        if archive_path.exists() {
            return Ok(());
        }

        fs::create_dir_all(&tmpl_dir)?;

        let asset_name = format!("{}.tar.gz", template);
        if let Some(content) = TmplAssets::get(&asset_name) {
            fs::write(&archive_path, &content.data)?;
            Ok(())
        } else {
            Err(KamError::TemplateNotFound(format!(
                "Built-in template '{}' not found",
                template
            )))
        }
    }

    /// List all available built-in templates
    pub fn list_builtin_templates() -> Vec<String> {
        TmplAssets::iter()
            .filter_map(|name| name.strip_suffix(".tar.gz").map(|s| s.to_string()))
            .collect()
    }
}

/// Template variable processor - handles variable flattening and parsing
pub struct TemplateVariableProcessor;

impl TemplateVariableProcessor {
    /// Flatten KamToml into a HashMap of string keys and values for template variables
    pub fn flatten_kam_toml(kt: &KamToml) -> HashMap<String, String> {
        let value = serde_json::to_value(kt).unwrap();
        let mut vars = HashMap::new();
        Self::flatten_json("", &value, &mut vars);
        vars
    }

    /// Recursively flatten a serde_json::Value into a HashMap with dot-separated keys
    fn flatten_json(prefix: &str, value: &serde_json::Value, vars: &mut HashMap<String, String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    let new_prefix = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{}.{}", prefix, k)
                    };
                    Self::flatten_json(&new_prefix, v, vars);
                }
            }
            serde_json::Value::Array(arr) => {
                vars.insert(prefix.to_string(), serde_json::to_string(arr).unwrap());
            }
            serde_json::Value::String(s) => {
                vars.insert(prefix.to_string(), s.clone());
            }
            serde_json::Value::Number(n) => {
                vars.insert(prefix.to_string(), n.to_string());
            }
            serde_json::Value::Bool(b) => {
                vars.insert(prefix.to_string(), b.to_string());
            }
            serde_json::Value::Null => {}
        }
    }

    /// Parse template variables from CLI arguments
    pub fn parse_template_vars(vars: &[String]) -> Result<HashMap<String, String>, KamError> {
        let mut template_vars = HashMap::new();
        for var in vars {
            if let Some((key, value)) = var.split_once('=') {
                template_vars.insert(key.to_string(), value.to_string());
            } else {
                return Err(KamError::InvalidVarFormat(format!(
                    "Invalid template variable format: {}",
                    var
                )));
            }
        }
        Ok(template_vars)
    }
}

/// Template copier - handles copying and variable replacement
pub struct TemplateCopier;

impl TemplateCopier {
    /// Copy template files from src directory to dst directory, replacing placeholders
    pub fn copy_template_to(
        src: &Path,
        dst: &Path,
        kt: &KamToml,
        force: bool,
        id: &str,
    ) -> Result<(), KamError> {
        let vars = TemplateVariableProcessor::flatten_kam_toml(kt);
        Self::copy_and_replace(src, dst, &vars, force, id)
    }

    /// Copy template files from src directory to dst directory, replacing placeholders
    pub fn copy_and_replace(
        src: &Path,
        dst: &Path,
        vars: &HashMap<String, String>,
        force: bool,
        id: &str,
    ) -> Result<(), KamError> {
        let mut tera = Tera::default();
        tera.set_escape_fn(|s| s.to_string());
        let mut context = Context::new();
        for (k, v) in vars.iter() {
            context.insert(k, v);
        }

        // Safety check: prevent copying into a destination nested inside the source
        // directory. If `dst` is inside `src`, copying would cause new files to be
        // created under `src` while the WalkDir is still enumerating entries, which
        // leads to unbounded recursion and eventually stack overflow / infinite loop.
        //
        // We use the current working directory to create a stable absolute comparison
        // without requiring the paths to exist (canonicalize may fail for non-existent
        // destinations).
        let cwd = std::env::current_dir()?;
        let abs_src = if src.is_absolute() { src.to_path_buf() } else { cwd.join(src) };
        let abs_dst = if dst.is_absolute() { dst.to_path_buf() } else { cwd.join(dst) };
        if abs_dst.starts_with(&abs_src) {
            return Err(KamError::InvalidDirectory(format!(
                "Destination '{}' is inside the template source '{}': aborting to prevent recursive copy",
                abs_dst.display(),
                abs_src.display()
            )));
        }

        for entry in WalkDir::new(src) {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            let replaced_name = tera
                .render_str(&file_name, &context)
                .map_err(|e| KamError::CommandFailed(format!("Tera render error: {}", e)))?;
            let dst_path = dst.join(&replaced_name);
            let rel_path = dst_path
                .strip_prefix(dst)
                .unwrap_or(&dst_path)
                .to_string_lossy()
                .to_string();

            if entry.file_type().is_dir() {
                crate::utils::Utils::print_status(
                    &dst_path,
                    &rel_path,
                    crate::utils::PrintOp::Create { is_dir: true },
                    force,
                );
                std::fs::create_dir_all(&dst_path)?;
                Self::copy_and_replace(&entry.path(), &dst_path, vars, force, id)?;
            } else {
                let content = std::fs::read_to_string(entry.path())?;
                let replaced_content = tera
                    .render_str(&content, &context)
                    .map_err(|e| KamError::CommandFailed(format!("Tera render error: {}", e)))?;
                crate::utils::Utils::print_status(
                    &dst_path,
                    &rel_path,
                    crate::utils::PrintOp::Create { is_dir: false },
                    force,
                );
                std::fs::write(&dst_path, replaced_content)?;
            }
        }
        Ok(())
    }
}

/// Legacy TemplateManager for backward compatibility
pub struct TemplateManager;

impl TemplateManager {
    pub fn ensure_template(template: &str) -> Result<(), KamError> {
        TemplateCacheManager::ensure_template(template)
    }

    pub fn list_builtin_templates() -> Vec<String> {
        TemplateCacheManager::list_builtin_templates()
    }

    pub fn parse_template_vars(vars: &[String]) -> Result<HashMap<String, String>, KamError> {
        TemplateVariableProcessor::parse_template_vars(vars)
    }

    pub fn copy_template_to(
        src: &Path,
        dst: &Path,
        kt: &KamToml,
        force: bool,
        id: &str,
    ) -> Result<(), KamError> {
        TemplateCopier::copy_template_to(src, dst, kt, force, id)
    }

    pub fn copy_and_replace(
        src: &Path,
        dst: &Path,
        vars: &HashMap<String, String>,
        force: bool,
        id: &str,
    ) -> Result<(), KamError> {
        TemplateCopier::copy_and_replace(src, dst, vars, force, id)
    }
}
