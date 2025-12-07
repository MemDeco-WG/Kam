use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;
use crate::types::kam_toml::sections::TmplSection;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
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

    // Copy files
    crate::template::TemplateManager::copy_and_replace(
        &template_path,
        path,
        template_vars,
        force,
        &archive_id,
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
                            // Reset output_file to use the new module ID
                            build.output_file = Some("{{id}}-{{versionCode}}".to_string());
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

    // 1. Check built-in assets
    let asset_name = format!("{}.tar.gz", template_spec);
    if let Some(asset) = crate::assets::tmpl::TmplAssets::get(&asset_name) {
        let temp_dir = tempfile::tempdir().map_err(KamError::Io)?;
        let temp_path = temp_dir.path().join(format!("{}.tar.gz", template_spec));
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
            Some(template_spec.as_str()),
        );
    }

    // 2. Check cache
    if let Ok(Some(cached_path)) =
        crate::template::TemplateCacheManager::resolve_template_path(&template_spec)
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
            Some(template_spec.as_str()),
        );
    }

    // 3. Fallback to local path
    init_impl(
        path,
        id,
        name,
        version,
        author,
        description,
        Path::new(&template_spec),
        &mut template_vars,
        force,
        Some(template_spec.as_str()),
    )
}
