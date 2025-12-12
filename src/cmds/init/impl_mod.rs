use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;
use crate::types::kam_toml::sections::TmplSection;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use tera::{Context, Tera};

// Helper to extract archive
fn extract_archive(path: &Path, dst: &Path) -> Result<(), KamError> {
    let file = fs::File::open(path).map_err(KamError::Io)?;
    // Simple detection based on extension
    let path_str = path.to_string_lossy().to_lowercase();
    if path_str.ends_with(".tar.gz") || path_str.ends_with(".tgz") {
        let tar = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(dst).map_err(|e| KamError::Io(e))?;
    } else if path_str.ends_with(".zip") {
        let mut archive = zip::ZipArchive::new(file).map_err(KamError::Zip)?;
        archive.extract(dst).map_err(|e| KamError::Zip(e))?;
    } else {
        return Err(KamError::UnsupportedArchive(format!(
            "Unsupported archive: {}",
            path.display()
        )));
    }
    Ok(())
}

fn merge_template_defaults(
    kt_path: &Path,
    template_vars: &mut HashMap<String, String>,
) -> Result<(), KamError> {
    if !kt_path.exists() {
        return Ok(());
    }
    let kt = KamToml::load_from_file(kt_path)?;
    if let Some(tmpl) = kt.tmpl {
        for (k, v) in tmpl.variables {
            if let Some(default_val) = v.default {
                template_vars.entry(k).or_insert(default_val);
            }
        }
    }
    Ok(())
}

