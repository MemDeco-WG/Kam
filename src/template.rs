use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tera::{Context, Tera};
use walkdir::WalkDir;

// Import assets
use crate::assets::tmpl::TmplAssets;
use crate::utils::{PrintOp, Utils};

pub struct TemplateCacheManager;

impl TemplateCacheManager {
    pub fn get_cache_dir() -> Result<PathBuf, KamError> {
        let home = dirs::home_dir().ok_or_else(|| {
            KamError::InvalidDirectory("Could not determine home directory".to_string())
        })?;
        let cache_dir = home.join(".kam").join("templates");
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(KamError::Io)?;
        }
        Ok(cache_dir)
    }

    /// List all available templates (built-in + cached)
    pub fn list_templates() -> Vec<String> {
        let mut templates = HashSet::new();

        // 1. Built-in templates from assets
        for file in TmplAssets::iter() {
            let filename = file.as_ref();
            if filename.ends_with(".tar.gz") {
                if let Some(name) = filename.strip_suffix(".tar.gz") {
                    templates.insert(name.to_string());
                }
            } else if filename.ends_with(".zip") {
                if let Some(name) = filename.strip_suffix(".zip") {
                    templates.insert(name.to_string());
                }
            }
        }

        // 2. Local cache templates
        if let Ok(cache_dir) = Self::get_cache_dir() {
            if let Ok(entries) = fs::read_dir(cache_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        // Handle .tar.gz special case for stem
                        let name = if path.to_string_lossy().ends_with(".tar.gz") {
                            stem.strip_suffix(".tar").unwrap_or(stem)
                        } else {
                            stem
                        };
                        templates.insert(name.to_string());
                    }
                }
            }
        }

        let mut list: Vec<String> = templates.into_iter().collect();
        list.sort();
        list
    }

    /// Check if a template exists (built-in or cached)
    pub fn ensure_template(template: &str) -> Result<(), KamError> {
        let list = Self::list_templates();
        if list.contains(&template.to_string()) {
            Ok(())
        } else {
            Err(KamError::TemplateNotFound(format!(
                "Template '{}' not found in built-in assets or local cache",
                template
            )))
        }
    }

    /// Get path to template archive/directory.
    pub fn resolve_template_path(template: &str) -> Result<Option<PathBuf>, KamError> {
        let cache_dir = Self::get_cache_dir()?;

        // Check for directory
        let dir_path = cache_dir.join(template);
        if dir_path.exists() && dir_path.is_dir() {
            return Ok(Some(dir_path));
        }

        // Check for archives
        let extensions = [".tar.gz", ".tgz", ".zip"];
        for ext in extensions {
            let archive_path = cache_dir.join(format!("{}{}", template, ext));
            if archive_path.exists() {
                return Ok(Some(archive_path));
            }
        }

        Ok(None)
    }

    /// List local cached templates
    pub fn list_local_templates() -> Result<Vec<String>, KamError> {
        let cache_dir = Self::get_cache_dir()?;
        let mut templates = Vec::new();
        if cache_dir.exists() {
            for entry in fs::read_dir(cache_dir).map_err(KamError::Io)? {
                let entry = entry.map_err(KamError::Io)?;
                let path = entry.path();
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let name = if path.to_string_lossy().ends_with(".tar.gz") {
                        stem.strip_suffix(".tar").unwrap_or(stem)
                    } else {
                        stem
                    };
                    templates.push(name.to_string());
                }
            }
        }
        templates.sort();
        Ok(templates)
    }

    /// Install a template from a local path (directory or archive)
    pub fn install_template(name: &str, source: &Path) -> Result<(), KamError> {
        let cache_dir = Self::get_cache_dir()?;

        if source.is_dir() {
            let dest = cache_dir.join(name);
            if dest.exists() {
                return Err(KamError::CommandFailed(format!(
                    "Template '{}' already exists in cache",
                    name
                )));
            }
            crate::utils::copy_dir_all(source, &dest).map_err(KamError::Io)?;
        } else if source.is_file() {
            let filename = source
                .file_name()
                .ok_or_else(|| KamError::InvalidDirectory("Invalid source filename".to_string()))?
                .to_string_lossy();

            let dest_name = if filename.ends_with(".tar.gz") {
                format!("{}.tar.gz", name)
            } else if let Some(ext) = source.extension().and_then(|s| s.to_str()) {
                format!("{}.{}", name, ext)
            } else {
                name.to_string()
            };

            let dest = cache_dir.join(dest_name);
            if dest.exists() {
                return Err(KamError::CommandFailed(format!(
                    "Template '{}' already exists in cache",
                    name
                )));
            }
            fs::copy(source, &dest).map_err(KamError::Io)?;
        } else {
            return Err(KamError::InvalidDirectory(format!(
                "Source '{}' does not exist",
                source.display()
            )));
        }
        Ok(())
    }

    /// Remove a template from cache
    pub fn remove_template(name: &str) -> Result<(), KamError> {
        let cache_dir = Self::get_cache_dir()?;

        // Check for directory
        let dir_path = cache_dir.join(name);
        if dir_path.exists() && dir_path.is_dir() {
            fs::remove_dir_all(&dir_path).map_err(KamError::Io)?;
            return Ok(());
        }

        // Check for archives
        let extensions = [".tar.gz", ".tgz", ".zip"];
        for ext in extensions {
            let archive_path = cache_dir.join(format!("{}{}", name, ext));
            if archive_path.exists() {
                fs::remove_file(&archive_path).map_err(KamError::Io)?;
                return Ok(());
            }
        }

        Err(KamError::TemplateNotFound(format!(
            "Template '{}' not found in cache",
            name
        )))
    }
}

