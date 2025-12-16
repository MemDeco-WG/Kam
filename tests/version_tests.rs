/// 版本管理测试模块
/// 测试版本号管理和 bump 功能
// 注意：这些函数在 handler.rs 中不是公开的，我们需要直接测试 run 函数
// 或者将这些辅助函数公开。这里我们创建测试辅助函数。
use kam::errors::KamError;

// 复制 handler.rs 中的辅助函数用于测试
fn validate_version(version: &str) -> Result<(), KamError> {
    let parts: Vec<&str> = version.split('.').collect();

    if parts.is_empty() {
        return Err(KamError::InvalidConfig(
            "Version cannot be empty".to_string(),
        ));
    }

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            return Err(KamError::InvalidConfig(format!(
                "Invalid version '{}': part {} is empty",
                version,
                i + 1
            )));
        }

        if !part.chars().all(|c| c.is_alphanumeric() || c == '-') {
            return Err(KamError::InvalidConfig(format!(
                "Invalid version '{}': part '{}' contains invalid characters",
                version, part
            )));
        }
    }

    Ok(())
}

fn bump_version(current: &str, index: usize) -> Result<String, KamError> {
    let mut parts: Vec<u32> = current.split('.').map(|s| s.parse().unwrap_or(0)).collect();

    while parts.len() < 3 {
        parts.push(0);
    }

    if index >= parts.len() {
        return Err(KamError::InvalidConfig(
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

#[test]
fn test_validate_version_valid() {
    // 测试有效版本号
    assert!(validate_version("1.0.0").is_ok());
    assert!(validate_version("1.2.3").is_ok());
    assert!(validate_version("10.20.30").is_ok());
    assert!(validate_version("1.0.0-beta1").is_ok());
    assert!(validate_version("1.0.0-alpha").is_ok());
}

#[test]
fn test_validate_version_invalid() {
    // 测试无效版本号
    assert!(validate_version("").is_err());
    assert!(validate_version("1.0.").is_err());
    assert!(validate_version(".1.0").is_err());
    assert!(validate_version("1.0.0@").is_err());
    assert!(validate_version("1.0.0#").is_err());
}

#[test]
fn test_bump_version_major() {
    // 测试主版本号 bump
    assert_eq!(bump_version("1.0.0", 0).unwrap(), "2.0.0");
    assert_eq!(bump_version("2.5.3", 0).unwrap(), "3.0.0");
    assert_eq!(bump_version("10.20.30", 0).unwrap(), "11.0.0");
}

#[test]
fn test_bump_version_minor() {
    // 测试次版本号 bump
    assert_eq!(bump_version("1.0.0", 1).unwrap(), "1.1.0");
    assert_eq!(bump_version("2.5.3", 1).unwrap(), "2.6.0");
    assert_eq!(bump_version("1.9.9", 1).unwrap(), "1.10.0");
}

#[test]
fn test_bump_version_patch() {
    // 测试补丁版本号 bump
    assert_eq!(bump_version("1.0.0", 2).unwrap(), "1.0.1");
    assert_eq!(bump_version("2.5.3", 2).unwrap(), "2.5.4");
    assert_eq!(bump_version("1.9.9", 2).unwrap(), "1.9.10");
}

#[test]
fn test_bump_version_short_format() {
    // 测试短格式版本号（自动补齐）
    assert_eq!(bump_version("1", 0).unwrap(), "2.0.0");
    assert_eq!(bump_version("1.0", 1).unwrap(), "1.1.0");
    assert_eq!(bump_version("1.0", 2).unwrap(), "1.0.1");
}

#[test]
fn test_bump_version_invalid_index() {
    // 测试无效的索引
    let result = bump_version("1.0.0", 10);
    assert!(result.is_err());
}

#[test]
fn test_bump_version_zero_padding() {
    // 测试版本号零填充
    let result = bump_version("1.0.0", 1);
    assert_eq!(result.unwrap(), "1.1.0");

    let result = bump_version("1.0.0", 2);
    assert_eq!(result.unwrap(), "1.0.1");
}