pub fn init_impl(
    path: &Path,
    id: &str,
    name: String,
    version: &str,
    author: &str,
    description: String,
    impl_source: &Path,
    template_vars: &mut HashMap<String, String>,
    force: bool,
    explicit_template_id: Option<&str>,
) -> Result<(), KamError> {
    // Prepare a temp dir for extraction if needed
    let temp_dir = tempfile::tempdir().map_err(KamError::Io)?;
    let mut template_path = impl_source.to_path_buf();

    if template_path.exists() {
        if template_path.is_file() {
            // Extract archive
            let extract_dst = temp_dir.path().join("extracted");
            fs::create_dir_all(&extract_dst).map_err(KamError::Io)?;
            extract_archive(&template_path, &extract_dst)?;
            template_path = extract_dst;
        }
    } else {
        return Err(KamError::InvalidDirectory(format!(
            "Template source not found: {}",
            impl_source.display()
        )));
    }

    // Flatten single directory if present
    if template_path.is_dir() {
        let entries: Vec<_> = fs::read_dir(&template_path)
            .map_err(KamError::Io)?
            .filter_map(|e| e.ok())
            .collect();

        // If only one entry and it is a directory, descend into it
        if entries.len() == 1 && entries[0].path().is_dir() {
            template_path = entries[0].path();
        }
    }

    // Determine archive_id
    let archive_id = if let Some(eid) = explicit_template_id {
        eid.to_string()
    } else {
        "template".to_string()
    };

    // Load template variables from template's kam.toml
    let template_kam_path = template_path.join("kam.toml");
    if template_kam_path.exists() {
        merge_template_defaults(&template_kam_path, template_vars)?;
    }

    // Prepare KamToml
    let mut kt = KamToml::new_with_current_timestamp(
        id.to_string(),
        name.clone(),
        version.to_string(),
        author.to_string(),
        description.clone(),
        None,
        None,
    );
    kt.kam.tmpl = Some(TmplSection {
        used_template: Some(archive_id.clone()),
        variables: BTreeMap::new(),
    });

    // Extract # vars
    let mut kam_vars: Vec<(String, String)> = Vec::new();
    let keys: Vec<String> = template_vars.keys().cloned().collect();
    for k in keys {
        if k.starts_with('#') {
            if let Some(v) = template_vars.get(&k) {
                kam_vars.push((k.clone(), v.clone()));
            }
        }
    }
    for (k, _) in &kam_vars {
        template_vars.remove(k);
    }

    if !kam_vars.is_empty() {
        kt.apply_vars(kam_vars)?;
    }

    // Ensure dest dir
    if !path.exists() {
        fs::create_dir_all(path).map_err(KamError::Io)?;
    }

    // Merge kt vars into template_vars
    let kt_vars = crate::template::TemplateVariableProcessor::flatten_kam_toml(&kt);
    for (k, v) in kt_vars {
        template_vars.entry(k).or_insert(v);
    }

    // Ensure shallow keys
    template_vars
        .entry("id".to_string())
        .or_insert_with(|| kt.prop.id.clone());
    template_vars
        .entry("version".to_string())
        .or_insert_with(|| kt.prop.version.clone());
    template_vars
        .entry("versionCode".to_string())
        .or_insert_with(|| kt.prop.versionCode.to_string());
    template_vars
        .entry("author".to_string())
        .or_insert_with(|| kt.prop.author.clone());

    template_vars
        .entry("name".to_string())
        .or_insert_with(|| kt.prop.name.clone());

    template_vars
        .entry("description".to_string())
        .or_insert_with(|| kt.prop.description.clone());

    // Check if kam.toml exists BEFORE copy_and_replace
    let kam_toml_path = path.join("kam.toml");
    let kam_toml_existed_before_copy = kam_toml_path.exists();

    // Copy files (respect build.include/build.exclude if present)
    let excludes = kt.kam.build.as_ref().and_then(|b| b.exclude.clone());
    let includes = kt.kam.build.as_ref().and_then(|b| b.include.clone());

    crate::template::TemplateManager::copy_and_replace_with_rules(
        &template_path,
        path,
        template_vars,
        force,
        &archive_id,
        excludes,
        includes,
    )?;

    // If kam.toml doesn't exist (not in template), write the generated one
    if !kam_toml_path.exists() {
        kt.write_to_dir(path)?;
    }

    // Render kam.toml in place if it exists
    // Only process if force is set OR kam.toml was just created (didn't exist before copy_and_replace)
    if kam_toml_path.exists() && (force || !kam_toml_existed_before_copy) {
        let content = fs::read_to_string(&kam_toml_path).map_err(KamError::Io)?;
        let mut context = Context::new();
        for (k, v) in template_vars.iter() {
            context.insert(k, v);
        }
        let mut tera = Tera::default();
        match tera.render_str(&content, &context) {
            Ok(rendered) => {
                // Parse rendered kam.toml and fix template-specific values
                match toml::from_str::<KamToml>(&rendered) {
                    Ok(mut rendered_kt) => {
                        // Override with user-provided values
                        rendered_kt.prop.id = id.to_string();
                        rendered_kt.prop.name = name.clone();
                        rendered_kt.prop.version = version.to_string();
                        rendered_kt.prop.author = author.to_string();
                        rendered_kt.prop.description = description.clone();
                        rendered_kt.prop.versionCode = kt.prop.versionCode;

                        // Reset template-specific build settings to sane defaults
                        if let Some(ref mut build) = rendered_kt.kam.build {
                            // Reset target_dir to "dist" (template may have ../../src/assets/tmpl)
                            build.target_dir = Some("dist".to_string());
                            // Reset output_file to use the new module ID (simple id-only default)
                            build.output_file = Some("{{id}}".to_string());
                            // Clear template-specific exclude list
                            build.exclude = None;
                        }

                        // Reset module_type to Kam (template's kam.toml has module_type = "template")
                        rendered_kt.kam.module_type = ModuleType::Kam;

                        // Reset workspace members (template workspace is not relevant)
                        if let Some(ref mut workspace) = rendered_kt.kam.workspace {
                            workspace.members = Some(vec![".".to_string()]);
                        }

                        // Write the fixed kam.toml
                        let fixed_content =
                            toml::to_string_pretty(&rendered_kt).unwrap_or(rendered.clone());
                        fs::write(&kam_toml_path, fixed_content).map_err(KamError::Io)?;

                        // Replace current in-memory kt with the rendered/finalized kam.toml to ensure
                        // subsequent logic (e.g. env file writing) reflects the final state
                        kt = rendered_kt;
                    }
                    Err(_) => {
                        // Fallback: just write the rendered content
                        fs::write(&kam_toml_path, rendered).map_err(KamError::Io)?;
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to render kam.toml template: {}", e);
            }
        }
    }

    // Write resolved template variables into `template-vars.env` during `kam init`
    // This file contains `KEY="VALUE"` lines that can be consumed or sourced by hooks.
    // Keys are normalized into `KAM_<PATH>` uppercase; dots and dashes become underscores.
    let env_file_path = path.join("template-vars.env");
    let mut env_lines: Vec<String> = Vec::new();

    // Basic project-level details
    env_lines.push(format!("KAM_PROJECT_ROOT=\"{}\"", path.to_string_lossy()));
    env_lines.push(format!("KAM_MODULE_ID=\"{}\"", kt.prop.id));
    env_lines.push(format!("KAM_MODULE_VERSION=\"{}\"", kt.prop.version));
    env_lines.push(format!(
        "KAM_MODULE_VERSION_CODE=\"{}\"",
        kt.prop.versionCode
    ));
    env_lines.push(format!("KAM_MODULE_NAME=\"{}\"", kt.prop.get_name()));
    env_lines.push(format!("KAM_MODULE_AUTHOR=\"{}\"", kt.prop.author));
    env_lines.push(format!(
        "KAM_MODULE_DESCRIPTION=\"{}\"",
        kt.prop.get_description()
    ));

    // Add flattened kam.toml keys (prop.* etc) as KAM_<PATH>
    let kt_flatvars = crate::template::TemplateVariableProcessor::flatten_kam_toml(&kt);
    for (k, v) in kt_flatvars.iter() {
        let base = k.to_ascii_uppercase().replace('.', "_").replace('-', "_");
        let key = format!("KAM_{}", base);
        let v_escaped = v.replace('"', "\\\"");
        env_lines.push(format!("{}=\"{}\"", key, v_escaped));
    }

    // Add template-defined variables (KAM_TMPL_<NAME>) using actual values in template_vars or defaults
    if let Some(tmpl_section) = &kt.kam.tmpl {
        for (var_name, var_def) in tmpl_section.variables.iter() {
            let nm = var_name
                .to_ascii_uppercase()
                .replace('.', "_")
                .replace('-', "_");
            let key = format!("KAM_TMPL_{}", nm);
            let val = template_vars
                .get(var_name)
                .cloned()
                .or_else(|| var_def.default.clone())
                .unwrap_or_default();
            let val_escaped = val.replace('"', "\\\"");
            env_lines.push(format!("{}=\"{}\"", key, val_escaped));
        }
    }

    // Persist file (create/overwrite)
    fs::write(&env_file_path, env_lines.join("\n")).map_err(KamError::Io)?;

    Ok(())
}

pub fn init_template(
    path: &Path,
    id: &str,
    name: String,
    version: &str,
    author: &str,
    description: String,
    var: &[String],
    impl_template: Option<String>,
    force: bool,
    _module_type: ModuleType,
    _update_json: Option<String>,
) -> Result<(), KamError> {
    let mut template_vars = crate::template::TemplateManager::parse_template_vars(var)?;

    let template_spec = impl_template
        .as_deref()
        .unwrap_or("kam_template")
        .to_string();

    // Strategy:
    // 1. Check if `template_spec` exists as a local file or directory (relative to CWD).
    // 2. Check if `template_spec` exists in cache or built-in assets.
    // 3. Check if `template_spec` exists in project-local `tmpl/` or `templates/` dirs.
    // 4. If NOT found and `template_spec` does NOT end with `_template` or look like a file archive,
    //    append `_template` and retry steps 2-3 (but not step 1 again, usually).

    let mut potential_names = vec![template_spec.clone()];

    // If it doesn't have an extension and doesn't end in _template, try appending it as a fallback
    let is_archive_or_path = template_spec.contains('/') ||
                            template_spec.contains('\\') ||
                            template_spec.ends_with(".tar.gz") ||
                            template_spec.ends_with(".zip");

    if !template_spec.ends_with("_template") && !is_archive_or_path {
        potential_names.push(format!("{}_template", template_spec));
    }

    for (_name_idx, spec) in potential_names.iter().enumerate() {
        // 1. Direct local path (only for the first/raw spec, or if we really want to support 'foo_template' local dir)
        let spec_path = Path::new(spec);
        if spec_path.exists() {
             return init_impl(
                path,
                id,
                name,
                version,
                author,
                description,
                spec_path,
                &mut template_vars,
                force,
                Some(spec.as_str()),
            );
        }

        // 2. Built-in assets
        // We typically store built-ins as "kam_template.tar.gz" -> check spec
        // If the spec is "kam", we might be in the second iteration where spec="kam_template"
        let asset_name = format!("{}.tar.gz", spec);
        if let Some(asset) = crate::assets::tmpl::TmplAssets::get(&asset_name) {
            let temp_dir = tempfile::tempdir().map_err(KamError::Io)?;
            let temp_path = temp_dir.path().join(format!("{}.tar.gz", spec));
            fs::write(&temp_path, &asset.data).map_err(KamError::Io)?;

            return init_impl(
                path,
                id,
                name,
                version,
                author,
                description,
                &temp_path,
                &mut template_vars,
                force,
                Some(spec.as_str()),
            );
        }

        // 3. Cache
        if let Ok(Some(cached_path)) =
            crate::template::TemplateCacheManager::resolve_template_path(spec)
        {
            return init_impl(
                path,
                id,
                name,
                version,
                author,
                description,
                &cached_path,
                &mut template_vars,
                force,
                Some(spec.as_str()),
            );
        }

        // 4. Project-local folder search (tmpl/ or templates/)
        let project_local_dirs = vec!["tmpl", "templates"];
        let archive_exts = vec![".tar.gz", ".tgz", ".zip", ".tar"];
        let mut candidates: Vec<PathBuf> = Vec::new();

        for d in &project_local_dirs {
            let base = Path::new(d);
            candidates.push(base.join(spec));
             for ext in &archive_exts {
                candidates.push(base.join(format!("{}{}", spec, ext)));
            }
        }
        // Also check project root for archives
         for ext in &archive_exts {
            candidates.push(Path::new(&format!("{}{}", spec, ext)).to_path_buf());
        }

        for candidate in candidates {
            if candidate.exists() {
                 return init_impl(
                    path,
                    id,
                    name.clone(),
                    version,
                    author,
                    description.clone(),
                    &candidate,
                    &mut template_vars,
                    force,
                    Some(spec.as_str()),
                );
            }
        }
    }

    // Final failure
    Err(KamError::TemplateNotFound(format!(
        "Template '{}' not found in built-in assets, local path, cache, or project directories.",
        template_spec
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    #[serial]
    fn test_init_template_find_local_tmpl_dir() {
        // Create a temporary project root and switch current directory to it
        let tmp = tempdir().expect("tempdir");
        let project_root = tmp.path();
        let tmpl_dir = project_root.join("tmpl");
        fs::create_dir_all(&tmpl_dir).expect("create tmpl dir");

        // Create a minimal template directory: tmpl/kam_template/
        let tk_dir = tmpl_dir.join("kam_template");
        fs::create_dir_all(&tk_dir).expect("create template dir");

        // Write a minimal kam.toml inside the template dir
        // Keep the content minimal but valid for KamToml parsing
        let kt_content = r#"[prop]
id = "kam_template"
name = "{{project_name}}"
version = "0.1.0"
versionCode = 1
author = "{{author}}"
description = "Test template"
metamodule = false

[kam]
module_type = "template"
[kam.tmpl.variables]
"#;
        fs::write(tk_dir.join("kam.toml"), kt_content).expect("write kam.toml");

        // Change current directory so that init_template's local search picks up tmpl/
        let prev_cwd = env::current_dir().expect("cwd");
        env::set_current_dir(&project_root).expect("set cwd");

        // Destination for initialization
        let dest_dir = project_root.join("my_module");

        // Prepare minimal arguments for init_template
        let vars: Vec<String> = Vec::new();

        let res = init_template(
            &dest_dir,
            "com.example.test",
            "Example Test".to_string(),
            "0.1.0",
            "Author",
            "Description".to_string(),
            &vars,
            Some("kam_template".to_string()),
            true,
            ModuleType::Kam,
            None,
        );

        // restore cwd
        env::set_current_dir(prev_cwd).expect("restore cwd");

        assert!(res.is_ok(), "init_template failed: {:?}", res.err());
        assert!(dest_dir.exists(), "destination dir not created");
        assert!(
            dest_dir.join("kam.toml").exists(),
            "kam.toml not created in destination dir"
        );
    }
}
