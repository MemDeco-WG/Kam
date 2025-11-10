#[cfg(test)]
mod tests {
    use crate::types::kam_toml::KamToml;
    use crate::types::kam_toml::error::ValidationResult;
    use crate::types::kam_toml::sections::{Dependency, DependencySection};
    use std::collections::BTreeMap;

    #[test]
    fn test_validate_id_valid() {
        assert!(matches!(KamToml::validate_id("a_module"), ValidationResult::Valid));
        assert!(matches!(KamToml::validate_id("a.module"), ValidationResult::Valid));
        assert!(matches!(KamToml::validate_id("module-101"), ValidationResult::Valid));
    }

    #[test]
    fn test_validate_id_invalid() {
        assert!(matches!(KamToml::validate_id("a module"), ValidationResult::Invalid(_)));
        assert!(matches!(KamToml::validate_id("1_module"), ValidationResult::Invalid(_)));
        assert!(matches!(KamToml::validate_id("-a-module"), ValidationResult::Invalid(_)));
    }

    #[test]
    fn test_default() {
        let kt = KamToml::default();
        assert_eq!(kt.prop.version, "0.1.0");
        assert_eq!(kt.prop.versionCode, 1);
        if let Some(mmrl) = &kt.mmrl {
            assert_eq!(mmrl.categories, Some(vec!["Kam".to_string()]));
            assert_eq!(mmrl.keywords, Some(vec!["android".to_string(), "root".to_string(), "module".to_string()]));
        }
    }

    #[test]
    fn test_new() {
        let mut name = BTreeMap::new();
        name.insert("en".to_string(), "Test Module".to_string());
        let mut description = BTreeMap::new();
        description.insert("en".to_string(), "Test Description".to_string());
        let kt = KamToml::new(
            "test_id".to_string(),
            name.clone(),
            "1.0.0".to_string(),
            100,
            "Test Author".to_string(),
            description.clone(),
            Some("https://example.com/update.json".to_string()),
        );
        assert_eq!(kt.prop.id, "test_id");
        assert_eq!(kt.prop.name, name);
        assert_eq!(kt.prop.updateJson, Some("https://example.com/update.json".to_string()));
    }

    #[test]
    fn test_serialization() {
        let mut name = BTreeMap::new();
        name.insert("en".to_string(), "Example Module".to_string());
        let mut description = BTreeMap::new();
        description.insert("en".to_string(), "Description".to_string());
        let kt = KamToml::new(
            "example".to_string(),
            name,
            "1.0.0".to_string(),
            123,
            "Author".to_string(),
            description,
            None,
        );
        let toml_str = kt.to_toml().unwrap();
        let deserialized: KamToml = KamToml::from_toml(&toml_str).unwrap();
        assert_eq!(kt.prop.id, deserialized.prop.id);
    }

    #[test]
    fn test_write_to_dir() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join("kam_test_write");
        fs::create_dir_all(&temp_dir).unwrap();

        let kt = KamToml::default();
        kt.write_to_dir(&temp_dir).unwrap();

        let file_path = temp_dir.join("kam.toml");
        assert!(file_path.exists());
        assert!(fs::read_to_string(file_path).unwrap().contains("[prop]"));

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_load_from_dir() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join("kam_test_load");
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a sample kam.toml
        let sample_toml = r#"
[prop]
id = "test_module"
name = { en = "Test Module" }
version = "1.0.0"
versionCode = 100
author = "Test Author"
description = { en = "Test Description" }

[mmrl]
zip_url = "https://example.com/module.zip"
changelog = "https://example.com/changelog.md"
license = "LICENSE"
readme = "README.md"

[kam]
min_api = 29
supported_arch = ["arm64-v8a"]
module_type = "Normal"
"#;
        fs::write(temp_dir.join("kam.toml"), sample_toml).unwrap();

        // Create LICENSE and README.md files
        fs::write(temp_dir.join("LICENSE"), "MIT License").unwrap();
        fs::write(temp_dir.join("README.md"), "# README").unwrap();

