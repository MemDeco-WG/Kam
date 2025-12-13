use crate::errors::KamError;
use crate::types::kam_toml::KamToml;

use super::args::VersionArgs;

// 处理版本命令
// 可以显示当前版本，也可以bump版本号
pub fn run(args: VersionArgs) -> Result<(), KamError> {
    let current_dir = std::env::current_dir()?;
    let mut kam_toml = KamToml::load_from_dir(&current_dir)?;

    if let Some(v) = args.version {
        // 有版本参数，就更新版本
        let new_version = match v.as_str() {
            "major" => bump_version(&kam_toml.prop.version, 0)?,  // 主版本号+1
            "minor" => bump_version(&kam_toml.prop.version, 1)?,  // 次版本号+1
            "patch" => bump_version(&kam_toml.prop.version, 2)?,  // 补丁版本号+1
            _ => {
                // 自定义版本号，先验证格式
                validate_version(&v)?;
                v
            }
        };

        if new_version != kam_toml.prop.version {
            let msg = trf!(
                "Bumped version: {} -> {}",
                &kam_toml.prop.version,
                &new_version
            );
            println!("{}", msg);
            kam_toml.prop.version = new_version;
        } else {
            println!("{}", trf!("Version unchanged: {}", &kam_toml.prop.version));
        }

        // 更新版本时总是更新versionCode（用当前时间戳）
        // 这样每次发布都有唯一的versionCode
        let old_code = kam_toml.prop.versionCode;
        let new_code = chrono::Utc::now().timestamp_millis();
        println!("{}", trf!(
            "Updated versionCode: {} -> {}",
            &old_code.to_string(),
            &new_code.to_string()
        ));
        kam_toml.prop.versionCode = new_code;

        // 写回文件
        kam_toml.write_to_dir(&current_dir)?;
    } else {
        // 没有版本参数，就显示当前版本
        println!("{}", trf!("Current version: {}", &kam_toml.prop.version));
        println!("{}", trf!("Current versionCode: {}", &kam_toml.prop.versionCode.to_string()));
    }

    Ok(())
}

// 验证版本号格式
// 支持语义化版本（如1.0.0）和预发布版本（如1.0.0-beta1）
fn validate_version(version: &str) -> Result<(), KamError> {
    let parts: Vec<&str> = version.split('.').collect();

    if parts.is_empty() {
        return Err(KamError::InvalidConfig(
            crate::i18n::tr_key("Version cannot be empty").to_string(),
        ));
    }

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            return Err(KamError::InvalidConfig(trf!(
                "Invalid version '{}': part {} is empty",
                version,
                i + 1
            )));
        }

        // 每个部分应该是数字或字母数字（支持预发布版本如1.0.0-beta1）
        // 允许字母数字和横线
        if !part.chars().all(|c| c.is_alphanumeric() || c == '-') {
            return Err(KamError::InvalidConfig(trf!(
                "Invalid version '{}': part '{}' contains invalid characters",
                version, part
            )));
        }
    }

    Ok(())
}

// Bump版本号
// index: 0=major, 1=minor, 2=patch
// bump后会重置后面的版本号（比如1.2.3 bump minor变成1.3.0）
fn bump_version(current: &str, index: usize) -> Result<String, KamError> {
    // 解析版本号，解析失败就当0处理
    let mut parts: Vec<u32> = current.split('.').map(|s| s.parse().unwrap_or(0)).collect();

    // 确保至少有3部分（语义化版本）
    while parts.len() < 3 {
        parts.push(0);
    }

    if index >= parts.len() {
        return Err(KamError::InvalidConfig(
            crate::i18n::tr_key("Invalid version format for bumping").to_string(),
        ));
    }

    // 指定的部分+1，后面的部分重置为0
    parts[index] += 1;
    for i in index + 1..parts.len() {
        parts[i] = 0;
    }

    // 重新组合成字符串
    Ok(parts
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join("."))
}
