/// 处理 config 命令（get/set/unset/list/show）
///
/// # Errors
///
/// Returns `KamError` when file I/O or TOML parse/serialize operations fail.
#[allow(clippy::too_many_lines)] // TODO: split this function into smaller helpers
pub fn run(args: ConfigArgs) -> Result<(), KamError> {
    // 如果传入了 -i/--interactive，就进入交互式向导
    if args.interactive {
        if args.command.is_some() {
            return Err(KamError::CommandFailed(crate::i18n::tr(
                "config.interactive.error.conflict_with_subcommand",
            )));
        }
        return interactive_config(&args);
    }

    // 非交互模式，必须有一个子命令
    let Some(cmd) = args.command else {
        return Err(KamError::CommandFailed(crate::i18n::tr(
            "config.interactive.error.no_subcommand",
        )));
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
                    println!("{val}");
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
                        "Invalid usage: unexpected option '{final_value}' used as a value. If you meant to pass a global/local option, use:\n  kam config set --global <key> <value>\nor the shorthand:\n  kam config set <key>=<value> --global",
                    )));
                }

                // 如果 key 使用了 key=value 的简写，我们就将其拆分出来作为要写入的 key/value
                if final_key.contains('=') {
                    // Convert to owned Strings to avoid borrowing `final_key` while mutating it.
                    let parts: Vec<String> =
                        final_key.splitn(2, '=').map(ToString::to_string).collect();
                    if parts.len() >= 2 {
                        final_key.clone_from(&parts[0]);
                        final_value.clone_from(&parts[1]);
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
            crate::utils::Utils::success(&crate::trf!(
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
                crate::utils::Utils::success(&crate::trf!(
                    "config.unset_success",
                    key,
                    path.display()
                ));
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
fn prompt_input<P: AsRef<str>>(prompt: P, default: Option<&str>) -> Result<String, KamError> {
    use std::io::{self, Write};
    let prompt_ref = prompt.as_ref();
    let default_str = default.unwrap_or("").to_string();

    // Prefer dialoguer Input for a nicer interactive UI; fall back to stdio if it fails
    if let Ok(v) = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt_ref)
        .allow_empty(true)
        .default(default_str.clone())
        .interact_text()
    {
        return Ok(v);
    }

    // Fallback simple prompt (non-TTY)
    print!("{prompt_ref} ");
    if !default_str.is_empty() {
        print!("({default_str}) ");
    }
    io::stdout().flush().map_err(KamError::Io)?;
    let mut s = String::new();
    io::stdin().read_line(&mut s).map_err(KamError::Io)?;
    let s = s.trim().to_string();
    if s.is_empty() { Ok(default_str) } else { Ok(s) }
}

fn prompt_confirm<P: AsRef<str>>(prompt: P, default: bool) -> Result<bool, KamError> {
    let prompt_ref = prompt.as_ref();
    if let Ok(v) = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt_ref)
        .default(default)
        .interact()
    {
        Ok(v)
    } else {
        // Fallback to text prompt
        let suffix = if default { "[Y/n]" } else { "[y/N]" };
        loop {
            let prompt_str = format!("{prompt_ref} {suffix}");
            let resp = prompt_input(prompt_str, None::<&str>)?;
            let resp = resp.trim().to_lowercase();
            if resp.is_empty() {
                return Ok(default);
            }
            if resp == "y" || resp == "yes" {
                return Ok(true);
            }
            if resp == "n" || resp == "no" {
                return Ok(false);
            }
            println!("{}", crate::i18n::tr("init.interactive.enter_yes_no"));
        }
    }
}

#[allow(clippy::too_many_lines)] // TODO: split this function into smaller helpers to reduce complexity
fn interactive_config(args: &ConfigArgs) -> Result<(), KamError> {
    use crate::i18n::tr;
    use crate::utils::Utils;

    Utils::banner(tr("config.interactive.title"));
    Utils::info(tr("config.interactive.view_builtins"));
    Utils::info(tr("config.interactive.press_enter"));
    println!();

    // Determine which config file to edit: global or local
    let target_is_global = if args.global {
        true
    } else if args.local {
        false
    } else {
        // Default: if we are inside a project (kam.toml exists) prefer local
        let in_project = std::env::current_dir().is_ok_and(|cwd| cwd.join("kam.toml").exists());
        let choices = vec![
            if in_project {
                tr("config.interactive.local_project_detected")
            } else {
                tr("config.interactive.local_project_not_detected")
            },
            tr("config.interactive.global"),
            tr("config.interactive.cancel"),
        ];

        let pick = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(tr("config.interactive.select_target"))
            .items(&choices)
            .default(usize::from(!in_project))
            .interact_opt();

        let idx = if let Ok(Some(i)) = pick {
            i
        } else {
            // Fallback to text input
            let def = if in_project { "1" } else { "2" };
            let sel_prompt = tr("config.interactive.select_target");
            let input = prompt_input(sel_prompt, Some(def))?;
            match input.trim() {
                "1" => 0,
                "2" => 1,
                _ => 2,
            }
        };

        if idx == 2 {
            println!("{}", tr("config.interactive.aborted"));
            return Ok(());
        }
        idx == 1
    };

    let path = get_config_paths(target_is_global, !target_is_global)?;

    loop {
        let v = read_toml(&path)?;
        let cur_lang = get_value_by_path(&v, "ui.language")
            .and_then(|sv| sv.as_str().map(ToString::to_string))
            .unwrap_or_else(|| "<not set>".to_string());
        let cur_root = get_value_by_path(&v, "root.manager")
            .and_then(|sv| sv.as_str().map(ToString::to_string))
            .unwrap_or_else(|| "<not set>".to_string());

        let menu = vec![
            crate::trf!("config.interactive.menu.set_ui_language", cur_lang),
            crate::trf!("config.interactive.menu.set_root_manager", cur_root),
            tr("config.interactive.set_custom_key").clone(),
            tr("config.interactive.view_builtins").clone(),
            tr("config.interactive.show_current_config").clone(),
            tr("config.interactive.exit").clone(),
        ];

        let pick = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(tr("config.interactive.choose_action"))
            .items(&menu)
            .default(0)
            .interact_opt();

        let idx = if let Ok(Some(i)) = pick {
            i
        } else {
            let input = prompt_input(tr("config.interactive.select_option"), Some("1"))?;
            match input.trim().parse::<usize>() {
                Ok(n) if n >= 1 && n <= menu.len() => n - 1,
                _ => {
                    println!("{}", tr("config.interactive.invalid_selection"));
                    continue;
                }
            }
        };

        match idx {
            0 => {
                // Set language
                let lang_prompt = tr("config.interactive.enter_ui_language");
                let lang = prompt_input(lang_prompt, Some(&cur_lang))?;
                if lang.trim().is_empty() {
                    println!("{}", tr("config.interactive.no_change"));
                } else {
                    let mut toml_v = read_toml(&path)?;
                    set_value_by_path(&mut toml_v, "ui.language", &lang);
                    write_toml(&path, &toml_v)?;
                    Utils::success(&crate::trf!(
                        "config.set_success",
                        "ui.language",
                        lang,
                        path.display()
                    ));
                }
            }
            1 => {
                // Set root manager
                let choices = vec!["Magisk", "KernelSU", "APatchSU", "Other"];
                let pick_rm = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt(tr("config.interactive.choose_root_manager"))
                    .items(&choices)
                    .default(0)
                    .interact_opt();

                let manager = if let Ok(Some(i)) = pick_rm {
                    if choices[i] == "Other" {
                        let custom_root_prompt = tr("config.interactive.choose_root_manager");
                        prompt_input(custom_root_prompt, Some(&cur_root))?
                    } else {
                        choices[i].to_string()
                    }
                } else {
                    let root_prompt = tr("config.interactive.choose_root_manager");
                    prompt_input(root_prompt, Some(&cur_root))?
                };

                if manager.trim().is_empty() {
                    println!("{}", tr("config.interactive.no_change"));
                } else {
                    let mut toml_v = read_toml(&path)?;
                    set_value_by_path(&mut toml_v, "root.manager", &manager);
                    write_toml(&path, &toml_v)?;
                    Utils::success(&crate::trf!(
                        "config.set_success",
                        "root.manager",
                        manager,
                        path.display()
                    ));
                }
            }
            2 => {
                // Set other configuration key
                let key_prompt = tr("config.interactive.enter_custom_key");
                let key = prompt_input(key_prompt, None::<&str>)?;
                if key.trim().is_empty() {
                    println!("{}", tr("config.interactive.no_key_entered"));
                } else {
                    let val_prompt = crate::trf!("config.interactive.enter_value_for_key", key);
                    let value = prompt_input(&val_prompt, None::<&str>)?;
                    if value.trim().is_empty() {
                        println!("{}", tr("config.interactive.no_change"));
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

        if !prompt_confirm(tr("config.interactive.make_more_changes"), true)? {
            break;
        }
    }

    Ok(())
}
