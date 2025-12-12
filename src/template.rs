use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use regex::Regex;
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
        // 1) Environment variable override (highest precedence).
        //    Allows temporary or test-specific overrides:
        //    KAM_TEMPLATE_CACHE_DIR=/tmp/kam_templates
        if let Ok(dir_str) = std::env::var("KAM_TEMPLATE_CACHE_DIR") {
            if !dir_str.trim().is_empty() {
                let cache_dir = if dir_str.starts_with("~/") {
                    if let Some(home) = dirs::home_dir() {
                        let rel = dir_str.trim_start_matches("~/");
                        home.join(rel)
                    } else {
                        PathBuf::from(dir_str)
                    }
                } else {
                    PathBuf::from(dir_str)
                };

                if !cache_dir.exists() {
                    fs::create_dir_all(&cache_dir).map_err(KamError::Io)?;
                }
                return Ok(cache_dir);
            }
        }

        // 2) Global configuration override in `~/.kam/config.toml`:
        //    If the config contains:
        //      [tmpl]
        //      cache_dir = "/some/path" (or "~/somepath")
        //    then use that as the template cache directory.
        if let Some(home) = dirs::home_dir() {
            let cfg_path = home.join(".kam").join("config.toml");
            if cfg_path.exists() {
                if let Ok(cfg_str) = fs::read_to_string(&cfg_path) {
                    if let Ok(cfg_v) = toml::from_str::<toml::Value>(&cfg_str) {
                        if let Some(tmpl_table) = cfg_v.get("tmpl") {
                            if let Some(cache_val) =
                                tmpl_table.get("cache_dir").and_then(|v| v.as_str())
                            {
                                let cache_dir = if cache_val.starts_with("~/") {
                                    home.join(cache_val.trim_start_matches("~/"))
                                } else {
                                    PathBuf::from(cache_val)
                                };
                                if !cache_dir.exists() {
                                    fs::create_dir_all(&cache_dir).map_err(KamError::Io)?;
                                }
                                return Ok(cache_dir);
                            }
                        }
                    }
                }
            }
        }

        // 3) Default fallback to `~/.kam/templates`
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

        // 3. Project-local templates (e.g., tmpl/ and templates/ directories in the project)
        //    Support both directory-based templates and archive files such as .tar.gz, .tgz, .zip, .tar
        let project_local_dirs = vec!["tmpl", "templates"];
        for dir in project_local_dirs {
            let dir_path = Path::new(dir);
            if dir_path.exists() && dir_path.is_dir() {
                if let Ok(entries) = fs::read_dir(dir_path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                                templates.insert(name.to_string());
                            }
                        } else if let Some(filename) = p.file_name().and_then(|s| s.to_str()) {
                            // Handle common archive extensions
                            if filename.ends_with(".tar.gz") {
                                if let Some(name) = filename.strip_suffix(".tar.gz") {
                                    templates.insert(name.to_string());
                                }
                            } else if filename.ends_with(".tgz") {
                                if let Some(name) = filename.strip_suffix(".tgz") {
                                    templates.insert(name.to_string());
                                }
                            } else if filename.ends_with(".zip") {
                                if let Some(name) = filename.strip_suffix(".zip") {
                                    templates.insert(name.to_string());
                                }
                            } else if filename.ends_with(".tar") {
                                if let Some(name) = filename.strip_suffix(".tar") {
                                    templates.insert(name.to_string());
                                }
                            }
                        }
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
        // Alias for templates using `project_name` variable
        vars.insert(
            "project_name".to_string(),
            kam_toml.prop.get_name().to_string(),
        );
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

        // mmrl.repo.* fields (helpful template variables and env vars)
        if let Some(mmrl) = &kam_toml.mmrl {
            if let Some(repo) = &mmrl.repo {
                if let Some(repository) = &repo.repository {
                    vars.insert("mmrl.repo.repository".to_string(), repository.clone());
                }
                if let Some(homepage) = &repo.homepage {
                    vars.insert("mmrl.repo.homepage".to_string(), homepage.clone());
                }
                if let Some(readme) = &repo.readme {
                    vars.insert("mmrl.repo.readme".to_string(), readme.clone());
                }
                if let Some(documentation) = &repo.documentation {
                    vars.insert("mmrl.repo.documentation".to_string(), documentation.clone());
                }
                if let Some(issues) = &repo.issues {
                    vars.insert("mmrl.repo.issues".to_string(), issues.clone());
                }
                if let Some(cover) = &repo.cover {
                    vars.insert("mmrl.repo.cover".to_string(), cover.clone());
                }
            }
        }

        // kam.build.* fields (target & hooks directories, optional)
        if let Some(build) = &kam_toml.kam.build {
            if let Some(source_dir) = &build.source_dir {
                vars.insert("kam.build.source_dir".to_string(), source_dir.clone());
            }
            if let Some(target_dir) = &build.target_dir {
                vars.insert("kam.build.target_dir".to_string(), target_dir.clone());
            }
            if let Some(output_file) = &build.output_file {
                vars.insert("kam.build.output_file".to_string(), output_file.clone());
            }
            if let Some(hooks_dir) = &build.hooks_dir {
                vars.insert("kam.build.hooks_dir".to_string(), hooks_dir.clone());
            }
        }

        // expose module_type (kam/template) for templates
        vars.insert(
            "kam.module_type".to_string(),
            format!("{:?}", kam_toml.kam.module_type).to_ascii_lowercase(),
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
    /// Backward-compatible wrapper that uses rule-based copy but with no include/exclude rules
    pub fn copy_and_replace(
        src: &Path,
        dst: &Path,
        vars: &HashMap<String, String>,
        force: bool,
        template_id: &str,
    ) -> Result<(), KamError> {
        TemplateCopier::copy_and_replace_with_rules(src, dst, vars, force, template_id, None, None)
    }

    /// Core copy function that supports `include` and `exclude` rules.
    pub fn copy_and_replace_with_rules(
        src: &Path,
        dst: &Path,
        vars: &HashMap<String, String>,
        force: bool,
        _template_id: &str,
        excludes: Option<Vec<String>>,
        includes: Option<Vec<String>>,
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

            // Evaluate include/exclude rules before touching the file
            let file_name_opt = src_path.file_name().and_then(|s| s.to_str());
            if should_skip(&dest_rel_path_str, file_name_opt, &includes, &excludes) {
                // Print a skip status for excluded files
                Utils::print_status(&dst_path, &dest_rel_path_str, PrintOp::Skip, force);
                continue;
            }

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

                // Binary files are copied as-is without templating
                if is_binary(src_path) {
                    fs::copy(src_path, &dst_path).map_err(KamError::Io)?;
                } else {
                    // Text file - perform substitution
                    let content = fs::read_to_string(src_path);
                    match content {
                        Ok(text) => {
                            // Try to render. If it fails (e.g. invalid syntax for Tera but valid for the file type),
                            // fallback to raw copy.
                            match tera.render_str(&text, &context) {
                                Ok(rendered) => {
                                    fs::write(&dst_path, rendered).map_err(KamError::Io)?;
                                }
                                Err(_e) => {
                                    // Make this verbose only if debugging, or just copy silent fallback for "invalid syntax"
                                    // Check if it is a syntax error that suggests it's not a template
                                    // For now, we tread it as: "Not a template, copy raw"
                                    // But we log it if verbose (TODO: add verbose flag), or just warning.
                                    // To reduce noise for things like GitHub workflows which use {{ }}, we can check the error.

                                    // We will just fallback to copy, maybe print a debug note if we had a logger.
                                    // For now, let's suppress the warning for common cases or make it less scary.
                                    // If force is true, maybe user expects templates.

                                    // Proceed to copy raw
                                    fs::copy(src_path, &dst_path).map_err(KamError::Io)?;
                                }
                            }
                        }
                        Err(_) => {
                            // Fallback to copy if read_to_string fails (e.g. invalid UTF-8 that wasn't caught by is_binary)
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

fn pattern_matches(pattern: &str, rel_path: &str, file_name: Option<&str>) -> bool {
    // Normalize pattern and rel_path for comparison
    let patt = pattern.trim();
    let rel = rel_path.trim();

    // Directory prefix e.g., "foo/" or "foo/bar/"
    if patt.ends_with('/') {
        let prefix = patt.trim_end_matches('/');
        if rel.starts_with(prefix) {
            return true;
        }
        // Also check with a leading "./"
        if rel.starts_with(&format!("./{}", prefix)) {
            return true;
        }
    }

    // Simple suffix wildcard '/*.ext' or '*.ext'
    if patt.starts_with("*.") {
        if let Some(fname) = file_name {
            return fname.ends_with(&patt[1..]);
        }
    }

    // If pattern contains wildcard characters, convert to regex
    if patt.contains('*') || patt.contains('?') {
        // Convert glob to a simple regex:
        let mut regex_str = regex::escape(patt);
        regex_str = regex_str.replace("\\*", ".*").replace("\\?", ".");
        let final_regex = format!("^{}$", regex_str);
        if let Ok(re) = Regex::new(&final_regex) {
            if re.is_match(rel) {
                return true;
            }
            if let Some(fname) = file_name {
                if re.is_match(fname) {
                    return true;
                }
            }
        }
    }

    // Exact match against rel path or file name
    if rel == patt {
        return true;
    }
    if let Some(fname) = file_name {
        if fname == patt {
            return true;
        }
    }

    false
}

fn should_skip(
    rel_path: &str,
    file_name: Option<&str>,
    includes: &Option<Vec<String>>,
    excludes: &Option<Vec<String>>,
) -> bool {
    // If includes exist and any of them matches, do NOT skip (force include).
    if let Some(includes_vec) = includes {
        for inc in includes_vec {
            if pattern_matches(inc, rel_path, file_name) {
                return false;
            }
        }
    }

    // If excludes exist and any matches, we skip unless included above.
    if let Some(excludes_vec) = excludes {
        for exc in excludes_vec {
            if pattern_matches(exc, rel_path, file_name) {
                return true;
            }
        }
    }
    false
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
    use serial_test::serial;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    #[serial]
    fn test_get_cache_dir_uses_env_var() {
        let cache_tmp = tempdir().expect("cache tmpdir");
        let cache_path = cache_tmp.path().join("templates");
        // Keep old value and restore after test
        let old_cache = std::env::var_os("KAM_TEMPLATE_CACHE_DIR");
        unsafe {
            std::env::set_var("KAM_TEMPLATE_CACHE_DIR", cache_path.to_str().unwrap());
        }

        let dir = TemplateCacheManager::get_cache_dir().expect("get cache dir");
        assert_eq!(dir, cache_path);

        // Restore old value
        if let Some(orig) = old_cache {
            unsafe {
                std::env::set_var("KAM_TEMPLATE_CACHE_DIR", orig);
            }
        } else {
            unsafe {
                std::env::remove_var("KAM_TEMPLATE_CACHE_DIR");
            }
        }
    }

    #[test]
    #[serial]
    fn test_get_cache_dir_tilde_expansion_works() {
        // Arrange: set up isolated HOME and KAM_TEMPLATE_CACHE_DIR values and capture old envs to restore later.
        let home_tmp = tempdir().expect("home tmpdir");
        let old_home = std::env::var_os("HOME");
        let old_cache = std::env::var_os("KAM_TEMPLATE_CACHE_DIR");

        unsafe {
            // Set HOME to a temp path so tilde expansion doesn't touch the real user's home.
            std::env::set_var("HOME", home_tmp.path().to_str().unwrap());
            // Use a tilde-prefixed path so that get_cache_dir expands it
            std::env::set_var("KAM_TEMPLATE_CACHE_DIR", "~/kam_test_cache_dir");
        }

        // Expected expansion should join $HOME (the temporary HOME) with the provided last component
        let expected = home_tmp.path().join("kam_test_cache_dir");

        let dir = TemplateCacheManager::get_cache_dir().expect("get cache dir");
        assert_eq!(dir, expected);

        // Cleanup created cache dir if present in that temp HOME
        if expected.exists() {
            std::fs::remove_dir_all(&expected).expect("cleanup cache dir");
        }

        // Restore env vars
        if let Some(orig) = old_home {
            unsafe {
                std::env::set_var("HOME", orig);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }

        if let Some(orig) = old_cache {
            unsafe {
                std::env::set_var("KAM_TEMPLATE_CACHE_DIR", orig);
            }
        } else {
            unsafe {
                std::env::remove_var("KAM_TEMPLATE_CACHE_DIR");
            }
        }
    }

    #[test]
    #[serial]
    fn test_get_cache_dir_respects_config_tmpl_cache_dir() {
        // Save old values and restore at the end
        let old_cache = std::env::var_os("KAM_TEMPLATE_CACHE_DIR");
        let old_home = std::env::var_os("HOME");

        // Create a temporary HOME directory to be used for tilde expansion and isolation
        let home_tmp = tempdir().expect("home tmpdir");

        unsafe {
            // Ensure we don't accidentally use the env var override
            std::env::remove_var("KAM_TEMPLATE_CACHE_DIR");
            // Point HOME specifically to our tempdir so expansion is predictable
            std::env::set_var("HOME", home_tmp.path().to_str().expect("home tmp to str"));
        }

        // Create a minimal global config: ~/.kam/config.toml
        let cfg_dir = home_tmp.path().join(".kam");
        std::fs::create_dir_all(&cfg_dir).expect("create config dir");
        let cfg_file = cfg_dir.join("config.toml");
        let cfg_content = r#"
[tmpl]
cache_dir = "~/my_config_cache_dir"
"#;
        std::fs::write(&cfg_file, cfg_content).expect("write config");

        // Expected expansion should join $HOME with the provided relative component
        let expected = home_tmp.path().join("my_config_cache_dir");

        let dir = TemplateCacheManager::get_cache_dir().expect("get cache dir");
        assert_eq!(dir, expected);

        // Cleanup: remove any created cache dir
        if expected.exists() {
            std::fs::remove_dir_all(&expected).expect("cleanup cache dir");
        }

        // Restore env vars
        if let Some(orig) = old_cache {
            unsafe {
                std::env::set_var("KAM_TEMPLATE_CACHE_DIR", orig);
            }
        } else {
            unsafe {
                std::env::remove_var("KAM_TEMPLATE_CACHE_DIR");
            }
        }
        if let Some(orig) = old_home {
            unsafe {
                std::env::set_var("HOME", orig);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    fn test_copy_and_replace_copies_binary() {
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

        // Binary should have been copied
        assert!(dst_dir.path().join("image.png").exists());

        // Binary contents must match the source
        let original_bytes = std::fs::read(&binary_file).unwrap();
        let copied_bytes = std::fs::read(dst_dir.path().join("image.png")).unwrap();
        assert_eq!(original_bytes, copied_bytes);

        // Text should be present and rendered
        let rendered = std::fs::read_to_string(dst_dir.path().join("README.md")).unwrap();
        assert!(rendered.contains("Hello World"));
    }

    #[test]
    fn test_flatten_kam_toml_mmrl_and_kam_build() {
        // Construct a basic KamToml and set mmrl.repo and kam.build fields for testing
        let mut kt = crate::types::kam_toml::KamToml::new_with_current_timestamp(
            "com.example.test".to_string(),
            "Example Test".to_string(),
            "0.1.0".to_string(),
            "Author".to_string(),
            "Desc".to_string(),
            None,
            None,
        );

        // Set mmrl.repo.repository
        if let Some(mmrl) = kt.mmrl.as_mut() {
            if let Some(repo) = mmrl.repo.as_mut() {
                repo.repository = Some("https://github.com/test/repo".to_string());
            }
        }

        // Ensure build section exists and set source_dir
        if kt.kam.build.is_none() {
            kt.kam.build = Some(crate::types::kam_toml::sections::BuildSection::default());
        }
        kt.kam.build.as_mut().unwrap().source_dir = Some("src/test".to_string());

        // Flatten and assert the expected values were included
        let vars = TemplateVariableProcessor::flatten_kam_toml(&kt);
        assert_eq!(
            vars.get("mmrl.repo.repository").map(|s| s.as_str()),
            Some("https://github.com/test/repo")
        );
        assert_eq!(
            vars.get("kam.build.source_dir").map(|s| s.as_str()),
            Some("src/test")
        );
        assert_eq!(
            vars.get("project_name").map(|s| s.as_str()),
            Some("Example Test")
        );
    }

    #[test]
    fn test_copy_and_replace_respects_exclude_include_rules() {
        use std::fs::File;
        use std::io::Write;
        use tempfile::tempdir;

        let src_dir = tempdir().expect("src tempdir");
        let dst_dir = tempdir().expect("dst tempdir");

        // Create some test files in src_dir
        // keep.txt should be copied
        let keep_file = src_dir.path().join("keep.txt");
        let mut keep_f = File::create(&keep_file).expect("create keep file");
        keep_f.write_all(b"Keep me").expect("write keep file");

        // skip.log should be excluded via pattern
        let skip_file = src_dir.path().join("skip.log");
        let mut skip_f = File::create(&skip_file).expect("create skip file");
        skip_f.write_all(b"Skip this").expect("write skip file");

        // Binary asset - will be excluded via directory pattern "assets/"
        let assets_dir = src_dir.path().join("assets");
        std::fs::create_dir_all(&assets_dir).expect("create assets dir");
        let img_file = assets_dir.join("image.png");
        let mut img_f = File::create(&img_file).expect("create image");
        img_f.write_all(&[0u8, 0u8, 0u8]).expect("write image");

        // Include dir - should be included through include rules
        let include_dir = src_dir.path().join("include");
        std::fs::create_dir_all(&include_dir).expect("create include dir");
        let important_file = include_dir.join("important.txt");
        let mut imp_f = File::create(&important_file).expect("create important");
        imp_f.write_all(b"Important").expect("write important");

        // Prepare template variables
        let mut vars = std::collections::HashMap::new();
        vars.insert("name".to_string(), "World".to_string());

        // Setup exclude and include rules:
        let excludes = Some(vec!["*.log".to_string(), "assets/".to_string()]);
        let includes = Some(vec!["include/".to_string()]);

        // Perform copy using the "with_rules" variant so we can test include/exclude behavior
        crate::template::TemplateManager::copy_and_replace_with_rules(
            src_dir.path(),
            dst_dir.path(),
            &vars,
            true,
            "test_template",
            excludes.clone(),
            includes.clone(),
        )
        .expect("copy_and_replace failed");

        // Assertions:
        // keep.txt should exist
        assert!(dst_dir.path().join("keep.txt").exists());

        // skip.log should be excluded
        assert!(!dst_dir.path().join("skip.log").exists());

        // assets/image.png should be excluded by pattern
        assert!(!dst_dir.path().join("assets").join("image.png").exists());

        // include/important.txt should be present
        assert!(
            dst_dir
                .path()
                .join("include")
                .join("important.txt")
                .exists()
        );
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

    pub fn copy_and_replace_with_rules(
        src: &Path,
        dst: &Path,
        vars: &HashMap<String, String>,
        force: bool,
        template_id: &str,
        excludes: Option<Vec<String>>,
        includes: Option<Vec<String>>,
    ) -> Result<(), KamError> {
        TemplateCopier::copy_and_replace_with_rules(
            src,
            dst,
            vars,
            force,
            template_id,
            excludes,
            includes,
        )
    }
}
