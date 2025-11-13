use crate::assets::tmpl::TmplAssets;
use crate::cache::KamCache;
use crate::errors::KamError;
use crate::types::kam_toml::sections::tmpl::VariableDefinition;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Template manager for handling built-in templates
pub struct TemplateManager;

impl TemplateManager {
    /// Ensure a specific template archive is available in the cache
    ///
    /// This will check if the template archive exists in cache/tmpl/<template>.tar.gz
    /// If not, copy it from embedded assets to cache
    /// Does not extract the archive, only ensures the .tar.gz file is present
    pub fn ensure_template(template: &str) -> Result<(), KamError> {
        let cache = KamCache::new()?;
        let tmpl_dir = cache.tmpl_dir();
        let archive_path = tmpl_dir.join(format!("{}.tar.gz", template));

        // If archive already exists, skip
        if archive_path.exists() {
            return Ok(());
        }

        // Ensure tmpl_dir exists
        fs::create_dir_all(&tmpl_dir)?;

        // Try to get from embedded assets
        let asset_name = format!("{}.tar.gz", template);
        if let Some(content) = TmplAssets::get(&asset_name) {
            // Save the archive to cache
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
            .filter_map(|name| {
                name.strip_suffix(".tar.gz").map(|s| s.to_string())
            })
            .collect()
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

    /// Parse template variable definitions from CLI arguments
    pub fn parse_template_variables(vars: &[String]) -> Result<HashMap<String, VariableDefinition>, KamError> {
        let mut variables = HashMap::new();
        for var in vars {
            if let Some((key, value)) = var.split_once('=') {
                // Accept an optional fourth field as a human-friendly note/message.
                // Format: type:required:default[:note]
                let mut parts_iter = value.splitn(4, ':');
                let var_type = parts_iter.next().unwrap_or("").to_string();
                let required = parts_iter.next().unwrap_or("") == "true";
                let default_part = parts_iter.next().unwrap_or("");
                let default = if default_part.is_empty() {
                    None
                } else {
                    Some(default_part.to_string())
                };
                let note = parts_iter.next().map(|s| s.to_string());
                variables.insert(
                    key.to_string(),
                    VariableDefinition {
                        var_type,
                        required,
                        default,
                        note,
                        help: None,
                        example: None,
                        choices: None,
                    },
                );
            } else {
                return Err(KamError::InvalidVarFormat(format!(
                    "Invalid template variable format: {}. Expected key=type:required:default",
                    var
                )));
            }
        }
        Ok(variables)
    }

    /// Copy template files from src directory to dst directory, replacing placeholders
    pub fn copy_template_to(
        src: &Path,
        dst: &Path,
        vars: &HashMap<String, String>,
        force: bool,
        id: &str,
    ) -> Result<(), KamError> {
        Self::copy_and_replace(src, dst, vars, force, id)
    }

    fn copy_and_replace(
        src: &Path,
        dst: &Path,
        vars: &HashMap<String, String>,
        force: bool,
        id: &str,
    ) -> Result<(), KamError> {
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let file_name = entry.file_name().into_string().map_err(|_| {
                KamError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid filename",
                ))
            })?;
            if file_name == "kam.toml" {
                continue;
            }
            let replaced_name = Self::replace_placeholders(&file_name, vars);
            if file_name == "src" && entry.file_type()?.is_dir() {
                Self::copy_and_replace(&entry.path(), dst, vars, force, id)?;
            } else if replaced_name == id && entry.file_type()?.is_dir() {
                Self::copy_and_replace(&entry.path(), dst, vars, force, id)?;
            } else {
                let dst_path = dst.join(&replaced_name);
                let rel_path = dst_path
                    .strip_prefix(dst)
                    .unwrap_or(&dst_path)
                    .to_string_lossy()
                    .to_string();

                if entry.file_type()?.is_dir() {
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
                    let replaced_content = Self::replace_placeholders(&content, vars);
                    crate::utils::Utils::print_status(
                        &dst_path,
                        &rel_path,
                        crate::utils::PrintOp::Create { is_dir: false },
                        force,
                    );
                    std::fs::write(&dst_path, replaced_content)?;
                }
            }
        }
        Ok(())
    }

    fn replace_placeholders(text: &str, vars: &HashMap<String, String>) -> String {
        let mut result = text.to_string();
        for (k, v) in vars {
            result = result.replace(&format!("{{{{{}}}}}", k), v);
        }
        result
    }
}
