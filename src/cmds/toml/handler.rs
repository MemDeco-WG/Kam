use crate::errors::KamError;
use std::fs;
use std::path::PathBuf;

use super::args::TomlArgs;

// 查找kam.toml文件路径
// 如果指定了文件就用指定的，否则向上查找，找不到就用当前目录的
fn find_kam_toml_path(file: &Option<String>) -> Result<PathBuf, KamError> {
    if let Some(path) = file {
        return Ok(PathBuf::from(path));
    }
    // 向上查找kam.toml（支持在子目录里运行命令）
    let mut cwd = std::env::current_dir().map_err(KamError::Io)?;
    loop {
        let candidate = cwd.join("kam.toml");
        if candidate.exists() {
            return Ok(candidate);
        }
        if !cwd.pop() {
            break; // 到根目录了，停止查找
        }
    }
    // 找不到就回退到当前目录的kam.toml（虽然可能不存在）
    Ok(std::env::current_dir()
        .map_err(KamError::Io)?
        .join("kam.toml"))
}

fn read_toml(path: &PathBuf) -> Result<toml::Value, KamError> {
    if !path.exists() {
        return Err(KamError::InvalidDirectory(format!(
            "Toml file not found: {}",
            path.display()
        )));
    }
    let s = fs::read_to_string(path).map_err(KamError::Io)?;
    let v: toml::Value = toml::from_str(&s)
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse toml: {}", e)))?;
    Ok(v)
}

fn write_toml(path: &PathBuf, v: &toml::Value) -> Result<(), KamError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }
    let s = toml::to_string_pretty(v)
        .map_err(|e| KamError::CommandFailed(format!("Failed to serialize toml: {}", e)))?;
    fs::write(path, s).map_err(KamError::Io)?;
    Ok(())
}

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

// 根据路径设置值（支持点号分隔的路径，如"prop.version"）
// 会尝试自动推断类型（整数、布尔值、字符串）
fn set_value_by_path(value: &mut toml::Value, path: &str, new_value: &str) {
    let v = value;
    if !v.is_table() {
        *v = toml::Value::Table(Default::default());
    }
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = v.as_table_mut().unwrap();
    for (i, &part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // 最后一部分，设置值
            // 尝试推断类型：如果是纯数字就转整数，true/false转布尔值
            if part == "versionCode" {
                // versionCode必须是整数
                if let Ok(num) = new_value.parse::<i64>() {
                    current.insert(part.to_string(), toml::Value::Integer(num));
                } else {
                    // 解析失败就当作字符串（虽然可能不太对）
                    current.insert(part.to_string(), toml::Value::String(new_value.to_string()));
                }
            } else if let Ok(num) = new_value.parse::<i64>() {
                // 纯数字，转整数
                current.insert(part.to_string(), toml::Value::Integer(num));
            } else if new_value == "true" || new_value == "false" {
                // 布尔值
                current.insert(part.to_string(), toml::Value::Boolean(new_value == "true"));
            } else {
                // 其他情况都当字符串
                current.insert(part.to_string(), toml::Value::String(new_value.to_string()));
            }
            return;
        }
        // 中间路径，确保表存在
        if !current.contains_key(part) {
            current.insert(part.to_string(), toml::Value::Table(Default::default()));
        }
        current = current[part].as_table_mut().unwrap();
    }
}

fn unset_value_by_path(value: &mut toml::Value, path: &str) -> bool {
    let parts: Vec<&str> = path.split('.').collect();
    if !value.is_table() {
        return false;
    }
    let mut current_tbl = value.as_table_mut().unwrap();
    for (i, &part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            return current_tbl.remove(part).is_some();
        } else if let Some(next) = current_tbl.get_mut(part) {
            if next.is_table() {
                current_tbl = next.as_table_mut().unwrap();
            } else {
                return false;
            }
        } else {
            return false;
        }
    }
    false
}

// 处理toml命令（get/set/unset/list）
pub fn run(args: TomlArgs) -> Result<(), KamError> {
    let path = find_kam_toml_path(&args.file)?;

    match args.command {
        crate::cmds::toml::args::TomlCommand::Get { key } => {
            // 获取值并打印
            let v = read_toml(&path)?;
            get_value_by_path(&v, &key).map_or_else(
                || {
                    Err(KamError::CommandFailed(format!(
                        "Key '{}' not found in {}",
                        key,
                        path.display()
                    )))
                },
                |val| {
                    println!("{}", val);
                    Ok(())
                },
            )
        }
        crate::cmds::toml::args::TomlCommand::Set { key, value } => {
            // 设置值，支持key=value格式（如果value是None）
            let (key, new_value) = if let Some(v) = value {
                (key, v)
            } else if key.contains('=') {
                // 支持key=value单参数格式
                let parts: Vec<&str> = key.splitn(2, '=').collect();
                (parts[0].to_string(), parts[1].to_string())
            } else {
                return Err(KamError::InvalidFilename("No value provided".to_string()));
            };
            let mut v = read_toml(&path).unwrap_or_else(|_| toml::Value::Table(Default::default()));
            // 如果key已存在，确保新值的类型兼容
            // 这样不会把整数字段改成字符串（虽然可能有点严格）
            if let Some(existing) = get_value_by_path(&v, &key) {
                match existing {
                    toml::Value::Integer(_) => {
                        if new_value.parse::<i64>().is_err() {
                            return Err(KamError::CommandFailed(format!(
                                "Invalid value: '{}' is not an integer; existing type requires integer for {}",
                                new_value, key
                            )));
                        }
                    }
                    toml::Value::Boolean(_) => {
                        if !(new_value == "true" || new_value == "false") {
                            return Err(KamError::CommandFailed(format!(
                                "Invalid value: '{}' is not a boolean; existing type requires boolean for {}",
                                new_value, key
                            )));
                        }
                    }
                    _ => {} // 字符串或其他类型，不检查
                }
            } else {
                // key不存在，但如果是已知的整数字段（如versionCode），强制要求整数
                let parts: Vec<&str> = key.split('.').collect();
                if let Some(last) = parts.last()
                    && *last == "versionCode"
                    && new_value.parse::<i64>().is_err()
                {
                    return Err(KamError::CommandFailed(format!(
                        "Invalid value: '{}' is not an integer; {} must be an integer",
                        new_value, key
                    )));
                }
            }
            set_value_by_path(&mut v, &key, &new_value);
            write_toml(&path, &v)?;
            use crate::utils::Utils;
            Utils::success(&format!(
                "Set {} = {} in {}",
                key,
                new_value,
                path.display()
            ));
            Ok(())
        }
        crate::cmds::toml::args::TomlCommand::Unset { key } => {
            // 删除key
            let mut v = read_toml(&path)?;
            let removed = unset_value_by_path(&mut v, &key);
            if removed {
                write_toml(&path, &v)?;
                use crate::utils::Utils;
                Utils::success(&format!("Unset {} in {}", key, path.display()));
                Ok(())
            } else {
                Err(KamError::CommandFailed(format!(
                    "Key '{}' not found in {}",
                    key,
                    path.display()
                )))
            }
        }
        crate::cmds::toml::args::TomlCommand::List => {
            // 列出所有配置（用pretty格式）
            let v = read_toml(&path)?;
            println!("{}", toml::to_string_pretty(&v).unwrap_or_default());
            Ok(())
        }
    }
}
