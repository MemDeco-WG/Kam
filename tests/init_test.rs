use std::fs;
use tempfile::TempDir;

use kam::cmds::init::{InitArgs, run};

    #[test]
    fn test_init_basic() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap().to_string();

        // Create my_template directory structure in the workspace root
        let workspace_root = std::env::current_dir().unwrap();
        let template_dir = workspace_root.join("my_template");
        if !template_dir.exists() {
            fs::create_dir_all(&template_dir).unwrap();
            let src_dir = template_dir.join("src").join("{{id}}");
            fs::create_dir_all(&src_dir).unwrap();
            fs::write(src_dir.join("test.sh"), "#!/bin/bash\necho hello").unwrap();
        }

        let args = InitArgs {
            path: path.clone(),
            id: Some("test_module".to_string()),
            name: Some("Test Module".to_string()),
            version: Some("1.0.0".to_string()),
            author: Some("Test Author".to_string()),
            description: Some("Test Description".to_string()),
            force: false,
            lib: false,
            tmpl: false,
            r#impl: None,
            meta_inf: false,
            web_root: false,
            var: vec![],
        };

        let result = run(args);
        // Clean up
        if template_dir.exists() {
            fs::remove_dir_all(&template_dir).unwrap();
        }
        assert!(result.is_ok());

        // Check if kam.toml was created
        let kam_toml_path = temp_dir.path().join("kam.toml");
        assert!(kam_toml_path.exists());

        // Check content
        let content = fs::read_to_string(&kam_toml_path).unwrap();
        assert!(content.contains("id = \"test_module\""));
        assert!(content.contains("Test Module"));  // name is in a HashMap now
        assert!(content.contains("version = \"1.0.0\""));
        assert!(content.contains("author = \"Test Author\""));
        assert!(content.contains("Test Description"));  // description is in a HashMap now
    }

    #[test]
    fn test_init_with_template_vars() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap().to_string();

        // Create my_template directory structure in the workspace root
        let workspace_root = std::env::current_dir().unwrap();
        let template_dir = workspace_root.join("my_template");
        if !template_dir.exists() {
            fs::create_dir_all(&template_dir).unwrap();
            let src_dir = template_dir.join("src").join("{{id}}");
            fs::create_dir_all(&src_dir).unwrap();
            fs::write(src_dir.join("test.sh"), "#!/bin/bash\necho {{name}}").unwrap();
        }

        let args = InitArgs {
            path: path.clone(),
            id: None,
            name: None,
            version: None,
            author: None,
            description: None,
            force: false,
            lib: false,
            tmpl: false,
            r#impl: None,
            meta_inf: false,
            web_root: false,
            var: vec!["name=My Custom Module".to_string(), "version=2.0.0".to_string()],
        };

        let result = run(args);
        // Clean up
        if template_dir.exists() {
            fs::remove_dir_all(&template_dir).unwrap();
        }
        assert!(result.is_ok());

        let kam_toml_path = temp_dir.path().join("kam.toml");
        assert!(kam_toml_path.exists());

        let content = fs::read_to_string(&kam_toml_path).unwrap();
        assert!(content.contains("My Custom Module"));  // name is in a HashMap now
        assert!(content.contains("version = \"2.0.0\""));
    }

    #[test]
    fn test_init_template_mode() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap().to_string();

        // Create my_template directory structure in the workspace root
        let workspace_root = std::env::current_dir().unwrap();
        let template_dir = workspace_root.join("my_template");
        if !template_dir.exists() {
            fs::create_dir_all(&template_dir).unwrap();
            let src_dir = template_dir.join("src").join("{{id}}");
            fs::create_dir_all(&src_dir).unwrap();
            fs::write(src_dir.join("test.sh"), "#!/bin/bash\necho {{name}}").unwrap();
        }

        let args = InitArgs {
            path: path.clone(),
            id: Some("template_module".to_string()),
            name: Some("Template Module".to_string()),
            version: Some("1.0.0".to_string()),
            author: Some("Template Author".to_string()),
            description: Some("Template Description".to_string()),
            force: false,
            lib: false,
            tmpl: true,
            r#impl: None,
            meta_inf: false,
            web_root: false,
            var: vec!["name=string:true:Template Name".to_string()],
        };

        let result = run(args);
        // Clean up
        if template_dir.exists() {
            fs::remove_dir_all(&template_dir).unwrap();
        }
        assert!(result.is_ok());

        let kam_toml_path = temp_dir.path().join("kam.toml");
        assert!(kam_toml_path.exists());

        let content = fs::read_to_string(&kam_toml_path).unwrap();
        // Check that template-related content is present
        assert!(content.contains("module_type = \"Template\""));
        assert!(content.contains("[kam.tmpl.variables"));  // variables section should be present

        // Check if src was copied
        let src_path = temp_dir.path().join("src").join("template_module");
        assert!(src_path.exists());
        let test_sh = src_path.join("test.sh");
        assert!(test_sh.exists());
        let content = fs::read_to_string(&test_sh).unwrap();
        assert_eq!(content, "#!/bin/bash\necho {{name}}"); // No replacement in tmpl mode
    }

    #[test]
    fn test_init_impl_mode() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap().to_string();

        // Create template directory
        let template_dir = temp_dir.path().join("template");
        fs::create_dir_all(&template_dir).unwrap();
        fs::write(template_dir.join("kam.toml"), r#"
[prop]
id = "template"
version = "1.0.0"
versionCode = 1
author = "Template Author"

[prop.name]
en = "Template"

[prop.description]
en = "Template Description"

[mmrl]
zip_url = "https://example.com/module.zip"
changelog = "https://example.com/changelog.md"

[kam]
module_type = "Template"

[kam.tmpl]

[kam.tmpl.variables]
name = { var_type = "string", required = true, default = "Default Name" }
"#).unwrap();

        let src_dir = template_dir.join("src").join("template");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("script.sh"), "#!/bin/bash\necho {{{{name}}}}").unwrap();

        let args = InitArgs {
            path: path.clone(),
            id: Some("impl_module".to_string()),
            name: Some("Impl Module".to_string()),
            version: Some("1.0.0".to_string()),
            author: Some("Impl Author".to_string()),
            description: Some("Impl Description".to_string()),
            force: false,
            lib: false,
            tmpl: false,
            r#impl: Some(template_dir.to_str().unwrap().to_string()),
            meta_inf: false,
            web_root: false,
            var: vec!["name=Custom Name".to_string()],
        };

        let result = run(args);
        if let Err(e) = &result {
            eprintln!("Error in test_init_impl_mode: {}", e);
        }
        assert!(result.is_ok());

        let kam_toml_path = temp_dir.path().join("kam.toml");
        assert!(kam_toml_path.exists());

        let content = fs::read_to_string(&kam_toml_path).unwrap();
        assert!(content.contains("used_template") && content.contains("template"));

        // Check if src was copied and replaced
        let src_path = temp_dir.path().join("src").join("impl_module");
        if !src_path.exists() {
            eprintln!("src_path does not exist: {:?}", src_path);
            eprintln!("Contents of temp_dir: {:?}", fs::read_dir(temp_dir.path()).unwrap().map(|e| e.unwrap().path()).collect::<Vec<_>>());
            if let Ok(entries) = fs::read_dir(temp_dir.path().join("src")) {
                eprintln!("Contents of src: {:?}", entries.map(|e| e.unwrap().path()).collect::<Vec<_>>());
            }
        }
        assert!(src_path.exists());
        let script_sh = src_path.join("script.sh");
        assert!(script_sh.exists());
        let content = fs::read_to_string(&script_sh).unwrap();
        assert_eq!(content, "#!/bin/bash\necho Custom Name");
    }

    #[test]
    fn test_init_with_meta_inf() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap().to_string();

        let args = InitArgs {
            path: path.clone(),
            id: Some("meta_module".to_string()),
            name: Some("Meta Module".to_string()),
            version: Some("1.0.0".to_string()),
            author: Some("Meta Author".to_string()),
            description: Some("Meta Description".to_string()),
            force: false,
            lib: false,
            tmpl: false,
            r#impl: None,
            meta_inf: true,
            web_root: false,
            var: vec![],
        };

        let result = run(args);
        assert!(result.is_ok());

        let meta_inf_path = temp_dir.path().join("META-INF");
        assert!(meta_inf_path.exists());
        assert!(meta_inf_path.is_dir());
    }

    #[test]
    fn test_init_with_web_root() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap().to_string();

        let args = InitArgs {
            path: path.clone(),
            id: Some("web_module".to_string()),
            name: Some("Web Module".to_string()),
            version: Some("1.0.0".to_string()),
            author: Some("Web Author".to_string()),
            description: Some("Web Description".to_string()),
            force: false,
            lib: false,
            tmpl: false,
            r#impl: None,
            meta_inf: false,
            web_root: true,
            var: vec![],
        };

        let result = run(args);
        assert!(result.is_ok());

        let web_root_path = temp_dir.path().join("WEB-ROOT");
        assert!(web_root_path.exists());
        assert!(web_root_path.is_dir());
    }

    #[test]
    fn test_init_invalid_template_var() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap().to_string();

        let args = InitArgs {
            path: path.clone(),
            id: Some("invalid_module".to_string()),
            name: Some("Invalid Module".to_string()),
            version: Some("1.0.0".to_string()),
            author: Some("Invalid Author".to_string()),
            description: Some("Invalid Description".to_string()),
            force: false,
            lib: false,
            tmpl: false,
            r#impl: None,
            meta_inf: false,
            web_root: false,
            var: vec!["invalid_var".to_string()],
        };

        let result = run(args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid template variable format"));
    }

    #[test]
    fn test_init_template_mode_invalid_var() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap().to_string();

        let args = InitArgs {
            path: path.clone(),
            id: Some("invalid_tmpl_module".to_string()),
            name: Some("Invalid Tmpl Module".to_string()),
            version: Some("1.0.0".to_string()),
            author: Some("Invalid Tmpl Author".to_string()),
            description: Some("Invalid Tmpl Description".to_string()),
            force: false,
            lib: false,
            tmpl: true,
            r#impl: None,
            meta_inf: false,
            web_root: false,
            var: vec!["invalid".to_string()],
        };

        let result = run(args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid template variable format"));
    }

    #[test]
    fn test_init_impl_without_vars() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap().to_string();

        // Create template directory with required variables
        let template_dir = temp_dir.path().join("template_req");
        fs::create_dir_all(&template_dir).unwrap();
        fs::write(template_dir.join("kam.toml"), r#"
[prop]
id = "template_req"
version = "1.0.0"
versionCode = 1
author = "Template Req Author"

[prop.name]
en = "Template Req"

[prop.description]
en = "Template Req Description"

[mmrl]
zip_url = "https://example.com/module.zip"
changelog = "https://example.com/changelog.md"

[kam]
module_type = "Template"

[kam.tmpl]

[kam.tmpl.variables]
required_var = { var_type = "string", required = true }
"#).unwrap();

        let args = InitArgs {
            path: path.clone(),
            id: Some("impl_no_vars_module".to_string()),
            name: Some("Impl No Vars Module".to_string()),
            version: Some("1.0.0".to_string()),
            author: Some("Impl No Vars Author".to_string()),
            description: Some("Impl No Vars Description".to_string()),
            force: false,
            lib: false,
            tmpl: false,
            r#impl: Some(template_dir.to_str().unwrap().to_string()),
            meta_inf: false,
            web_root: false,
            var: vec![],
        };

        let result = run(args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Implementation requires template variables"));
    }

    #[test]
    fn test_init_force_overwrite() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap().to_string();

        // Create existing kam.toml
        let kam_toml_path = temp_dir.path().join("kam.toml");
        fs::write(&kam_toml_path, "existing content").unwrap();

        let args = InitArgs {
            path: path.clone(),
            id: Some("force_module".to_string()),
            name: Some("Force Module".to_string()),
            version: Some("1.0.0".to_string()),
            author: Some("Force Author".to_string()),
            description: Some("Force Description".to_string()),
            force: true,
            lib: false,
            tmpl: false,
            r#impl: None,
            meta_inf: false,
            web_root: false,
            var: vec![],
        };

        let result = run(args);
        assert!(result.is_ok());

        let content = fs::read_to_string(&kam_toml_path).unwrap();
        assert!(content.contains("id = \"force_module\""));
        assert!(!content.contains("existing content"));
    }