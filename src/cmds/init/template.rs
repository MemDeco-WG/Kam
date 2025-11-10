use std::collections::HashMap;
use std::path::Path;

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

    let mut kt = crate::types::kam_toml::KamToml::new_with_current_timestamp(
        id.to_string(),
        name_map,
        version.to_string(),
        author.to_string(),
        description_map,
        None,
    );
    kt.kam.module_type = crate::types::kam_toml::ModuleType::Template;
    kt.kam.tmpl = Some(crate::types::kam_toml::TmplSection { used_template: None, variables });
    kt.write_to_dir(path)?;
    let kam_toml_rel = "kam.toml".to_string();
    super::common::print_status(&path.join("kam.toml"), &kam_toml_rel, false, force);

    // Create basic src directory for template
    let src_dir = path.join("src").join(id);
    std::fs::create_dir_all(&src_dir)?;
    let src_rel = format!("src/{}/", id);
    super::common::print_status(&src_dir, &src_rel, true, force);

    // Create a sample template file
    let sample_file = src_dir.join("module.sh");
    let sample_content = r#"#!/system/bin/sh
# This is a sample module script for {{name}}
# Version: {{version}}
# Author: {{author}}

MODDIR=${0%/*}

# Your module code here

echo "Module {{name}} loaded"
"#;
    std::fs::write(&sample_file, sample_content)?;
    let file_rel = format!("src/{}/module.sh", id);
    super::common::print_status(&sample_file, &file_rel, false, force);

    Ok(())
}
