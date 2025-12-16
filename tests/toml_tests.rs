/// TOML 操作测试模块
/// 测试 kam toml 命令的功能
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn create_test_kam_toml(dir: &Path) -> PathBuf {
    let toml_path = dir.join("kam.toml");
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
module_type = "Kam"

[mmrl.repo]
repository = "https://github.com/test/test_module"
changelog = "https://github.com/test/test_module/CHANGELOG.md"
"#;
    fs::write(&toml_path, content).unwrap();
    toml_path
}

#[test]
fn test_toml_get_value() {
    // 测试获取 TOML 值
    let temp_dir = TempDir::new().unwrap();
    let toml_path = create_test_kam_toml(temp_dir.path());

    // 读取 TOML
    let content = fs::read_to_string(&toml_path).unwrap();
    let value: toml::Value = toml::from_str(&content).unwrap();

    // 测试获取嵌套值
    assert_eq!(value["prop"]["id"].as_str().unwrap(), "test_module");
    assert_eq!(value["prop"]["version"].as_str().unwrap(), "1.0.0");
    assert_eq!(
        value["mmrl"]["repo"]["repository"].as_str().unwrap(),
        "https://github.com/test/test_module"
    );
}

#[test]
fn test_toml_set_value() {
    // 测试设置 TOML 值
    let temp_dir = TempDir::new().unwrap();
    let toml_path = create_test_kam_toml(temp_dir.path());

    let mut content = fs::read_to_string(&toml_path).unwrap();
    let mut value: toml::Value = toml::from_str(&content).unwrap();

    // 设置字符串值
    value["prop"]["name"] = toml::Value::String("Updated Name".to_string());

    // 设置整数值
    value["prop"]["versionCode"] = toml::Value::Integer(2);

    // 写回文件
    content = toml::to_string_pretty(&value).unwrap();
    fs::write(&toml_path, content).unwrap();

    // 验证
    let content = fs::read_to_string(&toml_path).unwrap();
    let value: toml::Value = toml::from_str(&content).unwrap();
    assert_eq!(value["prop"]["name"].as_str().unwrap(), "Updated Name");
    assert_eq!(value["prop"]["versionCode"].as_integer().unwrap(), 2);
}

#[test]
fn test_toml_path_traversal() {
    // 测试路径遍历（点分隔的键）
    let temp_dir = TempDir::new().unwrap();
    let toml_path = create_test_kam_toml(temp_dir.path());

    let content = fs::read_to_string(&toml_path).unwrap();
    let value: toml::Value = toml::from_str(&content).unwrap();

    // 模拟路径遍历函数
    fn get_value_by_path(value: &toml::Value, path: &str) -> Option<toml::Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;
        for (i, &part) in parts.iter().enumerate() {
            if let Some(tbl) = current.as_table() {
                if let Some(next) = tbl.get(part) {
                    current = next;
                    if i == parts.len() - 1 {
                        return Some(current.clone());
                    }
                    continue;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        None
    }

    assert!(get_value_by_path(&value, "prop.id").is_some());
    assert!(get_value_by_path(&value, "prop.version").is_some());
    assert!(get_value_by_path(&value, "mmrl.repo.repository").is_some());
    assert!(get_value_by_path(&value, "nonexistent.key").is_none());
}

#[test]
fn test_toml_set_nested_path() {
    // 测试设置嵌套路径
    let temp_dir = TempDir::new().unwrap();
    let toml_path = create_test_kam_toml(temp_dir.path());

    let mut content = fs::read_to_string(&toml_path).unwrap();
    let mut value: toml::Value = toml::from_str(&content).unwrap();

    // 模拟设置嵌套路径的函数
    fn set_value_by_path(value: &mut toml::Value, path: &str, new_value: &str) {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value.as_table_mut().unwrap();
        for (i, &part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                current.insert(part.to_string(), toml::Value::String(new_value.to_string()));
                return;
            }
            if !current.contains_key(part) {
                current.insert(part.to_string(), toml::Value::Table(Default::default()));
            }
            current = current[part].as_table_mut().unwrap();
        }
    }

    set_value_by_path(&mut value, "prop.name", "New Name");
    set_value_by_path(&mut value, "new.section.key", "new value");

    content = toml::to_string_pretty(&value).unwrap();
    fs::write(&toml_path, content).unwrap();

    // 验证
    let content = fs::read_to_string(&toml_path).unwrap();
    let value: toml::Value = toml::from_str(&content).unwrap();
    assert_eq!(value["prop"]["name"].as_str().unwrap(), "New Name");
    assert_eq!(
        value["new"]["section"]["key"].as_str().unwrap(),
        "new value"
    );
}

#[test]
fn test_toml_unset_value() {
    // 测试删除 TOML 值
    let temp_dir = TempDir::new().unwrap();
    let toml_path = create_test_kam_toml(temp_dir.path());

    let mut content = fs::read_to_string(&toml_path).unwrap();
    let mut value: toml::Value = toml::from_str(&content).unwrap();

    // 删除一个键
    if let Some(table) = value.as_table_mut()
        && let Some(prop) = table.get_mut("prop")
        && let Some(prop_table) = prop.as_table_mut()
    {
        prop_table.remove("updateJson");
    }

    content = toml::to_string_pretty(&value).unwrap();
    fs::write(&toml_path, content).unwrap();

    // 验证
    let content = fs::read_to_string(&toml_path).unwrap();
    let value: toml::Value = toml::from_str(&content).unwrap();
    assert!(
        value
            .get("prop")
            .and_then(|p| p.get("updateJson"))
            .is_none()
    );
}
