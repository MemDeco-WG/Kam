use crate::cache::KamCache;
use crate::cmds::init::status::{StatusType, print_status};
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;
use crate::types::kam_toml::sections::TmplSection;
use serde_json;
use crate::types::source::Source;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use tera::{Context, Tera};

fn merge_template_defaults(
    kt_path: &Path,
    template_vars: &mut HashMap<String, String>,
) -> Result<(), KamError> {
    let kt_template = KamToml::load_from_file(kt_path)?;
    if let Some(tmpl) = &kt_template.kam.tmpl {
        for (var_name, var_def) in &tmpl.variables {
            if template_vars.contains_key(var_name.as_str()) {
                continue;
            }

            if var_def.required {
                if let Some(default) = &var_def.default {
                    template_vars.insert(var_name.to_string(), default.clone());
                    continue;
                }
                if let Some(note) = &var_def.note {
                    return Err(KamError::TemplateVarRequired(format!(
                        "Required template variable '{}' not provided: {}",
                        var_name, note
                    )));
                }
                return Err(KamError::TemplateVarRequired(format!(
                    "Required template variable '{}' not provided",
                    var_name
                )));
            }

            if let Some(default) = &var_def.default {
                template_vars.insert(var_name.to_string(), default.clone());
            }
        }
    }
    Ok(())
}

