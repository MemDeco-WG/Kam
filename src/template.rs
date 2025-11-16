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
    ///
    /// This wrapper canonicalizes `src` up-front and delegates actual work to an
    /// internal function so we keep a stable notion of the template root across
    /// recursion. The implementation:
    /// - converts flattened variables into a nested JSON/Tera context so templated
    ///   expressions like `{{prop.id}}` and `{{id}}` both work as expected,
    /// - prevents recursive copies by rejecting destinations that would be inside
    ///   the template source root, and
    /// - avoids writing potentially unsafe paths (e.g. '..' components),
    /// - supports both text (rendered) and binary (copied unchanged) files.
    pub fn copy_and_replace(
        src: &Path,
        dst: &Path,
        vars: &HashMap<String, String>,
        force: bool,
        id: &str,
    ) -> Result<(), KamError> {
        // Resolve a stable absolute root for the template source: prefer a canonicalized
        // path and fall back to normalizing against the current working directory.
        let cwd = std::env::current_dir()?;
        let root_src = std::fs::canonicalize(src).unwrap_or_else(|_| {
            if src.is_absolute() {
                src.to_path_buf()
            } else {
                cwd.join(src)
            }
        });

        Self::copy_and_replace_internal(src, dst, vars, force, id, &root_src)
    }

    fn copy_and_replace_internal(
        src: &Path,
        dst: &Path,
        vars: &HashMap<String, String>,
        force: bool,
        id: &str,
        root_src: &Path,
    ) -> Result<(), KamError> {
        // Build Tera instance and convert flattened vars into a nested context.
        let mut tera = Tera::default();
        tera.set_escape_fn(|s| s.to_string());

        // Build nested JSON object from flattened vars like "prop.id" -> { "prop": { "id": "..." } }
        let mut root_obj = serde_json::Map::new();
        for (k, v) in vars.iter() {
            if k.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = k.split('.').filter(|s| !s.is_empty()).collect();
            let mut cur = &mut root_obj;
            for (i, part) in parts.iter().enumerate() {
                let key = part.to_string();
                if i + 1 == parts.len() {
                    // leaf value
                    cur.insert(key, serde_json::Value::String(v.clone()));
                } else {
                    // intermediate object: create if missing
                    if !cur.contains_key(&key) {
                        cur.insert(key.clone(), serde_json::Value::Object(serde_json::Map::new()));
                    } else if !cur.get(&key).unwrap().is_object() {
                        // If a value already exists but isn't an object, replace it with an object
                        cur.insert(key.clone(), serde_json::Value::Object(serde_json::Map::new()));
                    }
                    cur = cur
                        .get_mut(&key)
                        .and_then(|v| v.as_object_mut())
                        .expect("expected object");
                }
            }
        }

        // Convenience shallow keys fallback (id/version/author/name/description).
        // This mirrors `init_impl` behaviour: templates can safely use `{{id}}` as
        // well as `{{prop.id}}`.
        if !root_obj.contains_key("id") {
            if let Some(prop) = root_obj.get("prop").and_then(|v| v.as_object()) {
                if let Some(idv) = prop.get("id").cloned() {
                    root_obj.insert("id".to_string(), idv);
                }
            }
        }

        if !root_obj.contains_key("version") {
            if let Some(prop) = root_obj.get("prop").and_then(|v| v.as_object()) {
                if let Some(ver) = prop.get("version").cloned() {
                    root_obj.insert("version".to_string(), ver);
                }
            }
        }

        if !root_obj.contains_key("author") {
            if let Some(prop) = root_obj.get("prop").and_then(|v| v.as_object()) {
                if let Some(a) = prop.get("author").cloned() {
                    root_obj.insert("author".to_string(), a);
                }
            }
        }

        // For `name` and `description`, extract `en` fallback or the first locale.
        if !root_obj.contains_key("name") {
            if let Some(prop) = root_obj.get("prop").and_then(|v| v.as_object()) {
                if let Some(namev) = prop.get("name") {
                    if namev.is_string() {
                        root_obj.insert("name".to_string(), namev.clone());
                    } else if let Some(map) = namev.as_object() {
                        if let Some(en) = map.get("en") {
                            root_obj.insert("name".to_string(), en.clone());
                        } else if let Some((_k, v)) = map.iter().next() {
                            root_obj.insert("name".to_string(), v.clone());
                        }
                    }
                }
            }
        }

        if !root_obj.contains_key("description") {
            if let Some(prop) = root_obj.get("prop").and_then(|v| v.as_object()) {
                if let Some(desc) = prop.get("description") {
                    if desc.is_string() {
                        root_obj.insert("description".to_string(), desc.clone());
                    } else if let Some(map) = desc.as_object() {
                        if let Some(en) = map.get("en") {
                            root_obj.insert("description".to_string(), en.clone());
                        } else if let Some((_k, v)) = map.iter().next() {
                            root_obj.insert("description".to_string(), v.clone());
                        }
                    }
                }
            }
        }

        let root_value = serde_json::Value::Object(root_obj);
        let mut context = Context::from_serialize(&root_value)
            .map_err(|e| KamError::CommandFailed(format!("Failed to build template context: {}", e)))?;

        // Safety check: prevent copying into a destination that is inside the initial template root
        // (e.g. calling `kam init .` with the destination placed inside template source).
        let cwd = std::env::current_dir()?;
        let abs_dst = std::fs::canonicalize(dst).unwrap_or_else(|_| {
            if dst.is_absolute() {
                dst.to_path_buf()
            } else {
                cwd.join(dst)
            }
        });
        if abs_dst.starts_with(root_src) {
            return Err(KamError::InvalidDirectory(format!(
                "Destination '{}' is inside the template source '{}': aborting to prevent recursive copy",
                abs_dst.display(),
                root_src.display()
            )));
        }

        for entry in WalkDir::new(src) {
            let entry = entry?;
            // Build a stable relative path for this entry and render it using Tera.
            // We render the entire relative path (not only the base name) so templates
            // can include directory components like `src/{{id}}/...`.
            let rel = entry
                .path()
                .strip_prefix(src)
                .map_err(|e| KamError::StripPrefixFailed(format!("failed to strip prefix {}: {}", src.display(), e)))?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");

            // Render the path using Tera. Guard against empty replacements or path
            // traversal which could produce incorrect target paths.
            let replaced_rel = if rel_str.is_empty() {
                String::new()
            } else {
                tera
                    .render_str(&rel_str, &context)
                    .map_err(|e| KamError::CommandFailed(format!("Tera render error for path '{}': {}", rel_str, e)))?
            };
            if replaced_rel.trim().is_empty() {
                // Skip entries that render to an empty path.
                continue;
            }

            // Do not accept `..` or other path traversal segments in the rendered relative path.
            use std::path::Component;
            let file_path_obj = Path::new(&replaced_rel);
            if file_path_obj
                .components()
                .any(|c| matches!(c, Component::ParentDir))
            {
                return Err(KamError::InvalidDirectory(format!(
                    "Invalid template path component '..' in '{}'",
                    replaced_rel
                )));
            }

            // Build the concrete destination path for this entry using the fully rendered relative path.
            // If `replaced_rel` is empty, map it to the destination root.
            let dst_path = if replaced_rel.is_empty() {
                dst.to_path_buf()
            } else {
                dst.join(&replaced_rel)
            };
            let rel_path = dst_path
                .strip_prefix(dst)
                .unwrap_or(&dst_path)
                .to_string_lossy()
                .to_string();

            // Avoid copying into the template root (inside a walk of the template) which causes loops.
            let abs_dst_path = std::fs::canonicalize(&dst_path).unwrap_or_else(|_| {
                if dst_path.is_absolute() {
                    dst_path.to_path_buf()
                } else {
                    cwd.join(&dst_path)
                }
            });
            if abs_dst_path.starts_with(root_src) {
                // Skip this entry and continue, rather than trying to copy back into the
                // template's source. This is a defensive measure to prevent accidental
                // recursion if templates contain self-referential symlinks or similar.
                println!(
                    "Warning: skipping {} -> {} (destination would be inside template source)",
                    entry.path().display(),
                    dst_path.display()
                );
                continue;
            }

            if entry.file_type().is_dir() {
                crate::utils::Utils::print_status(
                    &dst_path,
                    &rel_path,
                    crate::utils::PrintOp::Create { is_dir: true },
                    force,
                );
                // Create the directory and let WalkDir iterate its contents; do not recurse manually.
                std::fs::create_dir_all(&dst_path)?;
            } else {
                // Read as bytes and only attempt template rendering when we can safely
                // interpret the contents as UTF-8 text.
                let data = std::fs::read(entry.path())?;
                let rendered = if let Ok(text) = String::from_utf8(data.clone()) {
                    match tera.render_str(&text, &context) {
                        Ok(r) => r.into_bytes(),
                        Err(e) => {
                            eprintln!("Warning: Tera failed to render file '{}': {}; using original content", entry.path().display(), e);
                            text.into_bytes()
                        }
                    }
                } else {
                    data
                };

                crate::utils::Utils::print_status(
                    &dst_path,
                    &rel_path,
                    crate::utils::PrintOp::Create { is_dir: false },
                    force,
                );
                std::fs::write(&dst_path, &rendered)?;
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
