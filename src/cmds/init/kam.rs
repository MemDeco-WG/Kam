use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

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
) -> Result<(), KamError> {
    // Ensure the target directory exists
    std::fs::create_dir_all(path)?;

    // Use default KamToml
    let mut kt = KamToml::default();

    // Update with user values
    kt.prop.id = id.to_string();
    let name_map_btree: BTreeMap<_, _> = name_map.into_iter().collect();
    kt.prop.name = name_map_btree.clone();
    kt.prop.version = version.to_string();
    kt.prop.versionCode = chrono::Utc::now().timestamp_millis();
    kt.prop.author = author.to_string();
    let description_map_btree: BTreeMap<_, _> = description_map.into_iter().collect();
    kt.prop.description = description_map_btree.clone();

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

    let kam_toml_rel = "kam.toml".to_string();
    crate::cmds::init::status::print_status(
        crate::cmds::init::status::StatusType::Add,
        &kam_toml_rel,
        false,
    );

    kt.write_to_dir(path)?;

    // Copy template files and replace placeholders
    let template_dir = std::path::Path::new("tmpl").join("kam_template");
    if template_dir.exists() {
        // Load template kam.toml to check required variables
        let template_kt = KamToml::load_from_dir(&template_dir)?;
        if let Some(tmpl_section) = &template_kt.kam.tmpl {
            for (var_name, var_def) in &tmpl_section.variables {
                if var_def.required && !template_vars.contains_key(var_name) {
                    let error_msg = var_def.note.clone().unwrap_or_else(|| {
                        format!("Required template variable '{}' not provided", var_name)
                    });
                    return Err(KamError::TemplateVarRequired(error_msg));
                }
            }
        }

        let mut vars = template_vars.clone();
        vars.insert("id".to_string(), id.to_string());
        vars.insert(
            "name".to_string(),
            name_map_btree.get("en").unwrap_or(&"".to_string()).clone(),
        );
        vars.insert("version".to_string(), version.to_string());
        vars.insert("author".to_string(), author.to_string());
        vars.insert(
            "description".to_string(),
            description_map_btree
                .get("en")
                .unwrap_or(&"".to_string())
                .clone(),
        );
        vars.insert("versionCode".to_string(), kt.prop.versionCode.to_string());

        crate::template::TemplateManager::copy_template_to(&template_dir, path, &vars, force, &id)?;
    }

    Ok(())
}
