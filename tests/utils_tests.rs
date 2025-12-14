/// 工具函数测试模块
/// 测试 utils.rs 中的各种工具函数
use kam::utils;

#[test]
fn test_pattern_matches_directory_prefix() {
    // 测试目录前缀匹配
    assert!(utils::pattern_matches("foo/", "foo", None));
    assert!(utils::pattern_matches("foo/", "foo/bar", None));
    assert!(utils::pattern_matches("foo/", "./foo/bar", None));
    assert!(!utils::pattern_matches("foo/", "foobar", None));
    assert!(!utils::pattern_matches(".git/", ".github", None));
}

#[test]
fn test_pattern_matches_suffix() {
    // 测试后缀匹配
    assert!(utils::pattern_matches(
        "*.rs",
        "src/main.rs",
        Some("main.rs")
    ));
    assert!(utils::pattern_matches(
        "*.toml",
        "kam.toml",
        Some("kam.toml")
    ));
    assert!(!utils::pattern_matches(
        "*.rs",
        "src/main.txt",
        Some("main.txt")
    ));
}

#[test]
fn test_pattern_matches_glob() {
    // 测试通配符匹配
    assert!(utils::pattern_matches(
        "*.rs",
        "src/main.rs",
        Some("main.rs")
    ));
    assert!(utils::pattern_matches(
        "test_*.rs",
        "test_utils.rs",
        Some("test_utils.rs")
    ));
    assert!(utils::pattern_matches("src/*.rs", "src/main.rs", None));
}

#[test]
fn test_pattern_matches_exact() {
    // 测试精确匹配
    assert!(utils::pattern_matches(
        "kam.toml",
        "kam.toml",
        Some("kam.toml")
    ));
    assert!(utils::pattern_matches(
        "README.md",
        "README.md",
        Some("README.md")
    ));
    assert!(!utils::pattern_matches(
        "kam.toml",
        "config.toml",
        Some("config.toml")
    ));
}

#[test]
fn test_normalize_env_key() {
    // 测试环境变量键规范化
    assert_eq!(utils::Utils::normalize_env_key("prop.id"), "PROP_ID");
    assert_eq!(utils::Utils::normalize_env_key("mmrl.repo"), "MMRL_REPO");
    assert_eq!(utils::Utils::normalize_env_key("kam.build"), "KAM_BUILD");
    assert_eq!(utils::Utils::normalize_env_key("test-key"), "TEST_KEY");
}

#[test]
fn test_kam_env_var() {
    // 测试 KAM_ 环境变量生成
    assert_eq!(utils::Utils::kam_env_var("prop.id"), "KAM_PROP_ID");
    assert_eq!(utils::Utils::kam_env_var("mmrl.repo"), "KAM_MMRL_REPO");
    assert_eq!(utils::Utils::kam_env_var("version"), "KAM_VERSION");
}

#[test]
fn test_default_exclude_dir_names() {
    // 测试默认排除目录名称
    let dirs = utils::default_exclude_dir_names();
    assert!(dirs.contains(&"dist".to_string()));
    assert!(dirs.contains(&"templates".to_string()));
    assert!(dirs.contains(&"tmpl".to_string()));
}

#[test]
fn test_compute_index_path() {
    // 测试索引路径计算
    use std::path::PathBuf;

    let base = PathBuf::from("/tmp/index");

    // 单字符模块名
    let path1 = utils::compute_index_path(&base, "a");
    assert!(path1.to_string_lossy().contains("1/a"));

    // 两字符模块名
    let path2 = utils::compute_index_path(&base, "ab");
    assert!(path2.to_string_lossy().contains("2/ab"));

    // 三字符模块名
    let path3 = utils::compute_index_path(&base, "abc");
    assert!(path3.to_string_lossy().contains("3/a/abc"));

    // 长模块名
    let path4 = utils::compute_index_path(&base, "my_module");
    assert!(path4.to_string_lossy().contains("my_module"));
}
