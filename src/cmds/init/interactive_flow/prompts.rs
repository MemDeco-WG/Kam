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
use super::{template_init, post_init, pre_init};

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
