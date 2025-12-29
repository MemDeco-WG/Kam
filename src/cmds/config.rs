use crate::errors::KamError;
use clap::{Args, Subcommand};
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};
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

    /// Interactive mode: guide the user through common configuration (language, root manager, etc.)
    #[arg(short = 'i', long = "interactive")]
    pub interactive: bool,

    #[command(subcommand)]
    pub command: Option<ConfigCommand>,
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
        // 强制使用全局配置。支持 `KAM_HOME` 环境变量来控制 kam 的家目录（默认：$HOME/.kam）
        let dir = crate::utils::kam_home_dir()?;
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
        let dir = crate::utils::kam_home_dir()?;
        Ok(dir.join("config.toml"))
    }
}

// 读取TOML配置文件
// 文件不存在就返回空表（这样不会报错）
pub fn read_language_from_config() -> Option<String> {
    // Prefer local config's language first, then fallback to global if not set locally.
    // This explicitly checks both local and global config files so a missing language
    // in the project's config will still allow a global preference to be used.

    // 1) Try forced local config (prefer a project-local setting)
    if let Ok(local_path) = get_config_paths(false, true)
        && let Ok(local_toml) = read_toml(&local_path)
    {
        // First check ui.language (preferred)
        if let Some(val) = get_value_by_path(&local_toml, "ui.language")
            && let Some(s) = val.as_str()
        {
            return Some(s.to_string());
        }
        // Fallback to `language`
        if let Some(val) = get_value_by_path(&local_toml, "language")
            && let Some(s) = val.as_str()
        {
            return Some(s.to_string());
        }
    }

    // 2) Fallback to global config if local didn't provide a language
    if let Ok(global_path) = get_config_paths(true, false)
        && let Ok(global_toml) = read_toml(&global_path)
    {
        // First check ui.language (preferred)
        if let Some(val) = get_value_by_path(&global_toml, "ui.language")
            && let Some(s) = val.as_str()
        {
            return Some(s.to_string());
        }
        // Fallback to `language`
        if let Some(val) = get_value_by_path(&global_toml, "language")
            && let Some(s) = val.as_str()
        {
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
    BuiltinConfigKey {
        key: "root.manager",
        description_key: "Preferred root manager for device installs (Magisk|KernelSU|APatchSU)",
        example: "Magisk",
    },
];

fn show_builtin_keys() {
    use crate::i18n::tr_key;

    println!("{}", tr_key("config.builtin_keys"));
    println!();

    for key_info in BUILTIN_KEYS {
        println!("  {}", key_info.key);
        println!("    {}", tr_key(key_info.description_key));
        println!("    {} {}", tr_key("config.example"), key_info.example);
        println!();
    }

    println!("{}", tr_key("config.note_custom_keys"));
}

// 处理config命令（get/set/unset/list/show）
// 和toml命令类似，但操作的是配置文件而不是kam.toml
pub fn run(args: ConfigArgs) -> Result<(), KamError> {
    // 如果传入了 -i/--interactive，就进入交互式向导
    if args.interactive {
        if args.command.is_some() {
            return Err(KamError::CommandFailed(
                crate::i18n::tr_key("config.interactive.error.conflict_with_subcommand")
                    .to_string(),
            ));
        }
        return interactive_config(&args);
    }

    // 非交互模式，必须有一个子命令
    let cmd = match args.command {
        Some(c) => c,
        None => {
            return Err(KamError::CommandFailed(
                crate::i18n::tr_key("config.interactive.error.no_subcommand").to_string(),
            ));
        }
    };

    match cmd {
        ConfigCommand::Show => {
            show_builtin_keys();
            Ok(())
        }
        ConfigCommand::Get { key } => {
            let path = get_config_paths(args.global, args.local)?;
            // 获取配置值
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
            let mut final_key = key;
            let mut final_value = value;

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
                    // Convert to owned Strings to avoid borrowing `final_key` while mutating it.
                    let parts: Vec<String> =
                        final_key.splitn(2, '=').map(ToString::to_string).collect();
                    if parts.len() >= 2 {
                        final_key = parts[0].clone();
                        final_value = parts[1].clone();
                    } else {
                        return Err(KamError::CommandFailed(
                            "Invalid key=value shorthand".to_string(),
                        ));
                    }
                } else {
                    // 没有 '=' 且第二个参数是一个选项：说明用户写法错误
                    return Err(KamError::CommandFailed(
                        "Invalid usage: when passing `--global` or `--local` as the second positional argument, prefer key=value shorthand, e.g.:\n  kam config set language=en -- --local\nor use the normal form:\n  kam config set --global language en".to_string()
                    ));
                }
            } else {
                // final_value 不是 option（常规情况）
                if final_key.contains('=') {
                    // Convert to owned Strings to avoid borrowing `final_key` while mutating it.
                    let parts: Vec<String> =
                        final_key.splitn(2, '=').map(ToString::to_string).collect();
                    if parts.len() >= 2 {
                        let shorthand_key = parts[0].clone();
                        let shorthand_val = parts[1].clone();
                        final_key = shorthand_key;

                        if final_value.is_empty() {
                            final_value = shorthand_val;
                        } // else 用户显式提供了第二个位置参数 value，我们优先保留 final_value
                    }
                }
            }

            // 最终选择用于写入的配置路径（可能根据 effective_global/effective_local 覆盖 CLI flags）
            let target_path = get_config_paths(effective_global, effective_local)?;
            let mut toml_v = read_toml(&target_path)?;
            set_value_by_path(&mut toml_v, &final_key, &final_value);
            write_toml(&target_path, &toml_v)?;
            use crate::utils::Utils;
            Utils::success(&crate::trf!(
                "config.set_success",
                final_key,
                final_value,
                target_path.display()
            ));
            Ok(())
        }
        ConfigCommand::Unset { key } => {
            let path = get_config_paths(args.global, args.local)?;
            // 删除配置值
            let mut v = read_toml(&path)?;
            let removed = unset_value_by_path(&mut v, &key);
            if removed {
                write_toml(&path, &v)?;
                use crate::utils::Utils;
                Utils::success(&crate::trf!("config.unset_success", key, path.display()));
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
            let path = get_config_paths(args.global, args.local)?;
            // 列出所有配置
            let v = read_toml(&path)?;
            println!("{}", toml::to_string_pretty(&v).unwrap_or_default());
            Ok(())
        }
    }
}

/// Prompt helpers & interactive flow
fn prompt_input(prompt: &str, default: Option<&str>) -> Result<String, KamError> {
    let default_str = default.unwrap_or("").to_string();

    // Prefer dialoguer Input for a nicer interactive UI; fall back to stdio if it fails
    if let Ok(v) = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .allow_empty(true)
        .default(default_str.clone())
        .interact_text()
    {
        return Ok(v);
    }

    // Fallback simple prompt (non-TTY)
    use std::io::{self, Write};
    print!("{} ", prompt);
    if !default_str.is_empty() {
        print!("({}) ", default_str);
    }
    io::stdout().flush().map_err(KamError::Io)?;
    let mut s = String::new();
    io::stdin().read_line(&mut s).map_err(KamError::Io)?;
    let s = s.trim().to_string();
    if s.is_empty() { Ok(default_str) } else { Ok(s) }
}

fn prompt_confirm(prompt: &str, default: bool) -> Result<bool, KamError> {
    match Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(default)
        .interact()
    {
        Ok(v) => Ok(v),
        Err(_) => {
            // Fallback to text prompt
            let suffix = if default { "[Y/n]" } else { "[y/N]" };
            loop {
                let resp = prompt_input(&format!("{} {}", prompt, suffix), None)?;
                let resp = resp.trim().to_lowercase();
                if resp.is_empty() {
                    return Ok(default);
                } else if resp == "y" || resp == "yes" {
                    return Ok(true);
                } else if resp == "n" || resp == "no" {
                    return Ok(false);
                } else {
                    println!("{}", crate::i18n::tr_key("init.interactive.enter_yes_no"));
                    continue;
                }
            }
        }
    }
}

fn interactive_config(args: &ConfigArgs) -> Result<(), KamError> {
    use crate::i18n::tr_key;
    use crate::utils::Utils;

    Utils::banner(tr_key("config.interactive.title"));
    Utils::info(tr_key("config.interactive.view_builtins"));
    Utils::info(tr_key("config.interactive.press_enter"));
    println!();

    // Determine which config file to edit: global or local
    let target_is_global = if args.global {
        true
    } else if args.local {
        false
    } else {
        // Default: if we are inside a project (kam.toml exists) prefer local
        let in_project = std::env::current_dir()
            .map(|cwd| cwd.join("kam.toml").exists())
            .unwrap_or(false);
        let choices = vec![
            if in_project {
                tr_key("config.interactive.local_project_detected").to_string()
            } else {
                tr_key("config.interactive.local_project_not_detected").to_string()
            },
            tr_key("config.interactive.global").to_string(),
            tr_key("config.interactive.cancel").to_string(),
        ];

        let pick = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(tr_key("config.interactive.select_target"))
            .items(&choices)
            .default(if in_project { 0 } else { 1 })
            .interact_opt();

        let idx = match pick {
            Ok(Some(i)) => i,
            _ => {
                // Fallback to text input
                let def = if in_project { "1" } else { "2" };
                let sel_prompt = tr_key("config.interactive.select_target");
                let input = prompt_input(sel_prompt, Some(def))?;
                match input.trim() {
                    "1" => 0,
                    "2" => 1,
                    _ => 2,
                }
            }
        };

        if idx == 2 {
            println!("{}", tr_key("config.interactive.aborted"));
            return Ok(());
        }
        idx == 1
    };

    let path = get_config_paths(target_is_global, !target_is_global)?;

    loop {
        let v = read_toml(&path)?;
        let cur_lang = get_value_by_path(&v, "ui.language")
            .and_then(|sv| sv.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "<not set>".to_string());
        let cur_root = get_value_by_path(&v, "root.manager")
            .and_then(|sv| sv.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "<not set>".to_string());

        let menu = vec![
            crate::trf!("config.interactive.menu.set_ui_language", cur_lang),
            crate::trf!("config.interactive.menu.set_root_manager", cur_root),
            tr_key("config.interactive.set_custom_key").to_string(),
            tr_key("config.interactive.view_builtins").to_string(),
            tr_key("config.interactive.show_current_config").to_string(),
            tr_key("config.interactive.exit").to_string(),
        ];

        let pick = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(tr_key("config.interactive.choose_action"))
            .items(&menu)
            .default(0)
            .interact_opt();

        let idx = match pick {
            Ok(Some(i)) => i,
            _ => {
                let input = prompt_input(tr_key("config.interactive.select_option"), Some("1"))?;
                match input.trim().parse::<usize>() {
                    Ok(n) if n >= 1 && n <= menu.len() => n - 1,
                    _ => {
                        println!("{}", tr_key("config.interactive.invalid_selection"));
                        continue;
                    }
                }
            }
        };

        match idx {
            0 => {
                // Set language
                let lang_prompt = tr_key("config.interactive.enter_ui_language");
                let lang = prompt_input(lang_prompt, Some(&cur_lang))?;
                if !lang.trim().is_empty() {
                    let mut toml_v = read_toml(&path)?;
                    set_value_by_path(&mut toml_v, "ui.language", &lang);
                    write_toml(&path, &toml_v)?;
                    Utils::success(&crate::trf!(
                        "config.set_success",
                        "ui.language",
                        lang,
                        path.display()
                    ));
                } else {
                    println!("{}", tr_key("config.interactive.no_change"));
                }
            }
            1 => {
                // Set root manager
                let choices = vec!["Magisk", "KernelSU", "APatchSU", "Other"];
                let pick_rm = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt(tr_key("config.interactive.choose_root_manager"))
                    .items(&choices)
                    .default(0)
                    .interact_opt();

                let manager = match pick_rm {
                    Ok(Some(i)) => {
                        if choices[i] == "Other" {
                            let custom_root_prompt =
                                tr_key("config.interactive.choose_root_manager");
                            prompt_input(custom_root_prompt, Some(&cur_root))?
                        } else {
                            choices[i].to_string()
                        }
                    }
                    _ => {
                        let root_prompt = tr_key("config.interactive.choose_root_manager");
                        prompt_input(root_prompt, Some(&cur_root))?
                    }
                };

                if !manager.trim().is_empty() {
                    let mut toml_v = read_toml(&path)?;
                    set_value_by_path(&mut toml_v, "root.manager", &manager);
                    write_toml(&path, &toml_v)?;
                    Utils::success(&crate::trf!(
                        "config.set_success",
                        "root.manager",
                        manager,
                        path.display()
                    ));
                } else {
                    println!("{}", tr_key("config.interactive.no_change"));
                }
            }
            2 => {
                // Set other configuration key
                let key_prompt = tr_key("config.interactive.enter_custom_key");
                let key = prompt_input(key_prompt, None)?;
                if key.trim().is_empty() {
                    println!("{}", tr_key("config.interactive.no_key_entered"));
                } else {
                    let val_prompt = crate::trf!("config.interactive.enter_value_for_key", key);
                    let value = prompt_input(&val_prompt, None)?;
                    if value.trim().is_empty() {
                        println!("{}", tr_key("config.interactive.no_change"));
                    } else {
                        let mut toml_v = read_toml(&path)?;
                        set_value_by_path(&mut toml_v, &key, &value);
                        write_toml(&path, &toml_v)?;
                        Utils::success(&crate::trf!(
                            "config.set_success",
                            key,
                            value,
                            path.display()
                        ));
                    }
                }
            }
            3 => {
                // Show built-in keys
                show_builtin_keys();
            }
            4 => {
                // Show current config file
                let v = read_toml(&path)?;
                println!("{}", toml::to_string_pretty(&v).unwrap_or_default());
            }
            5 => break,
            _ => unreachable!(),
        }

        if !prompt_confirm(tr_key("config.interactive.make_more_changes"), true)? {
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn mk_temp_dir(prefix: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = env::temp_dir().join(format!("kam_test_{}_{}", prefix, now));
        // Ignore failure here; callers will handle creation explicitly if needed.
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn restore_env(
        orig_home: Option<String>,
        orig_kam_ui: Option<String>,
        orig_kam_lang: Option<String>,
        orig_cwd: PathBuf,
    ) {
        if let Some(h) = orig_home {
            unsafe { env::set_var("HOME", h) };
        } else {
            unsafe { env::remove_var("HOME") };
        }
        if let Some(k) = orig_kam_ui {
            unsafe { env::set_var("KAM_UI_LANGUAGE", k) };
        } else {
            unsafe { env::remove_var("KAM_UI_LANGUAGE") };
        }
        if let Some(k2) = orig_kam_lang {
            unsafe { env::set_var("KAM_LANG", k2) };
        } else {
            unsafe { env::remove_var("KAM_LANG") };
        }
        let _ = env::set_current_dir(orig_cwd);
    }

    #[test]
    #[serial]
    fn test_read_language_from_local_config_priority() {
        // Save environment / working dir
        let orig_home = env::var("HOME").ok();
        let orig_kam_ui = env::var("KAM_UI_LANGUAGE").ok();
        let orig_kam_lang = env::var("KAM_LANG").ok();
        let orig_cwd = env::current_dir().unwrap();

        // Prepare a project directory with a local `.kam/config.toml`
        let tmp = mk_temp_dir("local");
        fs::create_dir_all(tmp.join(".kam")).unwrap();
        fs::write(tmp.join("kam.toml"), "name = \"test\"").unwrap();
        fs::write(
            tmp.join(".kam").join("config.toml"),
            r#"ui.language = "zh""#,
        )
        .unwrap();

        // Make sure no env overrides interfere
        unsafe {
            env::remove_var("KAM_UI_LANGUAGE");
        }
        unsafe {
            env::remove_var("KAM_LANG");
        }
        // Do not override HOME for this local-only test; rely on the project directory detection.
        // (Setting HOME in-process is brittle because `dirs` may cache the home directory.)
        // Switch to our project dir (so get_config_paths(false, true) resolves to local)
        env::set_current_dir(&tmp).unwrap();

        let lang = read_language_from_config();
        assert_eq!(lang.as_deref(), Some("zh"));

        // Cleanup / restore
        restore_env(orig_home, orig_kam_ui, orig_kam_lang, orig_cwd);
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    #[serial]
    fn test_read_language_from_global_config_fallback() {
        // Save environment / working dir
        let orig_home = env::var("HOME").ok();
        let orig_kam_ui = env::var("KAM_UI_LANGUAGE").ok();
        let orig_kam_lang = env::var("KAM_LANG").ok();
        let orig_cwd = env::current_dir().unwrap();

        // Prepare a HOME with `.kam/config.toml`
        let htmp = mk_temp_dir("home");
        fs::create_dir_all(htmp.join(".kam")).unwrap();
        fs::write(
            htmp.join(".kam").join("config.toml"),
            r#"ui.language = "en""#,
        )
        .unwrap();

        // Make sure no env overrides interfere
        unsafe {
            env::remove_var("KAM_UI_LANGUAGE");
        }
        unsafe {
            env::remove_var("KAM_LANG");
        }
        // Set HOME to our fake home
        unsafe {
            env::set_var("HOME", htmp.to_str().unwrap());
        }
        // Because the `dirs` crate may cache the first-observed home path for the process,
        // confirm that it actually returned our new home value; if it didn't, skip the test
        // to avoid writing into the real user's home directory during testing.
        if dirs::home_dir().as_deref() != Some(htmp.as_path()) {
            // restore environment & perform cleanup, then skip the test
            restore_env(orig_home, orig_kam_ui, orig_kam_lang, orig_cwd);
            let _ = fs::remove_dir_all(&htmp);
            return;
        }
        // Ensure current dir is not a project containing kam.toml
        env::set_current_dir(env::temp_dir()).unwrap();

        let lang = read_language_from_config();
        assert_eq!(lang.as_deref(), Some("en"));

        // Cleanup / restore
        restore_env(orig_home, orig_kam_ui, orig_kam_lang, orig_cwd);
        let _ = fs::remove_dir_all(htmp);
    }

    #[test]
    #[serial]
    fn test_local_over_global_preference() {
        // Save environment / working dir
        let orig_home = env::var("HOME").ok();
        let orig_kam_ui = env::var("KAM_UI_LANGUAGE").ok();
        let orig_kam_lang = env::var("KAM_LANG").ok();
        let orig_cwd = env::current_dir().unwrap();

        // Prepare a project directory with local config
        let tmp = mk_temp_dir("both");
        fs::create_dir_all(tmp.join(".kam")).unwrap();
        fs::write(tmp.join("kam.toml"), "name = \"test\"").unwrap();
        fs::write(
            tmp.join(".kam").join("config.toml"),
            r#"ui.language = "zh""#,
        )
        .unwrap();

        // Prepare a global HOME with a different value
        let htmp = mk_temp_dir("home2");
        fs::create_dir_all(htmp.join(".kam")).unwrap();
        fs::write(
            htmp.join(".kam").join("config.toml"),
            r#"ui.language = "en""#,
        )
        .unwrap();

        // Remove env overrides
        unsafe {
            env::remove_var("KAM_UI_LANGUAGE");
        }
        unsafe {
            env::remove_var("KAM_LANG");
        }
        // Point HOME to the global config with `en`
        unsafe {
            env::set_var("HOME", htmp.to_str().unwrap());
        }
        // Ensure `dirs::home_dir()` reflects this change — otherwise skip to avoid mutating real home
        if dirs::home_dir().as_deref() != Some(htmp.as_path()) {
            restore_env(orig_home, orig_kam_ui, orig_kam_lang, orig_cwd);
            let _ = fs::remove_dir_all(&tmp);
            let _ = fs::remove_dir_all(&htmp);
            return;
        }
        // Set current dir into our project containing `kam.toml`
        env::set_current_dir(&tmp).unwrap();

        // Local (zh) should be preferred over global (en)
        let lang = read_language_from_config();
        assert_eq!(lang.as_deref(), Some("zh"));

        // Cleanup / restore
        restore_env(orig_home, orig_kam_ui, orig_kam_lang, orig_cwd);
        let _ = fs::remove_dir_all(tmp);
        let _ = fs::remove_dir_all(htmp);
    }

    #[test]
    #[serial]
    fn test_config_set_shorthand_with_misplaced_local_flag_parsed_correctly() {
        // Save environment / working dir
        let orig_home = env::var("HOME").ok();
        let orig_kam_ui = env::var("KAM_UI_LANGUAGE").ok();
        let orig_kam_lang = env::var("KAM_LANG").ok();
        let orig_cwd = env::current_dir().unwrap();

        // Prepare a temporary project directory with a kam.toml so get_config_paths uses local paths
        let tmp = mk_temp_dir("cfg_set_misuse");
        fs::create_dir_all(tmp.join(".kam")).unwrap();
        fs::write(tmp.join("kam.toml"), "name = \"test\"").unwrap();

        // Ensure no env override is in place
        unsafe {
            env::remove_var("KAM_UI_LANGUAGE");
        }
        unsafe {
            env::remove_var("KAM_LANG");
        }

        // Switch to our project dir so get_config_paths(false, false) resolves to local
        env::set_current_dir(&tmp).unwrap();

        // Simulate: `kam config set language=en -- --local`
        let args = ConfigArgs {
            global: false,
            local: false,
            interactive: false,
            command: Some(ConfigCommand::Set {
                key: "language=en".to_string(),
                value: "--local".to_string(),
            }),
        };

        // Execute command; this should write the "language = \"en\"" into the local config
        super::run(args).unwrap();

        // Verify the local config file contains language = "en"
        let config_path = get_config_paths(false, true).unwrap();
        let toml_v = read_toml(&config_path).unwrap();
        assert_eq!(
            get_value_by_path(&toml_v, "language").and_then(|v| v.as_str().map(|s| s.to_string())),
            Some("en".to_string())
        );

        // Cleanup / restore
        restore_env(orig_home, orig_kam_ui, orig_kam_lang, orig_cwd);
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    #[serial]
    fn test_interactive_conflicts_with_subcommand() {
        // Ensure using -i together with a subcommand returns a helpful error
        let args = ConfigArgs {
            global: false,
            local: false,
            interactive: true,
            command: Some(ConfigCommand::Show),
        };

        let res = super::run(args);
        assert!(res.is_err());
        if let Err(e) = res {
            match e {
                KamError::CommandFailed(msg) => {
                    assert!(msg.contains("cannot be used with subcommands"));
                }
                _ => panic!("Expected CommandFailed error"),
            }
        }
    }
}
