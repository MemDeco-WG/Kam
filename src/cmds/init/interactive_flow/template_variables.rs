#[allow(clippy::too_many_lines)] // TODO: consider splitting into smaller helper functions
fn prompt_template_variables(
    kam_toml_path: &Path,
    existing: &mut HashMap<String, String>,
) -> Result<(), KamError> {
    let kt = KamToml::load_from_file(kam_toml_path)?;
    if let Some(tmpl_section) = kt.tmpl {
        for (var_name, var_def) in &tmpl_section.variables {
            let default = existing
                .get(var_name)
                .cloned()
                .or_else(|| var_def.default.clone())
                .unwrap_or_default();

            // Show notes/help/example where available

            println!();
            if let Some(note) = &var_def.note {
                Utils::info(&trf!("init.interactive.variable_note", var_name, note));
            } else {
                Utils::info(&trf!("init.interactive.variable", var_name));
            }
            if let Some(help) = &var_def.help {
                println!(
                    "  {} {}",
                    "→".blue().dimmed(),
                    trf!("init.interactive.help", help).dimmed()
                );
            }
            if let Some(example) = &var_def.example {
                println!(
                    "  {} {}",
                    "→".blue().dimmed(),
                    trf!("init.interactive.example", example).dimmed()
                );
            }
            if !default.is_empty() {
                println!(
                    "  {} {}",
                    "→".blue().dimmed(),
                    trf!("init.interactive.default", default).dimmed()
                );
            }

            // 如果变量提供了choices，优先用可视化的Select，也允许自定义值
            if let Some(choices) = &var_def.choices {
                // 提供现有选项加上"<Custom value>"选项，允许自由输入
                let mut opts = choices.clone();
                opts.push(trf!("init.interactive.custom_value_choice"));

                // 默认索引：优先匹配默认值，没有就用0
                let default_idx = choices.iter().position(|c| c == &default).unwrap_or(0);

                // 尝试用dialoguer Select（方向键导航），失败就回退到文本输入
                // 虽然可能有点慢，但至少用户体验好一点
                match Select::with_theme(&ColorfulTheme::default())
                    .with_prompt(&trf!("init.interactive.select_value_for", var_name))
                    .items(&opts)
                    .default(default_idx)
                    .interact()
                {
                    Ok(idx) => {
                        if idx < choices.len() {
                            // Chosen one of the predefined choices
                            existing.insert(var_name.clone(), choices[idx].clone());
                        } else {
                            // User chose the '<Custom value>' option -> ask for free-form input
                            let custom = Input::<String>::with_theme(&ColorfulTheme::default())
                                .with_prompt(&trf!(
                                    "init.interactive.enter_custom_value_for",
                                    var_name
                                ))
                                .allow_empty(!var_def.required)
                                .default(default.clone())
                                .interact_text()
                                .map_err(|e| KamError::Io(e.into()))?;
                            if custom.is_empty() && var_def.required {
                                return Err(KamError::InvalidConfig(trf!(
                                    "init.interactive.value_required",
                                    var_name
                                )));
                            }
                            existing.insert(var_name.clone(), custom);
                        }
                    }
                    Err(_) => {
                        // dialoguer failed (likely non-interactive): fallback to text-based flow
                        loop {
                            let response = prompt_input(
                                &trf!("init.interactive.enter_value_for_index_or_value", var_name),
                                Some(&default),
                            )?;
                            if response.is_empty() && var_def.required {
                                Utils::warn(&trf!("init.interactive.value_required", var_name));
                                continue;
                            }
                            // numeric index allowed (1-based)
                            if let Ok(idx) = response.parse::<usize>() {
                                if idx > 0 && idx <= choices.len() {
                                    existing.insert(var_name.clone(), choices[idx - 1].clone());
                                    break;
                                }
                                Utils::warn(&trf!("init.interactive.invalid_selection_index"));
                                continue;
                            }
                            if !choices.contains(&response) {
                                if prompt_confirm(
                                    &trf!("init.interactive.value_is_not_choice_prompt"),
                                    false,
                                )? {
                                    existing.insert(var_name.clone(), response.clone());
                                    break;
                                }
                                continue;
                            }
                            existing.insert(var_name.clone(), response.clone());
                            break;
                        }
                    }
                }
            } else {
                // No choices: handle bool as Confirm, otherwise use Input
                if var_def.var_type == "bool" {
                    let default_bool =
                        !default.is_empty() && (default == "1" || default.to_lowercase() == "true");
                    match Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt(format!("{var_name}?"))
                        .default(default_bool)
                        .interact()
                    {
                        Ok(v) => {
                            existing.insert(
                                var_name.clone(),
                                if v { "1".to_string() } else { "0".to_string() },
                            );
                        }
                        Err(_) => {
                            // Fallback to text prompt for bool
                            loop {
                                let resp = prompt_input(
                                    &trf!(
                                        "init.interactive.enter_true_false_for",
                                        var_name,
                                        if default_bool { "true" } else { "false" }
                                    ),
                                    Some(if default_bool { "true" } else { "false" }),
                                )?;
                                if resp.is_empty() && var_def.required {
                                    Utils::warn(&trf!("init.interactive.value_required", var_name));
                                    continue;
                                }
                                let v = resp.to_lowercase();
                                if v == "true" || v == "1" {
                                    existing.insert(var_name.clone(), "1".to_string());
                                    break;
                                } else if v == "false" || v == "0" {
                                    existing.insert(var_name.clone(), "0".to_string());
                                    break;
                                }
                                Utils::warn(&trf!("init.interactive.enter_true_or_false"));
                            }
                        }
                    }
                } else {
                    // String input via dialoguer Input, fallback to prompt_input
                    loop {
                        let resp = Input::<String>::with_theme(&ColorfulTheme::default())
                            .with_prompt(&trf!("init.interactive.enter_value_for", var_name))
                            .allow_empty(true)
                            .default(default.clone())
                            .interact_text();
                        let response = match resp {
                            Ok(s) => s,
                            Err(_) => prompt_input(
                                &trf!("init.interactive.enter_value_for", var_name),
                                Some(&default),
                            )?,
                        };
                        if response.is_empty() && var_def.required {
                            Utils::warn(&trf!("init.interactive.value_required", var_name));
                            continue;
                        }
                        existing.insert(var_name.clone(), response.clone());
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

// 构建会被模板复制的文件列表（用于可视化）
// 这个功能主要是让用户看看模板会生成哪些文件
#[allow(clippy::too_many_lines)] // TODO: consider splitting into smaller helper functions
fn visualize_template(template_dir: &Path, limit: usize) -> Result<(), KamError> {
    // 构建树状列表（带缩进），提供Select UI来滚动和预览文件
    // 用dialoguer::Select支持方向键导航，比较友好
    let mut entries: Vec<(String, PathBuf, bool)> = Vec::new();

    // 遍历模板目录，收集所有文件
    for entry in WalkDir::new(template_dir)
        .into_iter()
        .filter_map(Result::ok)
    // 忽略读取失败的条目
    {
        // 使用metadata（如果可能），忽略损坏的条目
        let Ok(metadata) = entry.metadata() else {
            continue;
        }; // 读取失败就跳过
        // 只考虑文件和目录
        let path = entry.path();
        // 相对路径用于显示
        let rel = path
            .strip_prefix(template_dir)
            .unwrap_or(path)
            .to_path_buf();
        // 计算深度用于树状前缀
        let depth = rel.components().count().saturating_sub(1);
        let mut prefix = String::new();
        for _ in 0..depth {
            prefix.push_str("  "); // 每层缩进2个空格
        }
        // 显示名称（只用最后一部分，让树更紧凑）
        let is_file = metadata.is_file();
        let display_name = rel.file_name().and_then(|s| s.to_str()).map_or_else(
            || {
                if is_file {
                    format!("{prefix}{rel}", rel = rel.display())
                } else {
                    format!("{prefix}{rel}/", rel = rel.display())
                }
            },
            |name| {
                if is_file {
                    format!("{prefix}{name}")
                } else {
                    format!("{prefix}{name}/")
                }
            },
        );
        entries.push((display_name, rel, is_file));
    }

    // 排序，确保顺序确定（虽然可能不太重要，但至少一致）
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    // Nothing to show?
    if entries.is_empty() {
        Utils::info(&trf!("init.interactive.template_directory_empty"));
        return Ok(());
    }

    // Build the display items (with icons and color: 📁 for dirs, 📄 for files)
    let mut display_items: Vec<String> = entries
        .iter()
        .map(|(d, _, is_file)| {
            if *is_file {
                // file icon, dimmed for less emphasis
                format!("📄 {d}").dimmed().to_string()
            } else {
                // folder icon, cyan color to stand out
                format!("📁 {d}").cyan().to_string()
            }
        })
        .collect();
    // Add control items at the end
    display_items.push(trf!("init.interactive.preview_continue"));
    display_items.push(trf!("init.interactive.preview_exit"));

    // Interactive selection loop
    loop {
        // Interactive select (arrow keys). If it fails (non TTY / CI), fallback to text listing.
        if let Ok(idx) = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(&trf!("init.interactive.template_preview", entries.len()))
            .items(&display_items)
            .default(0)
            .interact()
        {
            let choices_len = display_items.len();
            // If choose continue -> exit preview and return Ok
            if idx == choices_len - 2 {
                break;
            }
            // If choose exit -> simply return Ok (cancel preview)
            if idx == choices_len - 1 {
                crate::utils::Utils::info(&trf!("init.interactive.preview_cancelled"));
                return Ok(());
            }
            // If an actual entry is selected
            if idx < entries.len() {
                let (_label, rel_path, is_file) = &entries[idx];
                let full_path = template_dir.join(rel_path);
                if !is_file {
                    // Directory selected: list immediate children as a small preview
                    println!(
                        "\n{}",
                        trf!(
                            "init.interactive.directory_preview_header",
                            rel_path.display()
                        )
                        .cyan()
                    );
                    if let Ok(children) = fs::read_dir(&full_path) {
                        let mut cvec: Vec<_> = children.filter_map(Result::ok).collect();
                        cvec.sort_by_key(std::fs::DirEntry::path);
                        for c in cvec.iter().take(50) {
                            if let Some(name) = c.path().file_name().and_then(|s| s.to_str()) {
                                if c.path().is_dir() {
                                    // Directory entries show an icon and cyan color
                                    println!("  - 📁 {}/", name.cyan());
                                } else {
                                    // Files show a file icon and dimmed color
                                    println!("  - 📄 {}", name.dimmed());
                                }
                            }
                        }
                        println!("{}\n", trf!("init.interactive.end_of_directory_preview"));
                        // Continue loop for further selection
                        continue;
                    }
                    // If we reach here, reading the directory failed — fall through to file preview
                    println!("{}\n", trf!("init.interactive.preview_failed_read_dir"));
                }
                // Show file preview (first N lines)
                match fs::read_to_string(&full_path) {
                    Ok(content) => {
                        let lines: Vec<&str> = content.lines().take(20).collect();
                        println!(
                            "{}",
                            trf!("init.interactive.preview_file_header", rel_path.display()).bold()
                        );
                        for l in lines {
                            println!("{}", l.dimmed());
                        }
                    }
                    Err(_) => {
                        println!("{}", trf!("init.interactive.preview_failed_read_file"));
                    }
                }

                // After showing preview, display end marker and ask whether to preview another file
                println!(
                    "{}\n",
                    trf!("init.interactive.preview_end", rel_path.display())
                );

                // Ask whether to preview another file (interactive)
                let cont = match Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(&trf!("init.interactive.preview_another_file"))
                    .default(true)
                    .interact()
                {
                    Ok(v) => v,
                    Err(_) => {
                        prompt_confirm(&trf!("init.interactive.preview_another_file"), false)?
                    }
                };
                if cont {
                    continue;
                }
                break;
            }
        } else {
            // Non-interactive: fallback to simple listing (no arrows)
            crate::utils::Utils::section(&trf!(
                "init.interactive.template_contents_showing_up_to_files",
                limit
            ));
            for (i, (_, rel, is_file)) in entries.iter().enumerate().take(limit) {
                let suffix = if *is_file { "" } else { "/" };
                println!("  {}) {}{}", i + 1, rel.display(), suffix);
            }
            if entries.len() > limit {
                println!(
                    "{}",
                    trf!("init.interactive.and_more_files", entries.len() - limit)
                );
            }
            println!();
            return Ok(());
        }
    }

    Ok(())
}

