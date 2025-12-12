use crate::errors::KamError;

use super::args::InitArgs;
use super::{impl_mod, post_init, pre_init};

/// Run the init command
pub fn run(args: InitArgs) -> Result<(), KamError> {
    // Prepare initialization data
    // Make `data` mutable because we'll pass a mutable reference to the template vars later
    let data = pre_init::prepare_init(&args)?;

    // Merge default template variables from `pre_init` with any CLI-provided vars.
    // `pre_init` already parsed CLI `--var` into `template_vars` and merged defaults,
    // so simply convert the resulting `HashMap` into `key=value` strings to feed the
    // `init_template` function which expects `&[String]`.
    let mut merged_var_vec: Vec<String> = data
        .template_vars
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    // Keep ordering deterministic for reproducibility
    merged_var_vec.sort();

    // Initialize using template with merged variables (pass clones to avoid moving `data`)
    impl_mod::init_template(
        &data.path,
        &data.id,
        data.name.clone(),
        &data.version,
        &data.author,
        data.description.clone(),
        &merged_var_vec,
        Some(data.impl_template.clone()),
        args.force,
        data.module_type,
        data.update_json.clone(),
    )?;

    // Post-process
    post_init::post_process(&data.path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;

    #[test]
    #[serial]
    fn test_init_creates_kam_toml_with_defaults() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();

        let args = InitArgs {
            name: "my_test_module".to_string(),
            id: None,
            project_name: None,
            version: None,
            author: None,
            update_json: None,
            description: None,
            force: true,
            r#impl: None,
            var: vec![],
            template: Some("tmpl_template".to_string()),
            tmpl: false,
        };

        run(args).unwrap();

        // Expect project directory created
        let kt_path = dir.join("my_test_module").join("kam.toml");
        assert!(kt_path.exists());
        let kt = crate::types::kam_toml::KamToml::load_from_file(&kt_path).unwrap();
        assert_eq!(kt.prop.id, "my_test_module");

        std::env::set_current_dir(orig).unwrap();
    }

    #[test]
    #[serial]
    fn test_init_with_custom_id_and_author() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();

        let args = InitArgs {
            name: "custom_mod".to_string(),
            id: Some("custom.id".to_string()),
            project_name: Some("Custom Name".to_string()),
            version: Some("0.1.2".to_string()),
            author: Some("LIghtJUNction".to_string()),
            update_json: None,
            description: Some("A custom module".to_string()),
            force: true,
            r#impl: None,
            var: vec![],
            template: Some("tmpl_template".to_string()),
            tmpl: false,
        };

        run(args).unwrap();
        let kt_path = dir.join("custom_mod").join("kam.toml");
        assert!(kt_path.exists());
        let kt = crate::types::kam_toml::KamToml::load_from_file(&kt_path).unwrap();
        assert_eq!(kt.prop.id, "custom.id");
        assert_eq!(kt.prop.author, "LIghtJUNction");
        assert_eq!(kt.prop.version, "0.1.2");

        std::env::set_current_dir(orig).unwrap();
    }
}
