use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io;
use std::path::Path;
use tempfile::TempDir;
use zip::ZipArchive;
use crate::types::kam_toml::module::TmplSection;
// toml_edit not needed here; use toml::Value for mutation

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
    // Declare zip_id here; it will be assigned in the branches below.
    let zip_id: String;
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
        let template_path = Path::new(impl_zip).to_path_buf();
        zip_id = template_path.file_name().unwrap_or(std::ffi::OsStr::new("unknown")).to_str().unwrap_or("unknown").to_string();
        (template_path, None)
    };

    // Load template variables and insert defaults (refactored to helper to avoid deep nesting)
    let template_kam_path = template_path.join("kam.toml");
    if template_kam_path.exists() {
        fn merge_template_defaults(
            kt_path: &std::path::Path,
            template_vars: &mut HashMap<String, String>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let kt_template = crate::types::kam_toml::KamToml::load_from_file(kt_path)?;
            if let Some(tmpl) = &kt_template.kam.tmpl {
                for (var_name, var_def) in &tmpl.variables {
                    if template_vars.contains_key(var_name.as_str()) {
                        continue;
                    }

                    if var_def.required {
                        if let Some(default) = &var_def.default {
                            template_vars.insert(var_name.clone(), default.clone());
                            continue;
                        }
                        if let Some(note) = &var_def.note {
                            return Err(format!("Required template variable '{}' not provided: {}", var_name, note).into());
                        }
                        return Err(format!("Required template variable '{}' not provided", var_name).into());
                    }

                    if let Some(default) = &var_def.default {
                        template_vars.insert(var_name.clone(), default.clone());
                    }
                }
            }
            Ok(())
        }

        merge_template_defaults(&template_kam_path, template_vars)?;
    }

    let name_map_btree: BTreeMap<_, _> = name_map.clone().into_iter().collect();
    let description_map_btree: BTreeMap<_, _> = description_map.clone().into_iter().collect();

    let kam_toml_path = path.join("kam.toml");
    let kam_toml_rel = "kam.toml".to_string();
    super::common::print_status(&kam_toml_path, &kam_toml_rel, false, force);

    let mut kt = crate::types::kam_toml::KamToml::new_with_current_timestamp(
        id.to_string(),
        name_map_btree,
        version.to_string(),
        author.to_string(),
        description_map_btree,
        None,
    );
    kt.kam.tmpl = Some(TmplSection { used_template: Some(zip_id.clone()), variables: BTreeMap::new() });

    // Apply any template variables that target kam.toml itself. Variables
    // intended to modify kam.toml must start with a leading '#', e.g.
    // `#prop.name.en` or `#prop.version`. These are applied to the KamToml
    // structure via toml_edit so nested fields can be set.
    let mut kam_vars: Vec<(String, String)> = Vec::new();
    let mut normal_vars: Vec<String> = Vec::new();
    for k in template_vars.keys() {
        if k.starts_with('#') {
            kam_vars.push((k.clone(), template_vars.get(k).unwrap().clone()));
        } else {
            normal_vars.push(k.clone());
        }
    }
    // Remove kam vars from template_vars so they won't be applied to file contents later
    for k in &kam_vars {
        template_vars.remove(&k.0);
    }

    if !kam_vars.is_empty() {
        // Serialize current kt to TOML, edit via toml::Value, then deserialize back.
        let toml_str = kt.to_toml()?;
        let mut val: toml::Value = toml::from_str(&toml_str).map_err(|e| format!("toml parse error: {}", e))?;
        for (raw_k, v) in kam_vars {
            let path = raw_k.trim_start_matches('#');
            let parts: Vec<&str> = path.split('.').collect();
            if parts.is_empty() { continue; }
            // Start from the root table
            let mut cursor = val.as_table_mut().ok_or("invalid toml root")?;
            for (i, part) in parts.iter().enumerate() {
                if i == parts.len() - 1 {
                    cursor.insert(part.to_string(), toml::Value::String(v.clone()));
                } else {
                    let key = part.to_string();
                    // To avoid nested mutable borrows, take ownership of any existing entry
                    // and then re-insert a table we can descend into.
                    let existing = cursor.remove(&key);
                    match existing {
                        Some(toml::Value::Table(t)) => {
                            // reuse existing table
                            cursor.insert(key.clone(), toml::Value::Table(t));
                        }
                        Some(_) | None => {
                            // create/overwrite with a new table
                            cursor.insert(key.clone(), toml::Value::Table(toml::value::Table::new()));
                        }
                    }
                    // Now descend into the (re)inserted table
                    match cursor.get_mut(&key) {
                        Some(toml::Value::Table(t2)) => cursor = t2,
                        _ => return Err("failed to create table".into()),
                    }
                }
            }
        }
        // Recreate kt from edited toml string
        let new_raw = toml::to_string(&val).map_err(|e| format!("toml serialize error: {}", e))?;
        let new_kt = crate::types::kam_toml::KamToml::from_string(new_raw)?;
        kt = new_kt;
    }
    kt.write_to_dir(path)?;

    // NOTE: For `impl` initialization, `kam.toml` is created from the
    // generated KamToml (`kt.write_to_dir`) and must NOT be modified by
    // template variables. Template variables are intended to affect files
    // other than `kam.toml` (for example source files under `src/`).
    // Therefore we intentionally do not perform any replacements inside
    // `kam.toml` here.

    // Copy src from template with replace.
    if template_path.exists() {
        // Try layouts in order of preference:
        // 1) src/<zip_id>
        // 2) src/ (flat)
        let mut src_temp = template_path.join("src").join(&zip_id);
        if !src_temp.exists() {
            let alt2 = template_path.join("src");
            if alt2.exists() {
                src_temp = alt2;
            }
        }

        if src_temp.exists() {
            let src_dir = path.join("src").join(id);
            let src_rel = format!("src/{}/", id);
            super::common::print_status(&src_dir, &src_rel, true, force);
            std::fs::create_dir_all(&src_dir)?;
            for entry in std::fs::read_dir(&src_temp)? {
                let entry = entry?;
                let filename = entry.file_name();
                let mut content = std::fs::read_to_string(entry.path())?;
                for (key, value) in template_vars.iter() {
                    content = content.replace(&format!("{{{{{}}}}}", key), value);
                }
                let dest_file = src_dir.join(&filename);
                let file_rel = format!("src/{}/{}", id, filename.to_string_lossy());
                super::common::print_status(&dest_file, &file_rel, false, force);
                std::fs::write(&dest_file, content)?;
            }
        }
    } else {
        return Err("Template not found".into());
    }

    Ok(())
}
