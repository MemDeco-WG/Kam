use crate::errors::KamError;
use clap::{Args, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
pub struct ConfigArgs {
    /// Use the global configuration file (~/.kam/config.toml)
    #[arg(long)]
    pub global: bool,
    /// Force use of the local configuration file (project `.kam/config.toml`)
    #[arg(long, conflicts_with = "global")]
    pub local: bool,

    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Get a configuration value by key (dot-separated path)
    Get {
        /// Configuration key (dot-separated path, e.g., "ui.language")
        key: String,
    },
    /// Set a configuration value by key
    Set {
        /// Configuration key (dot-separated path, e.g., "ui.language")
        key: String,
        /// Configuration value to set
        value: String,
    },
    /// Unset (remove) a configuration value by key
    Unset {
        /// Configuration key (dot-separated path, e.g., "ui.language")
        key: String,
    },
    /// List all config values in the target file
    List,
    /// Show built-in configuration keys and their descriptions
    Show,
}

// 获取配置文件路径
// 支持全局配置（~/.kam/config.toml）和本地配置（项目/.kam/config.toml）
pub fn get_config_paths(global: bool, local: bool) -> Result<PathBuf, KamError> {
    if global {
        // 强制使用全局配置
        let home = dirs::home_dir().ok_or_else(|| {
            KamError::CommandFailed("Cannot determine home directory for global config".to_string())
        })?;
        let dir = home.join(".kam");
        Ok(dir.join("config.toml"))
    } else if local {
        // 强制使用本地配置
        // 向上查找kam.toml找到项目根目录，找不到就用当前目录
        let mut cwd = std::env::current_dir().map_err(KamError::Io)?;
        loop {
            if cwd.join("kam.toml").exists() {
                break;
            }
            if !cwd.pop() {
                break;
            }
        }
        // 如果找不到kam.toml，就用当前目录（虽然可能不太对）
        if !cwd.join("kam.toml").exists() {
            cwd = std::env::current_dir().map_err(KamError::Io)?;
        }
        let dir = cwd.join(".kam");
        Ok(dir.join("config.toml"))
    } else {
        // 默认行为：如果在项目里就用本地配置，否则用全局配置
        let mut cwd = std::env::current_dir().map_err(KamError::Io)?;
        loop {
            if cwd.join("kam.toml").exists() {
                let dir = cwd.join(".kam");
                return Ok(dir.join("config.toml"));
            }
            if !cwd.pop() {
                break;
            }
        }
        // 不在项目里，用全局配置
        let home = dirs::home_dir().ok_or_else(|| {
            KamError::CommandFailed("Cannot determine home directory for global config".to_string())
        })?;
        let dir = home.join(".kam");
        Ok(dir.join("config.toml"))
    }
}

// 读取TOML配置文件
// 文件不存在就返回空表（这样不会报错）
pub fn read_language_from_config() -> Option<String> {
    // Use the default behavior for config location (local if in project, otherwise global)
    let path = match get_config_paths(false, false) {
        Ok(p) => p,
        Err(_) => return None,
    };

    let toml_v = match read_toml(&path) {
        Ok(v) => v,
        Err(_) => return None,
    };

    // First check ui.language (preferred)
    if let Some(val) = get_value_by_path(&toml_v, "ui.language") {
        if let Some(s) = val.as_str() {
            return Some(s.to_string());
        }
    }

    // Fallback to `language`
    if let Some(val) = get_value_by_path(&toml_v, "language") {
        if let Some(s) = val.as_str() {
            return Some(s.to_string());
        }
    }

    None
}

fn read_toml(path: &Path) -> Result<toml::Value, KamError> {
    if !path.exists() {
        return Ok(toml::Value::Table(Default::default()));
    }
    let s = fs::read_to_string(path).map_err(KamError::Io)?;
    let v: toml::Value = toml::from_str(&s)
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse config file: {}", e)))?;
    Ok(v)
}

// 写入TOML配置文件
// 用pretty格式，这样看起来比较好看
fn write_toml(path: &Path, v: &toml::Value) -> Result<(), KamError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }
    let s = toml::to_string_pretty(v)
        .map_err(|e| KamError::CommandFailed(format!("Failed to serialize config: {}", e)))?;
    fs::write(path, s).map_err(KamError::Io)?;
    Ok(())
}

