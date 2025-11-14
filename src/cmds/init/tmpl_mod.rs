use crate::cmds::init::status::{StatusType, print_status};
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;
use crate::types::kam_toml::sections::TmplSection;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::cache::KamCache;
use crate::errors::KamError;
use flate2;
use tar;
use tempfile::TempDir;
use walkdir;
use zip;

// Helper to extract a zip or tar.gz file into a TempDir and return the template folder path
pub fn extract_archive_to_temp(archive_path: &Path) -> Result<(TempDir, PathBuf), KamError> {
    let temp_dir = TempDir::new()?;
    let file = std::fs::File::open(archive_path)?;
    if archive_path.extension().and_then(|s| s.to_str()) == Some("zip") {
        let mut archive = zip::ZipArchive::new(file)?;
        archive.extract(temp_dir.path())?;
    } else {
        let gz_decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz_decoder);
        archive.unpack(temp_dir.path())?;
    }
    let template_path = temp_dir.path().to_path_buf();
    Ok((temp_dir, template_path))
}

/// Initialize a template project.
///
/// `impl_template` is an optional template selector. If provided, we will
/// search `cache/tmpl/<impl_template>.zip` first, then try embedded built-in
/// templates, then local repo (KAM_LOCAL_REPO), and finally try a direct URL
/// `impl_template` looks like one.
pub fn init_template(
    path: &Path,
    id: &str,
    name_map: HashMap<String, String>,
    version: &str,
    author: &str,
    description_map: HashMap<String, String>,
    vars: &[String],
    impl_template: Option<String>,
    force: bool,
    module_type: ModuleType,
    update_json: Option<String>,
) -> Result<(), KamError> {
    // Parse template variable definitions from CLI args and template kam.toml
    let mut variables = crate::template::TemplateManager::parse_template_variables(vars)?;

    // Protect core project parameters from being overridden by template variables.
    // These are provided via CLI flags or inferred (id/name/version/author) and
    // should take precedence.
    let protected_keys = ["id", "name", "version", "author"];
    for key in &protected_keys {
        variables.remove(&key.to_string());
    }

    // Build runtime values map and seed it with core project parameters first.
    let mut runtime_values: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    runtime_values.insert("id".to_string(), id.to_string());
    // name in name_map may be localized; prefer provided name arg if present
    runtime_values.insert(
        "name".to_string(),
        name_map.get("en").cloned().unwrap_or_default(),
    );
    runtime_values.insert("version".to_string(), version.to_string());
    runtime_values.insert("author".to_string(), author.to_string());

    // For variables marked `required` with no default, prompt the user interactively.
    // For others, use the default when provided. If non-interactive mode is set,
    // fail on missing required variables.
    for (k, def) in &variables {
        if let Some(d) = &def.default {
            runtime_values.insert(k.to_string(), d.clone());
        } else if def.required {
            // If non-interactive, surface an error that includes the template-provided
            // note when available to guide the user how to supply the missing value.
            if std::env::var("KAM_NONINTERACTIVE").is_ok() {
                if let Some(n) = &def.note {
                    return Err(KamError::TemplateVarRequired(format!(
                        "Required template variable '{}' not provided (non-interactive): {}",
                        k, n
                    )));
                }
                return Err(KamError::TemplateVarRequired(format!(
                    "Required template variable '{}' not provided (non-interactive)",
                    k
                )));
            }

            // Prompt user for required value. If the template provides a human-friendly
            // note, show it as the prompt; otherwise fall back to a generic prompt.
            use std::io::{Write, stdin, stdout};
            let mut input = String::new();
            if let Some(n) = &def.note {
                print!("{} ", n);
            } else {
                print!(
                    "Enter value for required template variable '{}' (type: {}): ",
                    k, def.var_type
                );
            }
            let _ = stdout().flush();
            stdin().read_line(&mut input)?;
            let val = input.trim().to_string();
            if val.is_empty() {
                if let Some(n) = &def.note {
                    return Err(KamError::TemplateVarRequired(format!(
                        "Required template variable '{}' not provided: {}",
                        k, n
                    )));
                }
                return Err(KamError::TemplateVarRequired(format!(
                    "Required template variable '{}' not provided",
                    k
                )));
            }
            runtime_values.insert(k.to_string(), val);
        }
    }

    let name_map_btree: BTreeMap<_, _> = name_map.into_iter().collect();
    let description_map_btree: BTreeMap<_, _> = description_map.into_iter().collect();

    let mut kt = KamToml::new_with_current_timestamp(
        id.to_string(),
        name_map_btree,
        version.to_string(),
        author.to_string(),
        description_map_btree,
        update_json,
        Some(ModuleType::Template),
    );
    kt.kam.module_type = module_type;
    let variables_btree: BTreeMap<_, _> = variables.into_iter().collect();
    kt.kam.tmpl = Some(TmplSection {
        used_template: impl_template.clone(),
        variables: variables_btree,
    });
    // Ensure the target directory exists before writing kam.toml
    std::fs::create_dir_all(path)?;

    let kam_toml_rel = "kam.toml".to_string();
    print_status(StatusType::Add, &kam_toml_rel, false);
    kt.write_to_dir(path)?;

    // Determine which template to use
    let template_key = impl_template.as_deref().unwrap_or("tmpl");

    // Ensure cache exists and try to find template in cache/tmpl
    // Refactored: determine and prepare the template zip (built-in / url only)
    fn prepare_template(template_key: &str) -> Result<(TempDir, PathBuf), KamError> {
        // Normalize template_key into an asset/base name we use, e.g.
        // input: "tmpl" | "template" | "tmpl_template" -> base "tmpl_template"
        let normalized_key = match template_key {
            "tmpl" | "template" => "tmpl_template",
            _ => template_key,
        };

        // If template_key is a URL, try downloading
        if template_key.starts_with("http://") || template_key.starts_with("https://") {
            let resp = reqwest::blocking::get(template_key)?;
            if resp.status().is_success() {
                let bytes = resp.bytes()?;
                let tmp = tempfile::NamedTempFile::new()?;
                std::fs::write(tmp.path(), &bytes)?;
                let (temp_dir, template_path) = extract_archive_to_temp(tmp.path())?;
                // Optionally save to cache, but for now just return
                return Ok((temp_dir, template_path));
            } else {
                return Err(KamError::FetchFailed(
                    "Failed to download template".to_string(),
                ));
            }
        }

        // Ensure the template is available in cache (only for built-ins)
        crate::template::TemplateManager::ensure_template(normalized_key)?;

        // Extract from cache
        let cache = KamCache::new()?;
        let cache_path = cache.tmpl_dir().join(format!("{}.tar.gz", normalized_key));
        let (temp_dir, template_path) = extract_archive_to_temp(&cache_path)?;

        Ok((temp_dir, template_path))
    }

    let (_temp_dir, template_path) = prepare_template(template_key)?;

    // Copy template files recursively from `src/` (and support placeholders in
    // both file/directory names and file contents). Placeholders like
    // `{{id}}` will be replaced by the confirmed project `id` from above.
    let src_temp = template_path.join("src");
    if src_temp.exists() {
        let dst_root = path.join("src");
        print_status(StatusType::Add, "src/", true);
        std::fs::create_dir_all(&dst_root)?;

        // Recursive copy that replaces variables in path segments and file contents.
        fn copy_replace_recursive(
            src: &std::path::Path,
            dst_base: &std::path::Path,
            rel: &std::path::Path,
            runtime_values: &std::collections::HashMap<String, String>,
            force: bool,
        ) -> Result<(), KamError> {
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                let file_name = entry
                    .file_name()
                    .into_string()
                    .unwrap_or_else(|s| s.to_string_lossy().into());
                // Replace placeholders in file or directory name
                let mut replaced_name = file_name;
                for (k, v) in runtime_values.iter() {
                    if !v.is_empty() {
                        replaced_name = replaced_name.replace(&format!("{{{{{}}}}}", k), v);
                    }
                }

                let rel_path = rel.join(&replaced_name);
                let dst_path = dst_base.join(&rel_path);

                if entry.file_type()?.is_dir() {
                    print_status(StatusType::Add, &rel_path.to_string_lossy(), true);
                    std::fs::create_dir_all(&dst_path)?;
                    copy_replace_recursive(
                        &entry.path(),
                        dst_base,
                        &rel_path,
                        runtime_values,
                        force,
                    )?;
                } else {
                    // File: read, replace content, write
                    // Try to read as text first for variable replacement
                    match std::fs::read_to_string(entry.path()) {
                        Ok(content) => {
                            let mut new_content = content;
                            for (k, v) in runtime_values {
                                if !v.is_empty() {
                                    new_content = new_content.replace(&format!("{{{{{}}}}}", k), v);
                                }
                            }
                            print_status(StatusType::Add, &rel_path.to_string_lossy(), false);
                            // Ensure parent dir exists
                            if let Some(p) = dst_path.parent() {
                                std::fs::create_dir_all(p)?;
                            }
                            std::fs::write(&dst_path, new_content)?;
                        }
                        Err(_) => {
                            // Binary file - copy as-is
                            let content = std::fs::read(entry.path())?;
                            print_status(StatusType::Add, &rel_path.to_string_lossy(), false);
                            if let Some(p) = dst_path.parent() {
                                std::fs::create_dir_all(p)?;
                            }
                            std::fs::write(&dst_path, content)?;
                        }
                    }
                }
            }
            Ok(())
        }

        copy_replace_recursive(
            &src_temp,
            &path.join("src"),
            std::path::Path::new(""),
            &runtime_values,
            force,
        )?;
    }

    // Special-case: if the template contains a top-level `.kam_venv` folder,
    // copy it to the project root as-is. This allows templates that represent
    // the virtual env layout (kam_venv) to be applied directly.
    let venv_temp = template_path.join(".kam_venv");
    if venv_temp.exists() {
        let dst = path.join(".kam_venv");
        print_status(StatusType::Add, &".kam_venv/".to_string(), true);
        std::fs::create_dir_all(&dst)?;
        // Reuse copy_replace_recursive to copy with replacements inside .kam-venv too
        // Build a small runtime map for names relative to project root: use same runtime_values
        fn copy_replace_recursive_top(
            src: &std::path::Path,
            dst: &std::path::Path,
            runtime_values: &std::collections::HashMap<String, String>,
        ) -> Result<(), KamError> {
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                let file_name = entry
                    .file_name()
                    .into_string()
                    .unwrap_or_else(|s| s.to_string_lossy().into());
                let mut replaced_name = file_name;
                for (k, v) in runtime_values.iter() {
                    if !v.is_empty() {
                        replaced_name = replaced_name.replace(&format!("{{{{{}}}}}", k), v);
                    }
                }
                let dst_path = dst.join(&replaced_name);
                if entry.file_type()?.is_dir() {
                    std::fs::create_dir_all(&dst_path)?;
                    copy_replace_recursive_top(&entry.path(), &dst_path, runtime_values)?;
                } else {
                    let content = std::fs::read(&entry.path())?;
                    std::fs::write(&dst_path, &content)?;
                }
            }
            Ok(())
        }

        copy_replace_recursive_top(&venv_temp, &dst, &runtime_values)?;
    }

    // Copy ALL template files except kam.toml (which is generated separately)
    // Handle both text and binary files properly
    for entry in walkdir::WalkDir::new(&template_path) {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(&template_path)?;

        // Skip kam.toml and src directory (already processed above)
        if rel_path == Path::new("kam.toml")
            || rel_path == Path::new("src")
            || rel_path.starts_with("src/")
        {
            continue;
        }

        // Skip .kam_venv directory (already processed above)
        if rel_path == Path::new(".kam_venv") || rel_path.starts_with(".kam_venv/") {
            continue;
        }

        let mut rel_str = rel_path.to_string_lossy().to_string();
        for (k, v) in &runtime_values {
            if !v.is_empty() {
                rel_str = rel_str.replace(&format!("{{{{{}}}}}", k), &v);
            }
        }

        let dst_path = path.join(&rel_str);

        if entry.file_type().is_dir() {
            print_status(StatusType::Add, &rel_str, true);
            std::fs::create_dir_all(&dst_path)?;
        } else {
            // Try to read as text first for variable replacement
            match std::fs::read_to_string(entry.path()) {
                Ok(content) => {
                    // Text file - apply variable replacements
                    let mut new_content = content;
                    for (k, v) in &runtime_values {
                        if !v.is_empty() {
                            new_content = new_content.replace(&format!("{{{{{}}}}}", k), &v);
                        }
                    }
                    print_status(StatusType::Add, &rel_str, false);
                    if let Some(p) = dst_path.parent() {
                        std::fs::create_dir_all(p)?;
                    }
                    std::fs::write(&dst_path, new_content)?;
                }
                Err(_) => {
                    // Binary file - copy as-is
                    let content = std::fs::read(entry.path())?;
                    print_status(StatusType::Add, &rel_str, false);
                    if let Some(p) = dst_path.parent() {
                        std::fs::create_dir_all(p)?;
                    }
                    std::fs::write(&dst_path, content)?;
                }
            }
        }
    }

    Ok(())
}