        let loaded = KamToml::load_from_dir(&temp_dir).unwrap();
        assert_eq!(loaded.prop.id, "test_module");
        assert_eq!(loaded.prop.version, "1.0.0");
        assert_eq!(loaded.prop.get_name(), "Test Module");
        assert_eq!(loaded.prop.get_description(), "Test Description");

        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_load_from_dir_missing_file() {
        let temp_dir = std::env::temp_dir().join("kam_test_missing");
        let result = KamToml::load_from_dir(&temp_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_load_from_dir_invalid_version() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join("kam_test_invalid");
        fs::create_dir_all(&temp_dir).unwrap();

        let invalid_toml = r#"
[prop]
id = "test"
name = { en = "Test" }
version = "1.0"
versionCode = 1
author = "Author"
description = { en = "Desc" }

[mmrl]
zip_url = "https://example.com/module.zip"
changelog = "https://example.com/changelog.md"

[kam]
min_api = 29
module_type = "Normal"
"#;
        fs::write(temp_dir.join("kam.toml"), invalid_toml).unwrap();

        let result = KamToml::load_from_dir(&temp_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be in format"));
        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_dependency_resolution_simple() {
        let mut name = BTreeMap::new();
        name.insert("en".to_string(), "Test Module".to_string());
        let mut description = BTreeMap::new();
        description.insert("en".to_string(), "Test Description".to_string());

        let mut kt = KamToml::new(
            "test".to_string(),
            name,
            "1.0.0".to_string(),
            1,
            "Author".to_string(),
            description,
            None,
        );

        // Add dependencies
        let dep1 = Dependency {
            id: "dep1".to_string(),
            version: Some("1.0.0".to_string()),
            source: None,
        };
        let dep2 = Dependency {
            id: "dep2".to_string(),
            version: Some("2.0.0".to_string()),
            source: None,
        };

        kt.kam.dependency = Some(DependencySection {
            normal: Some(vec![dep1, dep2]),
            dev: None,
        });

        let resolved = kt.resolve_dependencies().unwrap();
        let normal_group = resolved.get("normal").unwrap();
        assert_eq!(normal_group.dependencies.len(), 2);
        assert_eq!(normal_group.dependencies[0].id, "dep1");
        assert_eq!(normal_group.dependencies[1].id, "dep2");
    }

    #[test]
    fn test_dependency_resolution_with_includes() {
        let mut name = BTreeMap::new();
        name.insert("en".to_string(), "Test Module".to_string());
        let mut description = BTreeMap::new();
        description.insert("en".to_string(), "Test Description".to_string());

        let mut kt = KamToml::new(
            "test".to_string(),
            name,
            "1.0.0".to_string(),
            1,
            "Author".to_string(),
            description,
            None,
        );

        // Add dependencies with includes
        let normal_dep = Dependency {
            id: "normal-dep".to_string(),
            version: Some("1.0.0".to_string()),
            source: None,
        };
        let dev_include = Dependency {
            id: "include:normal".to_string(),
            version: None,
            source: None,
        };
        let dev_dep = Dependency {
            id: "dev-dep".to_string(),
            version: Some("2.0.0".to_string()),
            source: None,
        };

        kt.kam.dependency = Some(DependencySection {
            normal: Some(vec![normal_dep]),
            dev: Some(vec![dev_include, dev_dep]),
        });

        let resolved = kt.resolve_dependencies().unwrap();
        let dev_group = resolved.get("dev").unwrap();
        assert_eq!(dev_group.dependencies.len(), 2);
        assert_eq!(dev_group.dependencies[0].id, "normal-dep");
        assert_eq!(dev_group.dependencies[1].id, "dev-dep");
    }

    #[test]
    fn test_dependency_validation() {
        let mut name = BTreeMap::new();
        name.insert("en".to_string(), "Test Module".to_string());
        let mut description = BTreeMap::new();
        description.insert("en".to_string(), "Test Description".to_string());

        let mut kt = KamToml::new(
            "test".to_string(),
            name,
            "1.0.0".to_string(),
            1,
            "Author".to_string(),
            description,
            None,
        );

        // Add dependency with invalid include
        let invalid_include = Dependency {
            id: "include:nonexistent".to_string(),
            version: None,
            source: None,
        };

        kt.kam.dependency = Some(DependencySection {
            normal: Some(vec![invalid_include]),
            dev: None,
        });

        let result = kt.validate_dependencies();
        assert!(result.is_err());
    }

    #[test]
    fn test_dependency_cycle_detection() {
        use std::collections::BTreeMap;

        // Create a resolver directly to test cycle detection
        let mut groups = BTreeMap::new();

        let cycle_a = vec![Dependency {
            id: "include:group-b".to_string(),
            version: None,
            source: None,
        }];
        let cycle_b = vec![Dependency {
            id: "include:group-a".to_string(),
            version: None,
            source: None,
        }];

        groups.insert("group-a".to_string(), cycle_a);
        groups.insert("group-b".to_string(), cycle_b);

        let resolver = crate::dependency_resolver::DependencyResolver::from_groups(groups);
        let result = resolver.resolve();

        assert!(result.is_err());
    }
}
