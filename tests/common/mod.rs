/// 测试公共模块
/// 提供测试辅助函数和工具

use kam::types::kam_toml::KamToml;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// 创建测试用的临时 KamToml
pub fn create_test_kam_toml() -> KamToml {
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

/// 创建测试用的 kam.toml 文件
pub fn create_test_kam_toml_file(dir: &PathBuf) -> PathBuf {
    let kt = create_test_kam_toml();
    kt.write_to_dir(dir).unwrap();
    dir.join("kam.toml")
}

/// 创建临时测试目录
pub fn create_test_dir() -> TempDir {
    TempDir::new().unwrap()
}
