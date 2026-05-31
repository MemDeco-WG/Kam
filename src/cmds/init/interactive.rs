use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use walkdir::WalkDir;

use crate::assets::tmpl::TmplAssets;
use crate::cmds::tmpl::pull;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;
use crate::{template::TemplateCacheManager, template::TemplateManager};
use colored::Colorize;
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};

use super::args::InitArgs;
use super::{impl_mod, post_init, pre_init};

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

    // Fallback: standard input (non-TTY or dialoguer failure)
    if default_str.is_empty() {
        print!("{prompt}: ");
    } else {
        print!("{prompt} [{default_str}]: ");
    }
    io::stdout().flush().map_err(KamError::Io)?;
    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(KamError::Io)?;
    let input = input.trim();
    if input.is_empty() {
        Ok(default_str)
    } else {
        Ok(input.to_string())
    }
}

fn prompt_confirm(prompt: &str, default: bool) -> Result<bool, KamError> {
    // Try the dialoguer Confirm for a nicer TTY experience; if it fails (non-TTY),
    // fall back to a simple text-based confirmation prompt.
    if let Ok(v) = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(default)
        .interact()
    {
        Ok(v)
    } else {
        let suffix = if default { "[Y/n]" } else { "[y/N]" };
        loop {
            print!("{prompt} {suffix}: ");
            io::stdout().flush().map_err(KamError::Io)?;
            let mut input = String::new();
            io::stdin().read_line(&mut input).map_err(KamError::Io)?;
            let trimmed = input.trim().to_lowercase();
            if trimmed.is_empty() {
                return Ok(default);
            }
            match trimmed.as_str() {
                "y" | "yes" => return Ok(true),
                "n" | "no" => return Ok(false),
                _ => {
                    println!("{}", trf!("init.interactive.enter_yes_no"));
                }
            }
        }
    }
}

// 显示模板列表让用户选一个
// 如果默认模板存在就默认选中它
#[allow(clippy::too_many_lines)] // TODO: consider breaking this function into smaller units
fn choose_template(default_template: &str) -> Result<String, KamError> {
    loop {
        let mut choices = TemplateManager::list_builtin_templates();
        choices.sort();
        choices.dedup();
        // 加两个特殊选项：本地路径和在线拉取
        choices.push(trf!("init.interactive.choice_local_path"));
        choices.push(trf!("init.interactive.choice_pull_default_templates"));

        // 计算默认选中项，如果默认模板在列表里就选它
        let default_idx = choices
            .iter()
            .position(|t| t == default_template)
            .unwrap_or(0);

        // 先试试交互式选择（用方向键），如果不是TTY就回退到文本输入
        if let Ok(idx) = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(&trf!("init.interactive.choose_template"))
            .items(&choices[..])
            .default(default_idx)
            .interact()
        {
            let sel = &choices[idx];
            if sel == &trf!("init.interactive.choice_pull_default_templates") {
                // 从配置的URL拉取默认模板
                pull::run_pull(None, true, false)?;
                // 刷新列表再显示一次
                continue;
            }
            if sel == &trf!("init.interactive.choice_local_path") {
                // 让用户输入本地路径
                let input = Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt(&trf!("init.interactive.enter_local_template_path"))
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|e| KamError::Io(e.into()))?;
                let input_trim = input.trim();
                if input_trim.is_empty() {
                    // 如果留空，就下载默认模板然后重新显示菜单
                    pull::run_pull(None, true, false)?;
                    continue;
                }

                let p = Path::new(&input);
                if !p.exists() {
                    return Err(KamError::InvalidDirectory(format!(
                        "Local template path not found: {input}"
                    )));
                }
                if p.is_file() {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        // If template already exists in cache, reuse it instead of reinstalling
                        if let Ok(Some(_existing)) =
                            TemplateCacheManager::resolve_template_path(stem)
                        {
                            return Ok(stem.to_string());
                        }
                        // Otherwise install the local archive into the cache
                        TemplateCacheManager::install_template(stem, p)?;
                        return Ok(stem.to_string());
                    }
                    // return the provided path string if we couldn't derive a stem
                    return Ok(input.clone());
                }
                // Directory path - use it directly
                return Ok(input.clone());
            }
            return Ok(choices[idx].clone());
        }

        // Fallback to previous text-based interaction
        crate::utils::Utils::info(&trf!("init.interactive.non_interactive_fallback"));
        loop {
            let pick = prompt_input(
                &trf!("init.interactive.select_by_name_or_number"),
                Some(default_template),
            )?;
            if pick.trim().is_empty() {
                // Empty text input: download the default templates and refresh choices
                crate::cmds::tmpl::pull::run_pull(None, true, false)?;
                break; // Exit the fallback prompt and let the outer loop refresh the choices
            }
            if let Ok(num) = pick.parse::<usize>() {
                if num == 0 {
                    let p = prompt_input(
                        &trf!("init.interactive.enter_path_to_local_template"),
                        None::<&str>,
                    )?;
                    if !p.trim().is_empty() {
                        return Ok(p);
                    }
                } else if num > 0 && num <= choices.len() {
                    return Ok(choices[num - 1].clone());
                } else {
                    crate::utils::Utils::warn(&trf!("init.interactive.invalid_selection", num));
                }
            } else {
                let pick_trim = pick.trim();
                if !pick_trim.is_empty() {
                    // If it's a path and exists, handle it as local template path
                    let p = Path::new(pick_trim);
                    if p.exists() {
                        // try to install to cache if path exists
                        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                            TemplateCacheManager::install_template(stem, p)?;
                            return Ok(stem.to_string());
                        }
                        return Ok(pick_trim.to_string());
                    }
                    // Otherwise, assume it's a template name
                    return Ok(pick_trim.to_string());
                }
            }
        }
    }
}
// 解压压缩包到指定目录
// 支持tar.gz、tgz、zip格式
fn extract_archive(path: &Path, dst: &Path) -> Result<(), KamError> {
    let file = fs::File::open(path).map_err(KamError::Io)?;
    // normalize the file name and prefer extension checks
    let file_name_lower = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if file_name_lower.ends_with(".tar.gz")
        || path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("tgz"))
    {
        let tar = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(tar);
        archive
            .unpack(dst)
            .map_err(|e| KamError::Io(std::io::Error::other(e)))?;
    } else if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        let mut zip_arch = zip::ZipArchive::new(file).map_err(KamError::Zip)?;
        zip_arch.extract(dst).map_err(KamError::Zip)?;
    } else {
        return Err(KamError::UnsupportedArchive(format!(
            "Unsupported archive: {path_display}",
            path_display = path.display()
        )));
    }
    Ok(())
}