pub struct TemplateVariableProcessor;

impl TemplateVariableProcessor {
    pub fn flatten_kam_toml(kam_toml: &KamToml) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        // Top-level shortcuts
        vars.insert("id".to_string(), kam_toml.prop.id.clone());
        vars.insert("name".to_string(), kam_toml.prop.get_name().to_string());
        vars.insert("version".to_string(), kam_toml.prop.version.clone());
        vars.insert(
            "versionCode".to_string(),
            kam_toml.prop.versionCode.to_string(),
        );
        vars.insert("author".to_string(), kam_toml.prop.author.clone());
        vars.insert(
            "description".to_string(),
            kam_toml.prop.get_description().to_string(),
        );

        if let Some(uj) = &kam_toml.prop.updateJson {
            vars.insert("update_json".to_string(), uj.clone());
        }

        // Prop section
        vars.insert("prop.id".to_string(), kam_toml.prop.id.clone());
        vars.insert(
            "prop.name".to_string(),
            kam_toml.prop.get_name().to_string(),
        );
        vars.insert("prop.version".to_string(), kam_toml.prop.version.clone());
        vars.insert(
            "prop.versionCode".to_string(),
            kam_toml.prop.versionCode.to_string(),
        );
        vars.insert("prop.author".to_string(), kam_toml.prop.author.clone());
        vars.insert(
            "prop.description".to_string(),
            kam_toml.prop.get_description().to_string(),
        );

        vars
    }

    pub fn parse_template_vars(vars: &[String]) -> Result<HashMap<String, String>, KamError> {
        let mut map = HashMap::new();
        for var in vars {
            if let Some((k, v)) = var.split_once('=') {
                map.insert(k.to_string(), v.to_string());
            } else {
                map.insert(var.to_string(), "".to_string());
            }
        }
        Ok(map)
    }
}

pub struct TemplateCopier;

impl TemplateCopier {
    pub fn copy_and_replace(
        src: &Path,
        dst: &Path,
        vars: &HashMap<String, String>,
        force: bool,
        _template_id: &str,
    ) -> Result<(), KamError> {
        if !src.exists() {
            return Err(KamError::InvalidDirectory(format!(
                "Source does not exist: {}",
                src.display()
            )));
        }

        // Build Tera context by unflattening the variables
        let context_value = unflatten_vars(vars);
        let context = Context::from_serialize(&context_value).map_err(|e| {
            KamError::CommandFailed(format!("Failed to build template context: {}", e))
        })?;

        let mut tera = Tera::default();
        tera.set_escape_fn(|s| s.to_string()); // Disable auto-escaping for file content

        for entry in WalkDir::new(src) {
            let entry = entry.map_err(|e| KamError::Io(e.into()))?;
            let src_path = entry.path();

            if src_path == src {
                continue;
            }

            // Calculate relative path
            let rel_path = src_path
                .strip_prefix(src)
                .map_err(|e| KamError::StripPrefixFailed(e.to_string()))?;

            // Replace variables in filename
            let rel_path_str = rel_path.to_string_lossy();
            let mut dest_rel_path_str = rel_path_str.to_string();

            // Simple string replacement for filenames
            for (k, v) in vars {
                let placeholder = format!("{{{{{}}}}}", k); // {{key}}
                dest_rel_path_str = dest_rel_path_str.replace(&placeholder, v);
            }

            // Also try to render filename with Tera if it contains {{
            if dest_rel_path_str.contains("{{") {
                if let Ok(rendered) = tera.render_str(&dest_rel_path_str, &context) {
                    dest_rel_path_str = rendered;
                }
            }

            let dst_path = dst.join(&dest_rel_path_str);

            if entry.file_type().is_dir() {
                // Create directory silently without printing
                fs::create_dir_all(&dst_path).map_err(KamError::Io)?;
            } else if entry.file_type().is_file() {
                // Ensure parent exists
                if let Some(parent) = dst_path.parent() {
                    fs::create_dir_all(parent).map_err(KamError::Io)?;
                }

                if dst_path.exists() && !force {
                    Utils::print_status(&dst_path, &dest_rel_path_str, PrintOp::Skip, force);
                    continue;
                }

                // Check if file exists before writing to determine correct status
                let file_existed = dst_path.exists();

                // Check if binary: explicitly skip binary files while applying template
                if is_binary(src_path) {
                    Utils::print_status(&dst_path, &dest_rel_path_str, PrintOp::Skip, force);
                    continue;
                } else {
                    // Text file - perform substitution
                    let content = fs::read_to_string(src_path);
                    match content {
                        Ok(text) => {
                            match tera.render_str(&text, &context) {
                                Ok(rendered) => {
                                    fs::write(&dst_path, rendered).map_err(KamError::Io)?;
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Warning: Failed to render template '{}': {}",
                                        src_path.display(),
                                        e
                                    );
                                    // Fallback to copy
                                    fs::copy(src_path, &dst_path).map_err(KamError::Io)?;
                                }
                            }
                        }
                        Err(_) => {
                            // Fallback to copy if read_to_string fails
                            fs::copy(src_path, &dst_path).map_err(KamError::Io)?;
                        }
                    }
                }

                // Print appropriate status based on whether file existed before
                if file_existed && force {
                    Utils::print_status(&dst_path, &dest_rel_path_str, PrintOp::Update, force);
                } else if !file_existed {
                    Utils::print_status(
                        &dst_path,
                        &dest_rel_path_str,
                        PrintOp::Create { is_dir: false },
                        force,
                    );
                }
            }
        }
        Ok(())
    }
}

