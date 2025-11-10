use std::collections::HashMap;
use std::path::Path;

pub fn init_normal(
    path: &Path,
    id: &str,
    name_map: HashMap<String, String>,
    version: &str,
    author: &str,
    description_map: HashMap<String, String>,
    template_vars: &HashMap<String, String>,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = name_map.get("en").unwrap_or(&"".to_string()).clone();
    let description = description_map.get("en").unwrap_or(&"".to_string()).clone();

    let kt = crate::types::kam_toml::KamToml::new_with_current_timestamp(
        id.to_string(),
        name_map,
        version.to_string(),
        author.to_string(),
        description_map,
        None,
    );
    kt.write_to_dir(path)?;
    let kam_toml_path = path.join("kam.toml");
    let kam_toml_rel = "kam.toml".to_string();
    super::common::print_status(&kam_toml_path, &kam_toml_rel, false, force);

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

    // Copy src from my_template with replace
    let template_path = Path::new("my_template");
    if template_path.exists() {
        let src_temp = template_path.join("src").join("{{id}}");
        if src_temp.exists() {
            let src_dir = path.join("src").join(id);
            std::fs::create_dir_all(&src_dir)?;
            let src_rel = format!("src/{}/", id);
            super::common::print_status(&src_dir, &src_rel, true, force);
            for entry in std::fs::read_dir(&src_temp)? {
                let entry = entry?;
                let filename = entry.file_name();
                let mut content = std::fs::read_to_string(entry.path())?;
                for (key, value) in template_vars {
                    content = content.replace(&format!("{{{{{}}}}}", key), value);
                }
                let dest_file = src_dir.join(&filename);
                std::fs::write(&dest_file, content)?;
                let file_rel = format!("src/{}/{}", id, filename.to_string_lossy());
                super::common::print_status(&dest_file, &file_rel, false, force);
            }
        }
    } else {
        return Err("my_template not found".into());
    }

    Ok(())
}
