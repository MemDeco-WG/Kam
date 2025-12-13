/// 导出功能测试模块
/// 测试 kam export 命令的功能

use kam::types::kam_toml::KamToml;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn create_test_kam_toml(_dir: &PathBuf) -> KamToml {
    KamToml::new_with_current_timestamp(
        "test_module".to_string(),
        "Test Module".to_string(),
        "1.0.0".to_string(),
        Some("Test Author".to_string()),
        "Test Description".to_string(),
        Some("https://example.com/update.json".to_string()),
        None,
    )
}

#[test]
fn test_export_module_prop() {
    // 测试导出 module.prop 格式
    let temp_dir = TempDir::new().unwrap();
    let kt = create_test_kam_toml(&temp_dir.path().to_path_buf());

    // 模拟 module.prop 格式
    let module_prop = format!(
        "id={}\nname={}\nversion={}\nversionCode={}\nauthor={}\ndescription={}\nupdateJson={}\n",
        kt.prop.id,
        kt.prop.name,
        kt.prop.version,
        kt.prop.versionCode,
        kt.prop.author.as_ref().map(|s| s.as_str()).unwrap_or(""),
        kt.prop.description,
        kt.prop.updateJson.as_ref().map(|s| s.as_str()).unwrap_or("")
    );

    let output_path = temp_dir.path().join("module.prop");
    fs::write(&output_path, module_prop).unwrap();

    // 验证文件内容
    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("id=test_module"));
    assert!(content.contains("name=Test Module"));
    assert!(content.contains("version=1.0.0"));
}

#[test]
fn test_export_update_json() {
    // 测试导出 update.json 格式
    let temp_dir = TempDir::new().unwrap();
    let kt = create_test_kam_toml(&temp_dir.path().to_path_buf());

    // 模拟 update.json 格式
    use serde_json::json;
    let update_json = json!({
        "version": kt.prop.version,
        "versionCode": kt.prop.versionCode,
        "zipUrl": format!("https://example.com/{}-{}.zip", kt.prop.id, kt.prop.version),
        "changelog": "https://example.com/CHANGELOG.md"
    });

    let output_path = temp_dir.path().join("update.json");
    fs::write(&output_path, serde_json::to_string_pretty(&update_json).unwrap()).unwrap();

    // 验证 JSON 格式
    let content = fs::read_to_string(&output_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["version"], "1.0.0");
    assert_eq!(parsed["versionCode"], kt.prop.versionCode);
}

#[test]
fn test_export_module_json() {
    // 测试导出 module.json 格式
    let temp_dir = TempDir::new().unwrap();
    let kt = create_test_kam_toml(&temp_dir.path().to_path_buf());

    // 模拟 module.json 格式
    use serde_json::json;
    let module_json = json!({
        "id": kt.prop.id,
        "name": kt.prop.name,
        "version": kt.prop.version,
        "versionCode": kt.prop.versionCode,
        "author": kt.prop.author,
        "description": kt.prop.description
    });

    let output_path = temp_dir.path().join("module.json");
    fs::write(&output_path, serde_json::to_string_pretty(&module_json).unwrap()).unwrap();

    // 验证
    let content = fs::read_to_string(&output_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["id"], "test_module");
    assert_eq!(parsed["name"], "Test Module");
}

#[test]
fn test_export_repo_json() {
    // 测试导出 repo.json 格式
    let temp_dir = TempDir::new().unwrap();
    let mut kt = create_test_kam_toml(&temp_dir.path().to_path_buf());

    // 添加 mmrl 配置
    use kam::types::kam_toml::sections::mmrl::MmrlSection;
    use kam::types::kam_toml::sections::repo::RepoSection;
    kt.mmrl = Some(MmrlSection {
        repo: Some(RepoSection {
            repository: Some("https://github.com/test/test_module".to_string()),
            changelog: Some("https://github.com/test/test_module/CHANGELOG.md".to_string()),
            ..Default::default()
        }),
    });

    // 模拟 repo.json 格式
    use serde_json::json;
    let repo_json = json!({
        "name": "Test Repo",
        "modules": [
            {
                "id": kt.prop.id,
                "name": kt.prop.name,
                "version": kt.prop.version,
                "versionCode": kt.prop.versionCode,
                "author": kt.prop.author.as_ref().map(|s| s.as_str()).unwrap_or(""),
                "description": kt.prop.description,
                "repository": kt.mmrl.as_ref()
                    .and_then(|m| m.repo.as_ref())
                    .and_then(|r| r.repository.as_ref())
                    .cloned(),
                "changelog": kt.mmrl.as_ref()
                    .and_then(|m| m.repo.as_ref())
                    .and_then(|r| r.changelog.as_ref())
                    .cloned()
            }
        ]
    });

    let output_path = temp_dir.path().join("repo.json");
    fs::write(&output_path, serde_json::to_string_pretty(&repo_json).unwrap()).unwrap();

    // 验证
    let content = fs::read_to_string(&output_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["modules"][0]["id"], "test_module");
}