fn unflatten_vars(vars: &HashMap<String, String>) -> Value {
    let mut root = serde_json::Map::new();

    for (key, value) in vars {
        if key.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = key.split('.').collect();
        let mut current = &mut root;

        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // Leaf
                current.insert(part.to_string(), Value::String(value.clone()));
            } else {
                // Node
                if !current.contains_key(*part) || !current[*part].is_object() {
                    current.insert(part.to_string(), Value::Object(serde_json::Map::new()));
                }
                current = current.get_mut(*part).unwrap().as_object_mut().unwrap();
            }
        }
    }
    Value::Object(root)
}

fn is_binary(path: &Path) -> bool {
    // Simple heuristic: read first few bytes and check for null byte
    if let Ok(mut file) = fs::File::open(path) {
        use std::io::Read;
        let mut buffer = [0; 1024];
        if let Ok(n) = file.read(&mut buffer) {
            for b in &buffer[..n] {
                if *b == 0 {
                    return true;
                }
            }
        }
    }
    // Check extension
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let binary_exts = [
            "png", "jpg", "jpeg", "gif", "ico", "zip", "tar", "gz", "so", "a", "o", "bin", "exe",
        ];
        if binary_exts.contains(&ext.to_lowercase().as_str()) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::io::Write;

    #[test]
    fn test_copy_and_replace_skips_binary() {
        let src_dir = tempdir().expect("tempdir");
        let dst_dir = tempdir().expect("tempdir");

        let binary_file = src_dir.path().join("image.png");
        let mut bf = std::fs::File::create(&binary_file).unwrap();
        bf.write_all(&[0u8, 0u8, 0u8, 0u8]).unwrap(); // include null byte

        let text_file = src_dir.path().join("README.md");
        let mut tf = std::fs::File::create(&text_file).unwrap();
        tf.write_all(b"Hello {{name}}\n").unwrap();

        let mut vars = std::collections::HashMap::new();
        vars.insert("name".to_string(), "World".to_string());

        TemplateManager::copy_and_replace(src_dir.path(), dst_dir.path(), &vars, true, "test")
            .expect("copy_and_replace");

        // Binary should be skipped
        assert!(!dst_dir.path().join("image.png").exists());

        // Text should be present and rendered
        let rendered = std::fs::read_to_string(dst_dir.path().join("README.md")).unwrap();
        assert!(rendered.contains("Hello World"));
    }
}

pub struct TemplateManager;

impl TemplateManager {
    pub fn ensure_template(template: &str) -> Result<(), KamError> {
        TemplateCacheManager::ensure_template(template)
    }

    pub fn list_builtin_templates() -> Vec<String> {
        TemplateCacheManager::list_templates()
    }

    pub fn parse_template_vars(vars: &[String]) -> Result<HashMap<String, String>, KamError> {
        TemplateVariableProcessor::parse_template_vars(vars)
    }

    pub fn copy_and_replace(
        src: &Path,
        dst: &Path,
        vars: &HashMap<String, String>,
        force: bool,
        template_id: &str,
    ) -> Result<(), KamError> {
        TemplateCopier::copy_and_replace(src, dst, vars, force, template_id)
    }
}
