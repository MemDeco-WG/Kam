use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use crate::types::kam_toml::module::{ModuleType, TmplSection};

pub fn init_template(
    path: &Path,
    id: &str,
    name_map: HashMap<String, String>,
    version: &str,
    author: &str,
    description_map: HashMap<String, String>,
    vars: &[String],
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let variables = super::template_vars::parse_template_variables(vars)?;

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
    kt.kam.tmpl = Some(TmplSection { used_template: None, variables: variables_btree });
    let kam_toml_rel = "kam.toml".to_string();
    super::common::print_status(&path.join("kam.toml"), &kam_toml_rel, false, force);
    kt.write_to_dir(path)?;

    // Extract builtin tmpl template
    let (_temp_dir, template_path) = super::common::extract_builtin_template("tmpl")?;

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
            // Replace placeholders
            for (key, var_def) in &variables {
                if let Some(default) = &var_def.default {
                    content = content.replace(&format!("{{{{{}}}}}", key), default);
                }
            }
            let dest_file = src_dir.join(&filename);
            let file_rel = format!("src/{}/{}", id, filename.to_string_lossy());
            super::common::print_status(&dest_file, &file_rel, false, force);
            std::fs::write(&dest_file, content)?;
        }
    }

    Ok(())
}