/// Initialize a project from a template source.
///
/// extra parameter `explicit_template_id` is preferred (when available) to set
/// `[kam.tmpl].used_template` rather than deriving an id from a temporary path.
pub fn init_impl(
    path: &Path,
    id: &str,
    name_map: HashMap<String, String>,
    version: &str,
    author: &str,
    description_map: HashMap<String, String>,
    impl_source: &str,
    template_vars: &mut HashMap<String, String>,
    force: bool,
    explicit_template_id: Option<&str>,
) -> Result<(), KamError> {
    // Parse the template source specification
    let source = Source::parse(impl_source).map_err(|e| {
        KamError::FetchFailed(format!(
            "Failed to parse template source '{}': {}",
            impl_source, e
        ))
    })?;
    // Debug: show initial parameters for this invocation
    println!("DEBUG init_impl: init called with path: {}", path.display());
    println!("DEBUG init_impl: impl_source: {}", impl_source);
    println!("DEBUG init_impl: force: {}", force);

    // Create a dummy KamToml for the module (we'll load the real one from the template)
    let dummy_toml = KamToml::new_with_current_timestamp(
        "template".to_string(),
        [("en".to_string(), "Template".to_string())].into(),
        "1.0.0".to_string(),
        "Template Author".to_string(),
        [("en".to_string(), "Template description".to_string())].into(),
        None,
        None,
    );

    // Create KamModule and fetch the template (unpacked into a tempdir)
    // Clone `source` so we can continue to use it later without moving
    let module = crate::types::modules::base::KamModule::new(dummy_toml, Some(source.clone()));
    let mut template_path = module.fetch_to_temp()?;

    // If the template path is a directory that contains exactly one child directory,
    // treat the child directory as the real template root. This normalizes behavior
    // for archives that unpack into a single top-level folder (common tar/zip layout).
    if template_path.exists() && template_path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&template_path) {
            let child_dirs: Vec<std::path::PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            if child_dirs.len() == 1 {
                template_path = child_dirs.into_iter().next().unwrap();
            }
        }
    }

    // Determine archive_id from the source (prefer a human friendly/sanitized name).
    // If an explicit template id is provided we prefer it over deriving from `source`
    // (this avoids using a tempdir name).
    let archive_id = {
        // Sanitize an arbitrary string into a compact archive id
        fn sanitize_name(s: &str) -> String {
            let mut out = String::new();
            for ch in s.chars() {
                if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
                    out.push(ch.to_ascii_lowercase());
                } else {
                    out.push('-');
                }
            }
            // Trim leading/trailing separators inserted during sanitization
            let trimmed = out.trim_matches(|c: char| c == '-' || c == '.').to_string();
            if trimmed.is_empty() {
                "template".to_string()
            } else {
                trimmed
            }
        }

        // Prefer explicit id when present to avoid `.tmp*` fallback
        if let Some(explicit_id) = explicit_template_id {
            sanitize_name(explicit_id)
        } else {
            // Candidate extraction logic:
            // - Local: last path component
            // - Url: last path component (strip query/fragment), drop extension if present
            // - Git: last path component (strip `.git`)
            let candidate = match &source {
                Source::Local { path } => path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string()),
                Source::Url { url } => {
                    let s = url.as_str();
                    // Remove query and fragment; take last path segment; remove extension
                    let segment = s.split('?').next().unwrap_or(s).split('#').next().unwrap_or(s);
                    let last = segment.rsplit('/').next().unwrap_or(segment);
                    last.split('.').next().unwrap_or(last).to_string()
                }
                Source::Git { url, .. } => {
                    let s = url.as_str();
                    let s = if s.ends_with(".git") {
                        &s[..s.len() - 4]
                    } else {
                        s
                    };
                    s.rsplit('/').next().unwrap_or(s).to_string()
                }
            };

            sanitize_name(&candidate)
        }
    };

    // Load template variables and insert defaults (refactored to helper to avoid deep nesting)
    let template_kam_path = template_path.join("kam.toml");
    if template_kam_path.exists() {
        // helper hoisted to top-level: see `merge_template_defaults` above

        merge_template_defaults(&template_kam_path, template_vars)?;
    }

    let name_map_btree: BTreeMap<_, _> = name_map.into_iter().collect();
    let description_map_btree: BTreeMap<_, _> = description_map.into_iter().collect();

    // Prepare KamToml and set the used template
    let kam_toml_rel = "kam.toml".to_string();
    print_status(StatusType::Add, &kam_toml_rel, false);

    let mut kt = KamToml::new_with_current_timestamp(
        id.to_string(),
        name_map_btree,
        version.to_string(),
        author.to_string(),
        description_map_btree,
        None,
        None,
    );
    kt.kam.tmpl = Some(TmplSection {
        used_template: Some(archive_id.clone()),
        variables: BTreeMap::new(),
    });

    // Extract and separate '#'-prefixed variables that target kam.toml fields
    let mut kam_vars: Vec<(String, String)> = Vec::new();
    for k in template_vars.keys() {
        if k.starts_with('#') {
            kam_vars.push((k.to_string(), template_vars.get(k).unwrap().clone()));
        }
    }
    // Remove kam vars from template_vars so they are not applied to file contents
    for k in &kam_vars {
        template_vars.remove(&k.0);
    }

    if !kam_vars.is_empty() {
        kt.apply_vars(kam_vars)?;
    }

    // Write initial kam.toml (includes #var effects)
    println!(
        "DEBUG init_impl: about to write initial kam.toml to {}",
        path.display()
    );
    println!(
        "DEBUG init_impl: destination path exists: {}",
        path.exists()
    );
    // Ensure the destination directory exists so writing kam.toml succeeds.
    // If it does not exist, create all parent directories.
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    if path.exists() {
        let has_entries = std::fs::read_dir(path)
            .map(|mut r| r.next().is_some())
            .unwrap_or(false);
        println!(
            "DEBUG init_impl: destination path already contains entries: {}",
            has_entries
        );
    }
    kt.write_to_dir(path)?;

    // Merge flattened variables from the freshly created `kt` into `template_vars`
    // so that the templating context (used for kam.toml rendering as well as file
    // copying) includes values derived from the final KamToml structure such as
    // `id`, `version`, `name` and other property paths.
    //
    // Important note: we prefer explicit CLI-provided template variables already in
    // `template_vars` and therefore only insert a key when it is not present,
    // preserving CLI overrides.
    let kt_vars = crate::template::TemplateVariableProcessor::flatten_kam_toml(&kt);
    for (k, v) in kt_vars.into_iter() {
        template_vars.entry(k).or_insert(v);
    }

    // Ensure shallow convenience keys exist for templates that expect them:
    // id, name, version, versionCode, author, description.
    //
    // These are populated from the final KamToml structure (`kt`) and are only
    // inserted when the key isn't already present: we do not override CLI or
    // template-provided explicit values.
    if !template_vars.contains_key("id") {
        template_vars.insert("id".to_string(), kt.prop.id.clone());
    }
    if !template_vars.contains_key("version") {
        template_vars.insert("version".to_string(), kt.prop.version.clone());
    }
    if !template_vars.contains_key("versionCode") {
        template_vars.insert("versionCode".to_string(), kt.prop.versionCode.to_string());
    }
    if !template_vars.contains_key("author") {
        template_vars.insert("author".to_string(), kt.prop.author.clone());
    }

    // For `name` and `description`, prefer the 'en' locale if present. Fall back
    // to the first map entry otherwise (or id/empty string as a final fallback).
    let shallow_name = kt
        .prop
        .name
        .get("en")
        .cloned()
        .or_else(|| kt.prop.name.iter().next().map(|(_k, v)| v.clone()))
        .unwrap_or_else(|| kt.prop.id.clone());
    if !template_vars.contains_key("name") {
        template_vars.insert("name".to_string(), shallow_name.clone());
    }

    let shallow_description = kt
        .prop
        .description
        .get("en")
        .cloned()
        .or_else(|| kt.prop.description.iter().next().map(|(_k, v)| v.clone()))
        .unwrap_or_default();
    if !template_vars.contains_key("description") {
        template_vars.insert("description".to_string(), shallow_description.clone());
    }

    eprintln!(
        "DEBUG init_impl: merged kt flattened variables into template_vars, keys now: {:?}",
        template_vars.keys().collect::<Vec<_>>()
    );

    // Apply non-# template variables into kam.toml (allows placeholders like {{project_name}} in kam.toml)
    let kam_toml_path = path.join("kam.toml");
    if kam_toml_path.exists() {
        let mut content = std::fs::read_to_string(&kam_toml_path)?;
        let mut context = Context::new();
        for (k, v) in template_vars.iter() {
            context.insert(k, v);
        }
        let mut tera = Tera::default();
        // Debugging: print path, content length, preview and template variables prior to rendering
        let preview: String = content.chars().take(1024).collect();
        let vars_json = serde_json::to_string(&template_vars)
            .unwrap_or_else(|_| "<vars-json-error>".to_string());
        eprintln!(
            "DEBUG init_impl: rendering kam.toml: {}",
            kam_toml_path.display()
        );
        eprintln!("DEBUG init_impl: content length: {}", content.len());
        eprintln!("DEBUG init_impl: content preview: {}", preview);
        eprintln!("DEBUG init_impl: template_vars: {}", vars_json);
        content = tera.render_str(&content, &context).map_err(|e| {
            // Also print debug info upon template render error for easier diagnosis
            eprintln!("DEBUG init_impl: tera render error: {}", e);
            eprintln!("DEBUG init_impl: failing content (preview): {}", preview);
            eprintln!("DEBUG init_impl: template_vars (on error): {}", vars_json);
            KamError::TemplateRenderError(e.to_string())
        })?;
        std::fs::write(&kam_toml_path, content)?;
    }

    // Copy the template's files into the project root (this includes README, LICENSE, .kam_venv, src/<id>, etc.)
    println!(
        "DEBUG init_impl: about to copy files from {} to {}",
        template_path.display(),
        path.display()
    );
    println!(
        "DEBUG init_impl: template_path exists: {}",
        template_path.exists()
    );
    println!(
        "DEBUG init_impl: archive_id: {}, force: {}",
        archive_id, force
    );
    crate::template::TemplateManager::copy_and_replace(
        &template_path,
        path,
        template_vars,
        force,
        &archive_id,
    )?;
    // Re-ensure kam.toml is the canonical one with applied '#'-vars (overwrite if template carried a kam.toml)
    println!("DEBUG init_impl: re-writing kam.toml to {}", path.display());
    println!(
        "DEBUG init_impl: destination path exists (re-check): {}",
        path.exists()
    );
    kt.write_to_dir(path)?;

    Ok(())
}