// 解析模板源，找到包含kam.toml的路径
// 返回 (kam_toml_path, optional_tempdir)
// 如果模板需要解压到临时目录，就返回TempDir（这样它不会被drop）
// 否则返回None，kam_toml_path指向磁盘上的现有路径
fn resolve_template_kam_toml(template_spec: &str) -> Result<(PathBuf, Option<TempDir>), KamError> {
    let try_specs = {
        let mut v = vec![template_spec.to_string()];
        let is_archive_or_path = template_spec.contains('/')
            || template_spec.contains('\\')
            || template_spec.to_ascii_lowercase().ends_with(".tar.gz")
            || std::path::Path::new(template_spec)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("tgz"))
            || std::path::Path::new(template_spec)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));
        if !template_spec.ends_with("_template") && !is_archive_or_path {
            v.push(format!("{template_spec}_template"));
        }
        v
    };

    for spec in try_specs {
        // 1. If raw input is a path
        let spec_path = Path::new(&spec);
        if spec_path.exists() {
            if spec_path.is_dir() {
                let k = spec_path.join("kam.toml");
                if k.exists() {
                    return Ok((k, None));
                }
                // Could be a template root without kam.toml (unlikely) — treat as error
                return Err(KamError::InvalidDirectory(format!(
                    "Template '{spec_path}' is a directory but no kam.toml found",
                    spec_path = spec_path.display()
                )));
            } else if spec_path.is_file() {
                let tmp = tempfile::tempdir().map_err(KamError::Io)?;
                extract_archive(spec_path, tmp.path())?;
                // Find kam.toml in the extracted temp
                let kt_path = tmp.path().join("kam.toml");
                if kt_path.exists() {
                    return Ok((kt_path, Some(tmp)));
                }
                // maybe templates are under single nested dir; attempt to descend
                // find the first directory that contains kam.toml
                for entry in fs::read_dir(tmp.path()).map_err(KamError::Io)? {
                    let entry = entry.map_err(KamError::Io)?;
                    let p = entry.path();
                    let candidate = p.join("kam.toml");
                    if candidate.exists() {
                        return Ok((candidate, Some(tmp)));
                    }
                }
                return Err(KamError::InvalidDirectory(format!(
                    "Archive '{spec}' did not contain kam.toml"
                )));
            }
        }

        // 2. Check cached template path
        if let Ok(Some(cached_path)) = TemplateCacheManager::resolve_template_path(&spec) {
            if cached_path.is_dir() {
                let k = cached_path.join("kam.toml");
                if k.exists() {
                    return Ok((k, None));
                }
                return Err(KamError::InvalidDirectory(format!(
                    "Cached template '{spec}' has no kam.toml"
                )));
            } else if cached_path.is_file() {
                let tmp = tempfile::tempdir().map_err(KamError::Io)?;
                extract_archive(&cached_path, tmp.path())?;
                let kt_path = tmp.path().join("kam.toml");
                if kt_path.exists() {
                    return Ok((kt_path, Some(tmp)));
                }
            }
        }

        // 3. Check built-in assets (assets are .tar.gz)
        {
            let asset_name = format!("{spec}.tar.gz");
            if let Some(asset) = TmplAssets::get(&asset_name) {
                // create temp file and extract
                let tmp = tempfile::tempdir().map_err(KamError::Io)?;
                let bin_path = tmp.path().join(asset_name);
                fs::write(&bin_path, &asset.data).map_err(KamError::Io)?;
                extract_archive(&bin_path, tmp.path())?;
                // Kam file could be in root or inside top-level dir
                let kt_path = tmp.path().join("kam.toml");
                if kt_path.exists() {
                    return Ok((kt_path, Some(tmp)));
                }
                for entry in fs::read_dir(tmp.path()).map_err(KamError::Io)? {
                    let entry = entry.map_err(KamError::Io)?;
                    let p = entry.path();
                    let candidate = p.join("kam.toml");
                    if candidate.exists() {
                        return Ok((candidate, Some(tmp)));
                    }
                }
                return Err(KamError::InvalidDirectory(format!(
                    "Built-in asset '{spec}' did not contain kam.toml"
                )));
            }
        }
    } // end for potentials

    Err(KamError::TemplateNotFound(format!(
        "Template '{template_spec}' not found in built-in assets, cache, or local path."
    )))
}

