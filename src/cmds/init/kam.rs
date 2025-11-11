use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use crate::types::kam_toml::module::ModuleType;

pub fn init_kam(
    path: &Path,
    id: &str,
    name_map: HashMap<String, String>,
    version: &str,
    author: &str,
    description_map: HashMap<String, String>,
    template_vars: &HashMap<String, String>,
    force: bool,
    module_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = name_map.get("en").unwrap_or(&"".to_string()).clone();
    let description = description_map.get("en").unwrap_or(&"".to_string()).clone();

    // Extract builtin template
    let (_temp_dir, template_path) = super::common::extract_builtin_template(module_type)?;

    // Use default KamToml
    let mut kt = crate::types::kam_toml::KamToml::default();

    // Update with user values
    kt.prop.id = id.to_string();
    let name_map_btree: BTreeMap<_, _> = name_map.into_iter().collect();
    kt.prop.name = name_map_btree;
    kt.prop.version = version.to_string();
    kt.prop.versionCode = chrono::Utc::now().timestamp_millis() as u64;
    kt.prop.author = author.to_string();
    let description_map_btree: BTreeMap<_, _> = description_map.into_iter().collect();
    kt.prop.description = description_map_btree;

    // For library and kam, set module_type
    match module_type {
        "library" => {
            kt.kam.module_type = ModuleType::Library;
        }
        "kam" => {
            // already the default ModuleType::Kam
        }
        _ => {}
    }

    let kam_toml_path = path.join("kam.toml");
    let kam_toml_rel = "kam.toml".to_string();
    super::common::print_status(&kam_toml_path, &kam_toml_rel, false, force);

    kt.write_to_dir(path)?;

    // Replace in kam.toml
    if !template_vars.is_empty() {
        let mut content = std::fs::read_to_string(&kam_toml_path)?;
        for (key, value) in template_vars {
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

    // Copy src from template
    let src_temp = template_path.join("src").join("module");
    if src_temp.exists() {
        let src_dir = path.join("src").join(id);
        let src_rel = format!("src/{}/", id);
        super::common::print_status(&src_dir, &src_rel, true, force);
        std::fs::create_dir_all(&src_dir)?;
        for entry in std::fs::read_dir(&src_temp)? {
            let entry = entry?;
            let filename = entry.file_name();
            let mut content = std::fs::read_to_string(entry.path())?;
            for (key, value) in template_vars {
                content = content.replace(&format!("{{{{{}}}}}", key), value);
            }
            let dest_file = src_dir.join(&filename);
            let file_rel = format!("src/{}/{}", id, filename.to_string_lossy());
            super::common::print_status(&dest_file, &file_rel, false, force);
            std::fs::write(&dest_file, content)?;
        }
    }

    Ok(())
}