pub fn init_template(
    path: &Path,
    id: &str,
    name_map: BTreeMap<String, String>,
    version: &str,
    author: &str,
    description_map: BTreeMap<String, String>,
    var: &[String],
    impl_template: Option<String>,
    force: bool,
    _module_type: ModuleType,
    _update_json: Option<String>,
) -> Result<(), KamError> {
    // Parse command-line template variables into a HashMap
    let mut template_vars = crate::template::TemplateManager::parse_template_vars(var)?;

    // Determine the candidate template spec (builtin name or a user-provided source)
    let template_spec = impl_template
        .as_deref()
        .unwrap_or("kam_template")
        .to_string();

    // Detect if the chosen template is a builtin template ID
    const BUILTIN_TEMPLATES: &[&str] = &[
        "kam_template",
        "lib_template",
        "tmpl_template",
        "repo_template",
        "venv_template",
    ];
    let is_builtin = BUILTIN_TEMPLATES.contains(&template_spec.as_str());

    if is_builtin {
        // Ensure the built-in template archive exists in the cache
        let cache = KamCache::new()?;
        crate::template::TemplateManager::ensure_template(&template_spec)?;
        let tmpl_dir = cache.tmpl_dir();

        // Candidate paths for templates (gz/tgz/tar, zip, or directory)
        let tar_gz_path = tmpl_dir.join(format!("{}.tar.gz", template_spec));
        let zip_path = tmpl_dir.join(format!("{}.zip", template_spec));
        let dir_path = tmpl_dir.join(&template_spec);

        // Choose the first existing candidate
        let chosen: Option<PathBuf> = if tar_gz_path.exists() {
            Some(tar_gz_path)
        } else if zip_path.exists() {
            Some(zip_path)
        } else if dir_path.exists() {
            Some(dir_path)
        } else {
            None
        };

        if let Some(chosen_path) = chosen {
            let src_spec = chosen_path.to_string_lossy().to_string();
            // Convert BTreeMap -> HashMap to satisfy init_impl signature.
            let name_hash: HashMap<String, String> = name_map
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let description_hash: HashMap<String, String> = description_map
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            // Delegate to the "fetch-then-initialize" implementation
            // Pass `template_spec` as explicit template id so `used_template` in
            // generated kam.toml references the friendly id rather than `.tmp*`.
            return init_impl(
                path,
                id,
                name_hash,
                version,
                author,
                description_hash,
                &src_spec,
                &mut template_vars,
                force,
                Some(template_spec.as_str()),
            );
        } else {
            return Err(KamError::TemplateNotFound(format!(
                "Builtin template '{}' not found in cache at {}",
                template_spec,
                tmpl_dir.display()
            )));
        }
    } else {
        // Non-builtin: treat as a general source spec (URL, git, or local path)
        // Convert BTreeMap -> HashMap to satisfy init_impl signature.
        let name_hash: HashMap<String, String> = name_map
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let description_hash: HashMap<String, String> = description_map
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
    return init_impl(
        path,
        id,
        name_hash,
        version,
        author,
        description_hash,
        &template_spec,
        &mut template_vars,
        force,
        Some(template_spec.as_str()),
    );
}
}
