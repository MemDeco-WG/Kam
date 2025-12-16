/// 集成测试模块
/// 测试多个功能模块的协同工作
use kam::types::kam_toml::KamToml;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_full_workflow() {
    // 测试完整工作流程：创建 -> 修改 -> 导出
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path();

    // 1. 创建新项目
    let kt = KamToml::new_with_current_timestamp(
        "my_module".to_string(),
        "My Module".to_string(),
        "1.0.0".to_string(),
        Some("Author".to_string()),
        "Description".to_string(),
        None,
        None,
    );

    // 2. 保存配置
    kt.write_to_dir(project_dir).unwrap();
    assert!(project_dir.join("kam.toml").exists());

    // 3. 加载配置
    let loaded = KamToml::load_from_dir(project_dir).unwrap();
    assert_eq!(loaded.prop.id, "my_module");

    // 4. 修改版本
    let mut kt = loaded;
    kt.prop.version = "1.1.0".to_string();
    kt.prop.versionCode = chrono::Utc::now().timestamp_millis();
    kt.write_to_dir(project_dir).unwrap();

    // 5. 验证修改
    let updated = KamToml::load_from_dir(project_dir).unwrap();
    assert_eq!(updated.prop.version, "1.1.0");
}

#[test]
fn test_template_variables_workflow() {
    // 测试模板变量应用工作流
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path();

    let mut kt = KamToml::new_with_current_timestamp(
        "template_test".to_string(),
        "Template Test".to_string(),
        "1.0.0".to_string(),
        Some("Author".to_string()),
        "Description".to_string(),
        None,
        None,
    );

    kt.write_to_dir(project_dir).unwrap();

    // 应用模板变量
    let vars = vec![
        ("prop.name".to_string(), "Custom Name".to_string()),
        ("prop.version".to_string(), "2.0.0".to_string()),
        ("prop.author".to_string(), "Custom Author".to_string()),
    ];

    kt.apply_vars(vars).unwrap();
    kt.write_to_dir(project_dir).unwrap();

    // 验证
    let loaded = KamToml::load_from_dir(project_dir).unwrap();
    assert_eq!(loaded.prop.name, "Custom Name");
    assert_eq!(loaded.prop.version, "2.0.0");
    assert_eq!(loaded.prop.author, Some("Custom Author".to_string()));
}

#[test]
fn test_version_bump_workflow() {
    // 测试版本 bump 工作流
    // 使用本地定义的 bump_version 函数
    fn bump_version(current: &str, index: usize) -> Result<String, kam::errors::KamError> {
        let mut parts: Vec<u32> = current.split('.').map(|s| s.parse().unwrap_or(0)).collect();
        while parts.len() < 3 {
            parts.push(0);
        }
        if index >= parts.len() {
            return Err(kam::errors::KamError::InvalidConfig(
                "Invalid version format for bumping".to_string(),
            ));
        }
        parts[index] += 1;
        for part in parts.iter_mut().skip(index + 1) {
            *part = 0;
        }
        Ok(parts
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join("."))
    }

    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path();

    let mut kt = KamToml::new_with_current_timestamp(
        "version_test".to_string(),
        "Version Test".to_string(),
        "1.0.0".to_string(),
        Some("Author".to_string()),
        "Description".to_string(),
        None,
        None,
    );

    kt.write_to_dir(project_dir).unwrap();

    // Bump patch version
    let new_version = bump_version(&kt.prop.version, 2).unwrap();
    assert_eq!(new_version, "1.0.1");

    kt.prop.version = new_version;
    kt.prop.versionCode = chrono::Utc::now().timestamp_millis();
    kt.write_to_dir(project_dir).unwrap();

    // 验证
    let loaded = KamToml::load_from_dir(project_dir).unwrap();
    assert_eq!(loaded.prop.version, "1.0.1");
}

#[test]
fn test_config_export_workflow() {
    // 测试配置导出工作流
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path();

    let kt = KamToml::new_with_current_timestamp(
        "export_test".to_string(),
        "Export Test".to_string(),
        "1.0.0".to_string(),
        Some("Author".to_string()),
        "Description".to_string(),
        Some("https://example.com/update.json".to_string()),
        None,
    );

    kt.write_to_dir(project_dir).unwrap();

    // 导出 module.prop
    let module_prop = format!(
        "id={}\nname={}\nversion={}\nversionCode={}\nauthor={}\ndescription={}\nupdateJson={}\n",
        kt.prop.id,
        kt.prop.name,
        kt.prop.version,
        kt.prop.versionCode,
        kt.prop.author.as_deref().unwrap_or(""),
        kt.prop.description,
        kt.prop.updateJson.as_deref().unwrap_or("")
    );

    let prop_path = project_dir.join("module.prop");
    fs::write(&prop_path, module_prop).unwrap();

    // 验证导出文件
    assert!(prop_path.exists());
    let content = fs::read_to_string(&prop_path).unwrap();
    assert!(content.contains("id=export_test"));
    assert!(content.contains("version=1.0.0"));
}