// 提示用户输入模板定义的变量（根据kam.toml的定义）
// 返回变量名到值的HashMap（覆盖默认值）
// 这个函数会交互式地询问用户，所以如果非TTY可能会出问题
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

fn save_global_config(
    author: Option<&str>,
    name: Option<&str>,
    version: Option<&str>,
) -> Result<(), KamError> {
    // Use `toml` to update or create a global config at Kam's home directory (defaults to ~/.kam/config.toml).
    // The `KAM_HOME` environment variable can override the Kam home directory.
    let config_dir = crate::utils::kam_home_dir()?;
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(KamError::Io)?;
    }
    let cfg_path = config_dir.join("config.toml");

    // Parse existing TOML if present; else start with an empty table
    let mut doc_table: toml::value::Table = if cfg_path.exists() {
        let s = fs::read_to_string(&cfg_path).map_err(KamError::Io)?;
        let val: toml::Value = toml::from_str(&s).map_err(|e| {
            KamError::CommandFailed(format!("Failed to parse existing config: {e}"))
        })?;
        match val {
            toml::Value::Table(t) => t,
            _ => toml::value::Table::new(),
        }
    } else {
        toml::value::Table::new()
    };

    // Ensure we have a [prop] table and update keys using get_mut to avoid Entry type mismatches
    if let Some(val) = doc_table.get_mut("prop") {
        if let toml::Value::Table(prop_table) = val {
            if let Some(a) = author {
                prop_table.insert("author".to_string(), toml::Value::String(a.to_string()));
            }
            if let Some(n) = name {
                prop_table.insert("name".to_string(), toml::Value::String(n.to_string()));
            }
            if let Some(v) = version {
                prop_table.insert("version".to_string(), toml::Value::String(v.to_string()));
            }
        } else {
            // Replace non-table 'prop' value with a proper table containing our values
            let mut prop_tbl = toml::value::Table::new();
            if let Some(a) = author {
                prop_tbl.insert("author".to_string(), toml::Value::String(a.to_string()));
            }
            if let Some(n) = name {
                prop_tbl.insert("name".to_string(), toml::Value::String(n.to_string()));
            }
            if let Some(v) = version {
                prop_tbl.insert("version".to_string(), toml::Value::String(v.to_string()));
            }
            *val = toml::Value::Table(prop_tbl);
        }
    } else {
        let mut prop_tbl = toml::value::Table::new();
        if let Some(a) = author {
            prop_tbl.insert("author".to_string(), toml::Value::String(a.to_string()));
        }
        if let Some(n) = name {
            prop_tbl.insert("name".to_string(), toml::Value::String(n.to_string()));
        }
        if let Some(v) = version {
            prop_tbl.insert("version".to_string(), toml::Value::String(v.to_string()));
        }
        doc_table.insert("prop".to_string(), toml::Value::Table(prop_tbl));
    }

    // Serialize and write back to file
    let final_value = toml::Value::Table(doc_table);
    let out = toml::to_string_pretty(&final_value)
        .map_err(|e| KamError::CommandFailed(format!("Failed to serialize config: {e}")))?;
    fs::write(cfg_path, out).map_err(KamError::Io)?;
    Ok(())
}