fn set_value_by_path(value: &mut toml::Value, path: &str, new_value: &str) {
    let v = value;
    if !v.is_table() {
        *v = toml::Value::Table(Default::default());
    }
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = v.as_table_mut().unwrap();
    for (i, &part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if part == "versionCode" {
                if let Ok(num) = new_value.parse::<i64>() {
                    current.insert(part.to_string(), toml::Value::Integer(num));
                } else {
                    current.insert(part.to_string(), toml::Value::String(new_value.to_string()));
                }
            } else {
                current.insert(part.to_string(), toml::Value::String(new_value.to_string()));
            }
            return;
        }
        if !current.contains_key(part) {
            current.insert(part.to_string(), toml::Value::Table(Default::default()));
        }
        current = current[part].as_table_mut().unwrap();
    }
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

fn unset_value_by_path(value: &mut toml::Value, path: &str) -> bool {
    let parts: Vec<&str> = path.split('.').collect();
    if !value.is_table() {
        return false;
    }
    let mut current_tbl = value.as_table_mut().unwrap();
    for (i, &part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            return current_tbl.remove(part).is_some();
        } else {
            if let Some(next) = current_tbl.get_mut(part) {
                if next.is_table() {
                    current_tbl = next.as_table_mut().unwrap();
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }
    }
    false
}

/// Built-in configuration keys and their descriptions
struct BuiltinConfigKey {
    key: &'static str,
    description_key: &'static str,
    example: &'static str,
}

const BUILTIN_KEYS: &[BuiltinConfigKey] = &[
    BuiltinConfigKey {
        key: "ui.language",
        description_key: "UI language preference (preferred over 'language')",
        example: "en, zh, zh-CN",
    },
    BuiltinConfigKey {
        key: "language",
        description_key: "UI language preference (fallback, use 'ui.language' instead)",
        example: "en, zh, zh-CN",
    },
    BuiltinConfigKey {
        key: "prop.author",
        description_key: "Default author name for new projects (saved during init)",
        example: "Your Name",
    },
    BuiltinConfigKey {
        key: "prop.name",
        description_key: "Default module name for new projects (saved during init)",
        example: "My Module",
    },
    BuiltinConfigKey {
        key: "prop.version",
        description_key: "Default version for new projects (saved during init)",
        example: "1.0.0",
    },
];

fn show_builtin_keys() {
    use crate::i18n::tr_key;

    println!("{}", tr_key("Built-in configuration keys:"));
    println!();

    for key_info in BUILTIN_KEYS {
        println!("  {}", key_info.key);
        println!("    {}", tr_key(key_info.description_key));
        println!("    {} {}", tr_key("Example:"), key_info.example);
        println!();
    }

    println!("{}", tr_key("Note: You can also set custom keys. Use 'kam config list' to see all configured values."));
}

// 处理config命令（get/set/unset/list/show）
// 和toml命令类似，但操作的是配置文件而不是kam.toml
pub fn run(args: ConfigArgs) -> Result<(), KamError> {
    match args.command {
        ConfigCommand::Show => {
            show_builtin_keys();
            Ok(())
        }
        _ => {
            let path = get_config_paths(args.global, args.local)?;

            match args.command {
                ConfigCommand::Get { key } => {
                    // 获取配置值
                    let v = read_toml(&path)?;
                    if let Some(val) = get_value_by_path(&v, &key) {
                        println!("{}", val);
                        Ok(())
                    } else {
                        Err(KamError::CommandFailed(format!(
                            "Key '{}' not found in {}",
                            key,
                            path.display()
                        )))
                    }
                }
                ConfigCommand::Set { key, value } => {
                    // 设置配置值
                    // 支持以下几种情况：
                    // 1) 标准用法：kam config set <key> <value> [--global|--local]
                    // 2) 简写：kam config set <key>=<value> [--global|--local]
                    // 3) 常见误用：kam config set <key>=<value> -- --global  -> 我们会检测并给出友好提示/修复
                    //
                    // 处理步骤：
                    // - 检测 value 是否是一个以 '-' 开头的选项（例如用户使用了 `--` 错误分隔参数）
                    // - 检测 key 是否为 key=value 简写
                    // - 根据实际 flags（args.global / args.local）或 value 中的误用选项来决定写入的目标配置文件
                    let mut effective_global = args.global;
                    let mut effective_local = args.local;
                    let mut final_key = key.clone();
                    let mut final_value = value.clone();

                    // 如果 value 看起来像一个选项（以 '-' 开头），有可能用户把 `--global` 当作 value 传入了（例如使用了 `--`）
                    if final_value.starts_with('-') {
                        // 检查是否是我们识别的 flag
                        if final_value == "--global" || final_value == "-g" {
                            effective_global = true;
                            effective_local = false;
                        } else if final_value == "--local" || final_value == "-l" {
                            effective_local = true;
                            effective_global = false;
                        } else {
                            // value 是以 - 开头，但不是我们识别的配置标志：很有可能是误用。
                            return Err(KamError::CommandFailed(format!(
                                "Invalid usage: unexpected option '{}' used as a value. If you meant to pass a global/local option, use:\n  kam config set --global <key> <value>\nor the shorthand:\n  kam config set <key>=<value> --global",
                                final_value
                            )));
                        }

                        // 如果 key 使用了 key=value 的简写，我们就将其拆分出来作为要写入的 key/value
                        if final_key.contains('=') {
                            let parts: Vec<&str> = final_key.splitn(2, '=').collect();
                            final_key = parts[0].to_string();
                            final_value = parts[1].to_string();
                        } else {
                            // 没有 '=' 且第二个参数是一个选项：说明用户写法错误
                            return Err(KamError::CommandFailed(
                                "Invalid usage: when passing `--global` or `--local` as the second positional argument, prefer key=value shorthand, e.g.:\n  kam config set language=en -- --global\nor use the normal form:\n  kam config set --global language en".to_string()
                            ));
                        }
                    } else {
                        // final_value 不是 option（常规情况）
                        if final_key.contains('=') {
                            // 支持 key=value 简写。如果用户同时也传了 value（非 option），优先使用用户显式传入的 value（value 参数）；
                            // 否则使用 key 中指定的 value。
                            let parts: Vec<&str> = final_key.splitn(2, '=').collect();
                            let shorthand_key = parts[0];
                            let shorthand_val = parts[1];
                            final_key = shorthand_key.to_string();

                            if final_value.is_empty() {
                                final_value = shorthand_val.to_string();
                            } // else 用户显式提供了第二个位置参数 value，我们优先保留 final_value
                        }
                    }

                    // 最终选择用于写入的配置路径（可能根据 effective_global/effective_local 覆盖 CLI flags）
                    let target_path = get_config_paths(effective_global, effective_local)?;
                    let mut toml_v = read_toml(&target_path)?;
                    set_value_by_path(&mut toml_v, &final_key, &final_value);
                    write_toml(&target_path, &toml_v)?;
                    use crate::utils::Utils;
                    Utils::success(&crate::trf!("Set {} = {} in {}", final_key, final_value, target_path.display()));
                    Ok(())
                }
                ConfigCommand::Unset { key } => {
                    // 删除配置值
                    let mut v = read_toml(&path)?;
                    let removed = unset_value_by_path(&mut v, &key);
                    if removed {
                        write_toml(&path, &v)?;
                        use crate::utils::Utils;
                        Utils::success(&crate::trf!("Unset {} in {}", key, path.display()));
                        Ok(())
                    } else {
                        Err(KamError::CommandFailed(format!(
                            "Key '{}' not found in {}",
                            key,
                            path.display()
                        )))
                    }
                }
                ConfigCommand::List => {
                    // 列出所有配置
                    let v = read_toml(&path)?;
                    println!("{}", toml::to_string_pretty(&v).unwrap_or_default());
                    Ok(())
                }
                ConfigCommand::Show => unreachable!(),
            }
        }
    }
}
