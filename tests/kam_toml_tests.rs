/// KamToml 配置测试模块
/// 测试 kam.toml 的加载、保存和操作
use kam::types::kam_toml::KamToml;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_kam_toml_default() {
    // 测试默认 KamToml 创建
    let kt = KamToml::default();
    assert!(!kt.prop.id.is_empty());
    assert!(!kt.prop.name.is_empty());
    assert!(!kt.prop.version.is_empty());
}

#[test]
fn test_kam_toml_from_prop() {
    // 测试从 PropSection 创建 KamToml
    use kam::types::kam_toml::sections::prop::PropSection;

    let prop = PropSection {
        id: "test_module".to_string(),
        name: "Test Module".to_string(),
        version: "1.0.0".to_string(),
        versionCode: 1,
        author: Some("Test Author".to_string()),
        description: "Test Description".to_string(),
        updateJson: None,
        metamodule: false,
    };

    let kt = KamToml::from_prop(prop);
    assert_eq!(kt.prop.id, "test_module");
    assert_eq!(kt.prop.name, "Test Module");
    assert_eq!(kt.prop.version, "1.0.0");
}

#[test]
fn test_kam_toml_new_with_timestamp() {
    // 测试使用时间戳创建 KamToml
    let kt = KamToml::new_with_current_timestamp(
        "test_id".to_string(),
        "Test Name".to_string(),
        "1.0.0".to_string(),
        Some("Author".to_string()),
        "Description".to_string(),
        None,
        None,
    );

    assert_eq!(kt.prop.id, "test_id");
    assert_eq!(kt.prop.name, "Test Name");
    assert!(kt.prop.versionCode > 0);
}

#[test]
fn test_kam_toml_load_and_save() {
    // 测试加载和保存 kam.toml
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path();

    // 创建初始配置
    let kt = KamToml::new_with_current_timestamp(
        "test_module".to_string(),
        "Test Module".to_string(),
        "1.0.0".to_string(),
        Some("Test Author".to_string()),
        "Test Description".to_string(),
        None,
        None,
    );

    // 保存
    kt.write_to_dir(project_dir).unwrap();

    // 验证文件存在
    let toml_path = project_dir.join("kam.toml");
    assert!(toml_path.exists());

    // 加载
    let loaded = KamToml::load_from_dir(project_dir).unwrap();
    assert_eq!(loaded.prop.id, "test_module");
    assert_eq!(loaded.prop.name, "Test Module");
    assert_eq!(loaded.prop.version, "1.0.0");
}

#[test]
fn test_kam_toml_load_from_file() {
    // 测试从文件加载
    let temp_dir = TempDir::new().unwrap();
    let toml_path = temp_dir.path().join("kam.toml");

    let content = r#"
[prop]
id = "test_module"
name = "Test Module"
version = "1.0.0"
versionCode = 1
author = "Test Author"
description = "Test Description"
updateJson = ""
metamodule = false

[kam]
module_type = "kam"
"#;

    fs::write(&toml_path, content).unwrap();

    let kt = KamToml::load_from_file(&toml_path).unwrap();
    assert_eq!(kt.prop.id, "test_module");
    assert_eq!(kt.prop.name, "Test Module");
}

#[test]
fn test_kam_toml_apply_vars() {
    // 测试应用模板变量
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path();

    let mut kt = KamToml::new_with_current_timestamp(
        "test_module".to_string(),
        "Test Module".to_string(),
        "1.0.0".to_string(),
        Some("Test Author".to_string()),
        "Test Description".to_string(),
        None,
        None,
    );

    kt.write_to_dir(project_dir).unwrap();

    // 应用变量
    let vars = vec![
        ("prop.name".to_string(), "Updated Name".to_string()),
        ("prop.version".to_string(), "2.0.0".to_string()),
    ];

    kt.apply_vars(vars).unwrap();

    assert_eq!(kt.prop.name, "Updated Name");
    assert_eq!(kt.prop.version, "2.0.0");
}

#[test]
fn test_kam_toml_load_not_found() {
    // 测试加载不存在的文件
    let result = KamToml::load_from_file("/nonexistent/path/kam.toml");
    assert!(result.is_err());
}