/// Run the interactive init flow.
///
/// # Errors
///
/// Returns `Err(KamError)` if initialization fails (I/O, rendering, or user input errors).
#[allow(clippy::too_many_lines)]
pub fn run(args: &InitArgs) -> Result<(), KamError> {
    Utils::banner(crate::i18n::tr("init.interactive.title"));
    Utils::info(crate::i18n::tr("init.interactive.press_enter"));
    println!();

    // Prepare defaults (non-interactive sanity pass)
    let mut data = match pre_init::prepare_init(args) {
        Ok(d) => d,
        Err(e) => {
            Utils::error(format!("Failed to prepare initialization defaults: {e}"));
            return Err(e);
        }
    };

    // Template selection
    let chosen_template = choose_template(&data.impl_template)?;
    data.impl_template = chosen_template;

    // Path selection: allow changing target path
    let cur_path_str = data.path.to_string_lossy().to_string();
    let new_path_str = prompt_input(
        &trf!("init.interactive.path_to_create_project"),
        Some(&cur_path_str),
    )?;
    if new_path_str != cur_path_str {
        // Normalize to an absolute path, respecting '.' and relative paths
        let final_path = if new_path_str.trim().is_empty() || new_path_str == "." {
            std::env::current_dir().map_err(KamError::Io)?
        } else {
            let candidate = PathBuf::from(&new_path_str);
            if candidate.is_relative() {
                std::env::current_dir()
                    .map_err(KamError::Io)?
                    .join(candidate)
            } else {
                candidate
            }
        };
        data.path = final_path;

        // If the user didn't pass --id explicitly, recompute `data.id`
        // to match the final folder basename. If the inferred id contains
        // invalid characters, prompt the user to accept a sanitized suggestion
        // or input a valid custom ID.
        if args.id.is_none()
            && let Some(basename) = data.path.file_name().and_then(|s| s.to_str())
        {
            let candidate = basename.to_string();
            // Check ID validity (alphanumeric, '.', '-', '_')
            if candidate
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
            {
                data.id = candidate;
            } else {
                Utils::warn(&trf!("init.interactive.inferred_id_invalid", candidate));

                // Suggest a sanitized version (replace spaces with underscore)
                let suggested = candidate.replace(' ', "_");
                if suggested
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
                {
                    if prompt_confirm(
                        &trf!("init.interactive.use_sanitized_module_id", suggested),
                        true,
                    )? {
                        data.id = suggested;
                    } else {
                        // Ask the user to input a valid ID
                        let id_input = prompt_input(
                            &trf!("init.interactive.module_id_prompt"),
                            Some(&data.id),
                        )?;
                        data.id = id_input;
                        if !data
                            .id
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
                        {
                            return Err(KamError::InvalidConfig(trf!(
                                "init.interactive.invalid_module_id",
                                data.id
                            )));
                        }
                    }
                } else {
                    // If we couldn't create a valid suggestion, force the user to input
                    let id_input = prompt_input("Module ID", Some(&data.id))?;
                    data.id = id_input;
                    if !data
                        .id
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
                    {
                        return Err(KamError::InvalidConfig(format!(
                            "Invalid module ID '{}': ID must contain only alphanumeric characters, dots, dashes, and underscores",
                            data.id
                        )));
                    }
                }
            }
        }
    }

    // Ensure module id matches path name
    let path_basename = data
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&data.id)
        .to_string();
    if path_basename != data.id {
        Utils::warn(&trf!(
            "init.interactive.module_id_mismatch",
            data.id,
            path_basename
        ));
        if prompt_confirm(&trf!("init.interactive.set_module_id_to_basename"), true)? {
            data.id = path_basename;
        } else {
            // Let user input a custom id
            let id_input =
                prompt_input(&trf!("init.interactive.module_id_prompt"), Some(&data.id))?;
            data.id = id_input;
            // re-validate
            if !data
                .id
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
            {
                return Err(KamError::InvalidConfig(trf!(
                    "init.interactive.invalid_module_id",
                    data.id
                )));
            }
        }
    }

    // Base params (name, version, author, description)
    data.name = prompt_input(&trf!("init.interactive.project_name"), Some(&data.name))?;
    data.version = prompt_input(&trf!("init.interactive.version"), Some(&data.version))?;
    data.author = prompt_input(&trf!("init.interactive.author"), Some(&data.author))?;
    data.description = prompt_input(
        &trf!("init.interactive.description"),
        Some(&data.description),
    )?;

    // Ask to persist defaults to global config (~/.kam/config.toml)
    if prompt_confirm(&trf!("init.interactive.save_base_values"), false)? {
        save_global_config(Some(&data.author), Some(&data.name), Some(&data.version))?;
        Utils::success(&trf!("init.interactive.saved_base_values"));
    }

    // Template variables: find template kam.toml then prompt for variables
    let (kt_path, maybe_tmp) = resolve_template_kam_toml(&data.impl_template)?;
    // `maybe_tmp` kept here to keep tempdir alive while we inspect its files
    if let Some(tmp) = &maybe_tmp {
        Utils::info(&trf!(
            "init.interactive.loaded_template_from_temp",
            tmp.path().display()
        ));
    }

    // Ensure data.template_vars contains default fallbacks from pre_init
    // Now prompt for each variable defined in template
    prompt_template_variables(&kt_path, &mut data.template_vars)?;

    // Optional: visualize the template contents to the user
    let template_dir = kt_path.parent().unwrap_or_else(|| Path::new("."));
    let _ = visualize_template(template_dir, 50);

    // Check for gh and cz presence, ask to install if missing
    // GH
    let gh_present = std::process::Command::new("gh")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !gh_present && prompt_confirm(&trf!("init.interactive.gh_not_found"), true)? {
        Utils::info(&trf!("init.interactive.recommend_github_cli"));
        Utils::info(&trf!("init.interactive.helper_script"));
    }

    // cz (commitizen)
    let cz_present = std::process::Command::new("cz")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !cz_present && prompt_confirm(&trf!("init.interactive.cz_not_found"), false)? {
        Utils::info(&trf!("init.interactive.recommend_cz_install"));
        Utils::info(&trf!("init.interactive.helper_script"));
    }

    // Present a summary and confirm before creating files
    println!();
    Utils::section("init.interactive.summary_title");
    Utils::kv(
        "init.interactive.summary.path",
        data.path.display().to_string(),
    );
    Utils::kv("init.interactive.summary.module_id", &data.id);
    Utils::kv("init.interactive.summary.project_name", &data.name);
    Utils::kv("init.interactive.summary.version", &data.version);
    Utils::kv("init.interactive.summary.author", &data.author);
    Utils::kv("init.interactive.summary.template", &data.impl_template);
    if !data.template_vars.is_empty() {
        println!(
            "  {} {}",
            "•".cyan(),
            crate::i18n::tr("init.interactive.template_variables").bold()
        );
        for (k, v) in &data.template_vars {
            println!("    {} {} = {}", "→".blue().dimmed(), k.bold(), v.dimmed());
        }
    }

    if !prompt_confirm(&trf!("init.interactive.confirm_proceed_create"), true)? {
        Utils::warn(&trf!("init.interactive.aborted"));
        return Err(KamError::CommandFailed(
            "User aborted interactive init".to_string(),
        ));
    }

    // Merge into `Vec<String>` for init_template
    let mut merged_var_vec: Vec<String> = data
        .template_vars
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    merged_var_vec.sort();

    // Call the same init_template as non-interactive run
    impl_mod::init_template(
        &data.path,
        &impl_mod::InitTemplateParams {
            id: &data.id,
            name: data.name.clone(),
            version: &data.version,
            author: &data.author,
            description: data.description.clone(),
            var: &merged_var_vec,
            impl_template: Some(data.impl_template.clone()),
            force: args.force,
            module_type: data.module_type,
            update_json: data.update_json.clone(),
        },
    )?;

    // Post-process
    post_init::post_process(&data.path)?;

    Utils::success(crate::i18n::tr("init.interactive.completed_successfully"));
    println!();
    Utils::info(crate::i18n::tr("init.interactive.next_steps"));
    println!("  {} cd {}", "→".blue(), data.path.display());
    println!("  {} kam build", "→".blue());
    Ok(())
}
