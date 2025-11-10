use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::Path;
use tempfile::TempDir;
use zip::ZipArchive;

pub fn init_impl(
    path: &Path,
    id: &str,
    name_map: HashMap<String, String>,
    version: &str,
    author: &str,
    description_map: HashMap<String, String>,
    impl_zip: &str,
    template_vars: &mut HashMap<String, String>,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut zip_id = "unknown".to_string();
    let (template_path, _temp_dir) = if impl_zip.ends_with(".zip") {
        let file = File::open(impl_zip)?;
        let mut archive = ZipArchive::new(file)?;
        let temp_dir = TempDir::new()?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = temp_dir.path().join(file.name());
            if file.name().ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut outfile = File::create(&outpath)?;
                io::copy(&mut file, &mut outfile)?;
            }
        }
        // Assume the zip has a root directory
        let root = if archive.len() > 0 {
            archive.by_index(0).unwrap().name().split('/').next().unwrap_or("").to_string()
        } else {
            "".to_string()
        };
        zip_id = root.clone();
        (temp_dir.path().join(root), Some(temp_dir))
    } else {
        (Path::new(impl_zip).to_path_buf(), None)
    };

    // Load template variables and insert defaults
    let template_kam_path = template_path.join("kam.toml");
    if template_kam_path.exists() {
        let kt_template = crate::types::kam_toml::KamToml::load_from_file(&template_kam_path)?;
        if let Some(tmpl) = &kt_template.kam.tmpl {
            for (var_name, var_def) in &tmpl.variables {
                if !template_vars.contains_key(var_name) {
                    if var_def.required {
                        if let Some(default) = &var_def.default {
                            template_vars.insert(var_name.clone(), default.clone());
                        } else {
                            return Err(format!("Required template variable '{}' not provided", var_name).into());
                        }
                    } else if let Some(default) = &var_def.default {
                        template_vars.insert(var_name.clone(), default.clone());
                    }
                }
            }
        }
    }

    let mut kt = crate::types::kam_toml::KamToml::new_with_current_timestamp(
        id.to_string(),
        name_map.clone(),
        version.to_string(),
        author.to_string(),
        description_map.clone(),
        None,
    );
    kt.kam.tmpl = Some(crate::types::kam_toml::TmplSection { used_template: Some(zip_id.clone()), variables: HashMap::new() });
    kt.write_to_dir(path)?;
    let kam_toml_path = path.join("kam.toml");
    let kam_toml_rel = "kam.toml".to_string();
    super::common::print_status(&kam_toml_path, &kam_toml_rel, false, force);

    // Replace in kam.toml
    if !template_vars.is_empty() {
        let name = name_map.get("en").unwrap_or(&"".to_string()).clone();
        let description = description_map.get("en").unwrap_or(&"".to_string()).clone();
        let mut content = std::fs::read_to_string(&kam_toml_path)?;
        for (key, value) in template_vars.iter() {
            let default_value = match key.as_str() {
                "id" => id,
                "name" => &name,
                "version" => version,
                "author" => author,
                "description" => &description,
                _ => continue,
            };
            content = content.replace(default_value, value);
        }
        std::fs::write(&kam_toml_path, content)?;
    }

    // Copy src from template with replace
    if impl_zip.ends_with(".zip") {
        if template_path.exists() {
            let src_temp = template_path.join("src").join(&zip_id);
            if src_temp.exists() {
                let src_dir = path.join("src").join(id);
                std::fs::create_dir_all(&src_dir)?;
                let src_rel = format!("src/{}/", id);
                super::common::print_status(&src_dir, &src_rel, true, force);
                for entry in std::fs::read_dir(&src_temp)? {
                    let entry = entry?;
                    let filename = entry.file_name();
                    let mut content = std::fs::read_to_string(entry.path())?;
                    for (key, value) in template_vars.iter() {
                        content = content.replace(&format!("{{{{{}}}}}", key), value);
                    }
                    let dest_file = src_dir.join(&filename);
                    std::fs::write(&dest_file, content)?;
                    let file_rel = format!("src/{}/{}", id, filename.to_string_lossy());
                    super::common::print_status(&dest_file, &file_rel, false, force);
                }
            }
        } else {
            return Err("Template not found".into());
        }
    } else {
        if template_path.exists() {
            let src_temp = template_path.join("src").join(&zip_id);
            if src_temp.exists() {
                let src_dir = path.join("src").join(id);
                std::fs::create_dir_all(&src_dir)?;
                let src_rel = format!("src/{}/", id);
                super::common::print_status(&src_dir, &src_rel, true, force);
                for entry in std::fs::read_dir(&src_temp)? {
                    let entry = entry?;
                    let filename = entry.file_name();
                    let mut content = std::fs::read_to_string(entry.path())?;
                    for (key, value) in template_vars.iter() {
                        content = content.replace(&format!("{{{{{}}}}}", key), value);
                    }
                    let dest_file = src_dir.join(&filename);
                    std::fs::write(&dest_file, content)?;
                    let file_rel = format!("src/{}/{}", id, filename.to_string_lossy());
                    super::common::print_status(&dest_file, &file_rel, false, force);
                }
            }
        } else {
            return Err("Template not found".into());
        }
    }

    Ok(())
}
