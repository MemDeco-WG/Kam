use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use crate::types::kam_toml::module::{ModuleType, TmplSection};
use crate::cache::KamCache;
use tempfile::TempDir;
use zip::ZipArchive;

/// Initialize a template project.
///
/// `impl_template` is an optional template selector. If provided, we will
/// search `cache/tmpl/<impl_template>.zip` first, then try embedded built-in
/// templates, then local repo (KAM_LOCAL_REPO), and finally try a direct URL
/// if `impl_template` looks like one.
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
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse template variable definitions from CLI args and template kam.toml
    let mut variables = super::template_vars::parse_template_variables(vars)?;

    // Protect core project parameters from being overridden by template variables.
    // These are provided via CLI flags or inferred (id/name/version/author) and
    // should take precedence.
    let protected_keys = ["id", "name", "version", "author"];
    for key in &protected_keys {
        variables.remove(&key.to_string());
    }

    // Build runtime values map and seed it with core project parameters first.
    let mut runtime_values: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    runtime_values.insert("id".to_string(), id.to_string());
    // name in name_map may be localized; prefer provided name arg if present
    runtime_values.insert("name".to_string(), name_map.get("en").cloned().unwrap_or_default());
    runtime_values.insert("version".to_string(), version.to_string());
    runtime_values.insert("author".to_string(), author.to_string());

    // For variables marked `required` with no default, prompt the user interactively.
    // For others, use the default when provided. If non-interactive mode is set,
    // fail on missing required variables.
    for (k, def) in &variables {
        if let Some(d) = &def.default {
            runtime_values.insert(k.clone(), d.clone());
        } else if def.required {
            if std::env::var("KAM_NONINTERACTIVE").is_ok() {
                return Err(format!("Required template variable '{}' not provided (non-interactive)", k).into());
            }
            // Prompt user for required value
            use std::io::{stdin, stdout, Write};
            let mut input = String::new();
            print!("Enter value for required template variable '{}' (type: {}): ", k, def.var_type);
            let _ = stdout().flush();
            stdin().read_line(&mut input)?;
            let val = input.trim().to_string();
            if val.is_empty() {
                return Err(format!("Required template variable '{}' not provided", k).into());
            }
            runtime_values.insert(k.clone(), val);
        }
    }

    let name_map_btree: BTreeMap<_, _> = name_map.into_iter().collect();
    let description_map_btree: BTreeMap<_, _> = description_map.into_iter().collect();

    let mut kt = crate::types::kam_toml::KamToml::new_with_current_timestamp(
        id.to_string(),
        name_map_btree,
        version.to_string(),
        author.to_string(),
        description_map_btree,
        None,
    );
    kt.kam.module_type = ModuleType::Template;
    let variables_btree: BTreeMap<_, _> = variables.clone().into_iter().collect();
    kt.kam.tmpl = Some(TmplSection { used_template: impl_template.clone(), variables: variables_btree });
    let kam_toml_rel = "kam.toml".to_string();
    super::common::print_status(&path.join("kam.toml"), &kam_toml_rel, false, force);
    kt.write_to_dir(path)?;

    // Determine which template to use
    let template_key = impl_template.as_deref().unwrap_or("tmpl");

    // Ensure cache exists and try to find template in cache/tmpl
    let cache = KamCache::new()?;
    cache.ensure_dirs()?;

    // Candidate cache zip path
    let _cached_zip = cache.tmpl_dir().join(format!("{}.zip", template_key));

    // Helper to extract a zip file into a TempDir and return the template folder path
    fn extract_zip_to_temp(zip_path: &Path, base: &str) -> Result<(TempDir, PathBuf), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let file = std::fs::File::open(zip_path)?;
        let mut archive = ZipArchive::new(file)?;
        archive.extract(temp_dir.path())?;
        let extracted_base = temp_dir.path().join(base);
        Ok((temp_dir, extracted_base))
    }
    // Refactored: determine and prepare the template zip (cache / built-in / local / url)
    fn prepare_template(template_key: &str, cache: &KamCache) -> Result<(TempDir, PathBuf), Box<dyn std::error::Error>> {
        // Normalize template_key into an asset/base name we use in cache, e.g.
        // input: "tmpl" | "template" | "tmpl_template" -> base "tmpl_template"
        let base = match template_key {
            "tmpl" | "template" | "tmpl_template" => "tmpl_template",
            "lib" | "library" | "lib_template" => "lib_template",
            "kam" | "kam_template" => "kam_template",
            other => other,
        };

        // Check cache first (use normalized base). Accept either a .zip or an
        // unpacked directory. If it's a directory, copy it into a TempDir so
        // callers always get a path inside a TempDir.
        let cached_zip = cache.tmpl_dir().join(format!("{}.zip", base));
        if cached_zip.exists() {
            return extract_zip_to_temp(&cached_zip, base);
        }
        let cached_dir = cache.tmpl_dir().join(base);
        if cached_dir.exists() && cached_dir.is_dir() {
            let temp_dir = TempDir::new()?;
            // copy cached_dir -> temp_dir/<base>
            let dst = temp_dir.path().join(base);
            std::fs::create_dir_all(&dst)?;
            fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
                for entry in std::fs::read_dir(src)? {
                    let entry = entry?;
                    let src_path = entry.path();
                    let file_name = entry.file_name();
                    let dst_path = dst.join(file_name);
                    if entry.file_type()?.is_dir() {
                        std::fs::create_dir_all(&dst_path)?;
                        copy_dir_all(&src_path, &dst_path)?;
                    } else {
                        std::fs::copy(&src_path, &dst_path)?;
                    }
                }
                Ok(())
            }
            copy_dir_all(&cached_dir, &dst)?;
            return Ok((temp_dir, dst));
        }

        // Try embedded built-in template. Try both the normalized base and
        // the original template_key to be forgiving.
        if let Ok((td, p)) = super::common::extract_builtin_template(base) {
            return Ok((td, p));
        }
        if let Ok((td, p)) = super::common::extract_builtin_template(template_key) {
            return Ok((td, p));
        }

        // Try local repo candidates (KAM_LOCAL_REPO and workspace tmpl/repo_templeta)
        if let Some(p) = std::env::var_os("KAM_LOCAL_REPO") {
            let repo_root = PathBuf::from(p);
            let candidate = repo_root.join(format!("{}.zip", base));
            if candidate.exists() {
                let _ = std::fs::create_dir_all(cache.tmpl_dir());
                let dst = cache.tmpl_dir().join(format!("{}.zip", base));
                if !dst.exists() { std::fs::copy(&candidate, &dst)?; }
                return extract_zip_to_temp(&dst, base);
            }
            // Also accept an unpacked directory in the local repo
            let candidate_dir = repo_root.join(base);
            if candidate_dir.exists() && candidate_dir.is_dir() {
                // copy into tempdir and return
                let temp_dir = TempDir::new()?;
                let dst = temp_dir.path().join(base);
                std::fs::create_dir_all(&dst)?;
                fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
                    for entry in std::fs::read_dir(src)? {
                        let entry = entry?;
                        let src_path = entry.path();
                        let file_name = entry.file_name();
                        let dst_path = dst.join(file_name);
                        if entry.file_type()?.is_dir() {
                            std::fs::create_dir_all(&dst_path)?;
                            copy_dir_all(&src_path, &dst_path)?;
                        } else {
                            std::fs::copy(&src_path, &dst_path)?;
                        }
                    }
                    Ok(())
                }
                copy_dir_all(&candidate_dir, &dst)?;
                return Ok((temp_dir, dst));
            }
        }

        if let Ok(cwd) = std::env::current_dir() {
            let candidates = vec![cwd.join("tmpl").join(format!("{}.zip", base)), cwd.join("repo_templeta").join(format!("{}.zip", base))];
            for cand in candidates {
                if cand.exists() {
                    let _ = std::fs::create_dir_all(cache.tmpl_dir());
                    let dst = cache.tmpl_dir().join(format!("{}.zip", base));
                    if !dst.exists() { std::fs::copy(&cand, &dst)?; }
                    return extract_zip_to_temp(&dst, base);
                }
            }
            // Also accept unpacked directories in the working copy (tmpl/<base> or repo_templeta/<base>)
            let dir_candidates = vec![cwd.join("tmpl").join(base), cwd.join("repo_templeta").join(base)];
            for cand in dir_candidates {
                if cand.exists() && cand.is_dir() {
                    let temp_dir = TempDir::new()?;
                    let dst = temp_dir.path().join(base);
                    std::fs::create_dir_all(&dst)?;
                    fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
                        for entry in std::fs::read_dir(src)? {
                            let entry = entry?;
                            let src_path = entry.path();
                            let file_name = entry.file_name();
                            let dst_path = dst.join(file_name);
                            if entry.file_type()?.is_dir() {
                                std::fs::create_dir_all(&dst_path)?;
                                copy_dir_all(&src_path, &dst_path)?;
                            } else {
                                std::fs::copy(&src_path, &dst_path)?;
                            }
                        }
                        Ok(())
                    }
                    copy_dir_all(&cand, &dst)?;
                    return Ok((temp_dir, dst));
                }
            }
        }

        // If template_key is a URL, try downloading
        if template_key.starts_with("http://") || template_key.starts_with("https://") {
            let resp = reqwest::blocking::get(template_key)?;
            if resp.status().is_success() {
                let bytes = resp.bytes()?;
                let tmp = tempfile::NamedTempFile::new()?;
                std::fs::write(tmp.path(), &bytes)?;
                let base = template_key.rsplit('/').next().unwrap_or("template").trim_end_matches(".zip");
                let _ = std::fs::create_dir_all(cache.tmpl_dir());
                let dst = cache.tmpl_dir().join(format!("{}.zip", base));
                if !dst.exists() { std::fs::copy(tmp.path(), &dst)?; }
                return extract_zip_to_temp(&dst, base);
            } else {
                return Err("Failed to download template".into());
            }
        }

        Err(format!("Template '{}' not found in cache, built-ins, or local repo", template_key).into())
    }

    let (_temp_dir, template_path) = prepare_template(template_key, &cache)?;

    // Copy template files recursively from `src/` (and support placeholders in
    // both file/directory names and file contents). Placeholders like
    // `{{id}}` will be replaced by the confirmed project `id` from above.
    let src_temp = template_path.join("src");
    if src_temp.exists() {
        let dst_root = path.join("src").join(id);
        super::common::print_status(&dst_root, &format!("src/{}/", id), true, force);
        std::fs::create_dir_all(&dst_root)?;

        // Recursive copy that replaces variables in path segments and file contents.
        fn copy_replace_recursive(
            src: &std::path::Path,
            dst_base: &std::path::Path,
            rel: &std::path::Path,
            runtime_values: &std::collections::HashMap<String, String>,
            force: bool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                let file_name = entry.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into());
                // Replace placeholders in file or directory name
                let mut replaced_name = file_name.clone();
                for (k, v) in runtime_values {
                    if !v.is_empty() {
                        replaced_name = replaced_name.replace(&format!("{{{{{}}}}}", k), v);
                    }
                }

                let rel_path = rel.join(&replaced_name);
                let dst_path = dst_base.join(&rel_path);

                if entry.file_type()?.is_dir() {
                    super::common::print_status(&dst_path, &rel_path.to_string_lossy(), true, force);
                    std::fs::create_dir_all(&dst_path)?;
                    copy_replace_recursive(&entry.path(), dst_base, &rel_path, runtime_values, force)?;
                } else {
                    // File: read, replace content, write
                    let content = std::fs::read_to_string(entry.path())?;
                    let mut new_content = content;
                    for (k, v) in runtime_values {
                        if !v.is_empty() {
                            new_content = new_content.replace(&format!("{{{{{}}}}}", k), v);
                        }
                    }
                    super::common::print_status(&dst_path, &rel_path.to_string_lossy(), false, force);
                    // Ensure parent dir exists
                    if let Some(p) = dst_path.parent() {
                        std::fs::create_dir_all(p)?;
                    }
                    std::fs::write(&dst_path, new_content)?;
                }
            }
            Ok(())
        }

        copy_replace_recursive(&src_temp, &path.join("src"), std::path::Path::new(id), &runtime_values, force)?;
    }

    // Special-case: if the template contains a top-level `.kam-venv` folder,
    // copy it to the project root as-is. This allows templates that represent
    // the virtual env layout (kam-venv) to be applied directly.
    let venv_temp = template_path.join(".kam-venv");
    if venv_temp.exists() {
        let dst = path.join(".kam-venv");
        super::common::print_status(&dst, &".kam-venv/".to_string(), true, force);
        std::fs::create_dir_all(&dst)?;
        // Reuse copy_replace_recursive to copy with replacements inside .kam-venv too
        // Build a small runtime map for names relative to project root: use same runtime_values
        fn copy_replace_recursive_top(
            src: &std::path::Path,
            dst: &std::path::Path,
            runtime_values: &std::collections::HashMap<String, String>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                let file_name = entry.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into());
                let mut replaced_name = file_name.clone();
                for (k, v) in runtime_values {
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

    Ok(())
}
