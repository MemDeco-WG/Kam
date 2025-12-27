use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use regex::Regex;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Serialize, Debug)]
pub struct FileResult {
    pub path: String,
    pub kind: String,
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub fixed: bool,
}

// 检查并可选地修复结构化数据文件（JSON、YAML、TOML）的辅助函数
// 统一了解析和重新格式化的逻辑，避免重复代码
fn check_structured_format<F, G>(
    content: &str,
    path: &Path,
    do_fix: bool,
    parse_fn: F,
    format_fn: G,
    format_name: &str,
) -> Result<(bool, bool), String>
where
    F: Fn(&str) -> Result<(), String>,
    G: Fn(&str) -> Result<String, String>,
{
    match parse_fn(content) {
        Ok(_) => {
            // 解析成功，如果要求修复就重新格式化
            if do_fix {
                match format_fn(content) {
                    Ok(pretty) => {
                        // 如果格式化后的内容和原内容不同，就写回去
                        if pretty != content {
                            fs::OpenOptions::new()
                                .write(true)
                                .truncate(true)
                                .open(path)
                                .map_err(|e| format!("Failed to open file: {}", e))?
                                .write_all(pretty.as_bytes())
                                .map_err(|e| format!("Failed to write file: {}", e))?;
                            return Ok((true, true)); // 有效且已修复
                        }
                    }
                    Err(e) => {
                        return Err(format!("Failed to format {}: {}", format_name, e));
                    }
                }
            }
            Ok((true, false)) // 有效但未修复
        }
        Err(e) => Err(format!("{} parse error: {}", format_name, e)),
    }
}

pub fn check_file(
    path: &Path,
    kind: &str,
    do_fix: bool,
    rules_cfg: Option<&std::collections::HashMap<String, crate::types::kam_toml::RuleConfig>>,
) -> Result<FileResult, KamError> {
    let mut s = fs::read_to_string(path)?;
    let mut fr = FileResult {
        path: path.to_string_lossy().to_string(),
        kind: kind.to_string(),
        valid: true,
        errors: Vec::new(),
        warnings: Vec::new(),
        fixed: false,
    };

    match kind {
        "json" => {
            let parse_fn = |s: &str| -> Result<(), String> {
                serde_json::from_str::<serde_json::Value>(s)
                    .map_err(|e| e.to_string())
                    .map(|_| ())
            };
            let format_fn = |s: &str| -> Result<String, String> {
                let v = serde_json::from_str::<serde_json::Value>(s).map_err(|e| e.to_string())?;
                serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
            };
            match check_structured_format(&s, path, do_fix, parse_fn, format_fn, "JSON") {
                Ok((valid, fixed)) => {
                    fr.valid = valid;
                    if fixed {
                        // Reload the newly formatted content so subsequent checks operate
                        // on the canonicalized content.
                        s = fs::read_to_string(path)?;
                        fr.fixed = true;
                    }
                }
                Err(e) => {
                    fr.valid = false;
                    fr.errors.push(e);
                }
            }
        }
        "yaml" => {
            let parse_fn = |s: &str| -> Result<(), String> {
                serde_yaml::from_str::<serde_yaml::Value>(s)
                    .map_err(|e| e.to_string())
                    .map(|_| ())
            };
            let format_fn = |s: &str| -> Result<String, String> {
                let v = serde_yaml::from_str::<serde_yaml::Value>(s).map_err(|e| e.to_string())?;
                serde_yaml::to_string(&v).map_err(|e| e.to_string())
            };
            match check_structured_format(&s, path, do_fix, parse_fn, format_fn, "YAML") {
                Ok((valid, fixed)) => {
                    fr.valid = valid;
                    if fixed {
                        // Reload the newly formatted content so subsequent checks operate
                        // on the canonicalized content.
                        s = fs::read_to_string(path)?;
                        fr.fixed = true;
                    }
                }
                Err(e) => {
                    fr.valid = false;
                    fr.errors.push(e);
                }
            }
        }
        "toml" => {
            // Ensure UNIX LF line endings are used in TOML files.
            // If CR or CRLF are found, report an error; if --fix is requested, normalize to LF.
            if s.contains('\r') {
                fr.valid = false;
                fr.errors.push(
                    "TOML files must use UNIX (LF) line endings; please convert CRLF/CR to LF"
                        .to_string(),
                );
                if do_fix {
                    let normalized = s.replace("\r\n", "\n").replace('\r', "\n");
                    fs::OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(path)?
                        .write_all(normalized.as_bytes())?;
                    // Update our working content so subsequent checks operate on normalized content
                    s = normalized;
                    fr.fixed = true;
                }
            }

            let parse_fn = |s: &str| -> Result<(), String> {
                toml::from_str::<toml::Value>(s)
                    .map_err(|e| {
                        // 尝试提取行号信息
                        extract_line_number(&e.to_string(), s)
                    })
                    .map(|_| ())
            };
            let format_fn = |s: &str| -> Result<String, String> {
                let v = toml::from_str::<toml::Value>(s)
                    .map_err(|e| extract_line_number(&e.to_string(), s))?;
                toml::to_string_pretty(&v).map_err(|e| e.to_string())
            };
            match check_structured_format(&s, path, do_fix, parse_fn, format_fn, "TOML") {
                Ok((valid, fixed)) => {
                    fr.valid = valid;
                    if fixed {
                        // Reload the newly formatted content so subsequent checks operate
                        // on the canonicalized content (important for subsequent deep checks).
                        s = fs::read_to_string(path)?;
                        fr.fixed = true;
                    }
                }
                Err(e) => {
                    fr.valid = false;
                    fr.errors.push(e);
                }
            }
            // 如果是 kam.toml 文件，进行深度检查
            if path.file_name().and_then(|n| n.to_str()) == Some("kam.toml") {
                check_kam_toml_deep(path, &mut fr);
            }
        }
        "sh" => {
            // Delegated to check_sh in sh.rs. If shellcheck succeeds we still want
            // to apply our Rust-based rules on top of the shellcheck result so that
            // rules from rules.d/ are enforced regardless of shellcheck availability.
            match super::sh::check_sh(path, do_fix) {
                Ok(mut p) => {
                    // Apply rules to the shellcheck result (mutating its warnings/errors)
                    crate::rules::apply_all_rules(path, &s, &mut p);
                    return Ok(p);
                }
                Err(e) => {
                    fr.warnings.push(format!("sh check failed: {}", e));
                }
            }
        }
        "markdown" => {
            // markdown文件：规范化换行符、去除行尾空格、确保文件末尾有换行
            // 主要是为了统一格式，避免git diff时出现不必要的变更
            if do_fix {
                let mut normalized = s.replace("\r\n", "\n");
                // Replace remaining CR if any
                normalized = normalized.replace("\r", "\n");

                // Remove trailing spaces from each line
                let lines: Vec<&str> = normalized.lines().collect();
                let stripped: Vec<String> =
                    lines.iter().map(|l| l.trim_end().to_string()).collect();
                normalized = stripped.join("\n");

                // Ensure final newline
                if !normalized.ends_with('\n') {
                    normalized.push('\n');
                }

                if normalized != s {
                    fs::OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(path)?
                        .write_all(normalized.as_bytes())?;
                    // Update in-memory content so subsequent rules operate on the fixed content
                    s = normalized;
                    fr.fixed = true;
                }
            }
        }
        _ => {}
    }

    // Apply rules as a final pass so that file-level rules (rules.d/*) can
    // append warnings/errors irrespective of the structured checks above.
    //
    // Run rules and allow optional in-memory fixes. When `do_fix` is true
    // these will be written back to the file and `fr.fixed` will be set.
    let new_s = crate::rules::apply_all_rules_with_fix(path, &s, &mut fr, do_fix, rules_cfg);
    if do_fix && new_s != s {
        fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)?
            .write_all(new_s.as_bytes())?;
        fr.fixed = true;
    }
    Ok(fr)
}

#[cfg(test)]
mod format_fix_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn json_fix_formats_file() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("t.json");
        fs::write(&p, r#"{"b":2,"a":1}"#).unwrap();

        let res = check_file(&p, "json", true, None).unwrap();
        assert!(res.fixed);
        let content = fs::read_to_string(&p).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let pretty = serde_json::to_string_pretty(&v).unwrap();
        assert_eq!(content, pretty);
    }

    #[test]
    fn yaml_fix_formats_file() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("y.yaml");
        let original = "a: 1\nb:\n  - x\n";
        fs::write(&p, original).unwrap();

        let res = check_file(&p, "yaml", true, None).unwrap();
        let after = fs::read_to_string(&p).unwrap();
        let v: serde_yaml::Value = serde_yaml::from_str(original).unwrap();
        let formatted = serde_yaml::to_string(&v).unwrap();
        assert_eq!(after, formatted);
        let expected_fixed = original != formatted;
        assert_eq!(
            res.fixed, expected_fixed,
            "fixed flag should match whether content changed"
        );
    }

    #[test]
    fn toml_fix_formats_file() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("t.toml");
        let original = "title = \"A\"\n[package]\nname = \"t\"\n";
        fs::write(&p, original).unwrap();

        let res = check_file(&p, "toml", true, None).unwrap();
        let after = fs::read_to_string(&p).unwrap();
        let v: toml::Value = toml::from_str(original).unwrap();
        let formatted = toml::to_string_pretty(&v).unwrap();
        assert_eq!(after, formatted);
        let expected_fixed = original != formatted;
        assert_eq!(
            res.fixed, expected_fixed,
            "fixed flag should match whether content changed"
        );
    }
}

/// 对 kam.toml 文件进行深度检查
/// 包括：字段验证、文件引用检查、版本号格式验证等
fn check_kam_toml_deep(path: &Path, fr: &mut FileResult) {
    let project_dir = path.parent().unwrap_or_else(|| Path::new("."));

    // 尝试加载 kam.toml
    match KamToml::load_from_file(path) {
        Ok(kam_toml) => {
            // 1. 检查必需字段
            check_required_fields(&kam_toml, fr);

            // 2. 检查版本号格式
            check_version_format(&kam_toml.prop.version, fr);

            // 3. 检查文件引用
            check_file_references(&kam_toml, project_dir, fr);

            // 4. 检查配置一致性
            check_config_consistency(&kam_toml, project_dir, fr);
        }
        Err(e) => {
            // 如果解析失败，错误已经在基本检查中报告了
            // 这里只添加一个提示
            fr.warnings
                .push(format!("Cannot perform deep validation: {}", e));
        }
    }
}

/// 检查必需字段
fn check_required_fields(kam_toml: &KamToml, fr: &mut FileResult) {
    if kam_toml.prop.id.trim().is_empty() {
        fr.valid = false;
        fr.errors.push("[prop] id is required".to_string());
    } else {
        // Enforce stricter id format: must start with a letter and contain only letters, digits, dot, underscore or hyphen.
        // Regex: ^[a-zA-Z][a-zA-Z0-9._-]+$
        let id_re = Regex::new(r"^[a-zA-Z][a-zA-Z0-9._-]+$").unwrap();
        if !id_re.is_match(&kam_toml.prop.id) {
            fr.valid = false;
            fr.errors
                .push("[prop] id must match regex: ^[a-zA-Z][a-zA-Z0-9._-]+$".to_string());
        }
    }

    if kam_toml.prop.name.trim().is_empty() {
        fr.valid = false;
        fr.errors.push("[prop] name is required".to_string());
    }

    if kam_toml.prop.version.trim().is_empty() {
        fr.valid = false;
        fr.errors.push("[prop] version is required".to_string());
    }

    if kam_toml.prop.versionCode <= 0 {
        fr.valid = false;
        fr.errors
            .push("[prop] versionCode must be a positive integer".to_string());
    }

    if kam_toml.prop.description.trim().is_empty() {
        fr.valid = false;
        fr.errors.push("[prop] description is required".to_string());
    }

    // author 是可选的，但建议填写
    if kam_toml
        .prop
        .author
        .as_ref()
        .map(|a| a.trim().is_empty())
        .unwrap_or(true)
    {
        fr.warnings
            .push("[prop] author is empty (recommended to fill)".to_string());
    }
}

/// 检查版本号格式（语义化版本规范）
fn check_version_format(version: &str, fr: &mut FileResult) {
    if version.trim().is_empty() {
        return; // 已经在必需字段检查中处理
    }

    let s = version.trim();

    // 不允许多重前缀 'vv'（例如 vv1.2.3）
    if s.to_lowercase().starts_with("vv") {
        fr.valid = false;
        fr.errors.push(format!(
            "[prop] version '{}' is invalid: leading 'vv' is not allowed (expected format: vX.Y.Z)",
            version
        ));
        return;
    }

    // 要求以小写或大写 'v' 前缀开始（例如 v1.2.3）
    if !(s.starts_with('v') || s.starts_with('V')) {
        fr.valid = false;
        fr.errors.push(format!(
            "[prop] version '{}' must start with 'v' (expected format: vX.Y.Z)",
            version
        ));
        return;
    }

    // 去掉可选的前缀 'v' 或 'V' 并使用 semver 解析剩余部分
    let naked = &s[1..];
    if semver::Version::parse(naked).is_err() {
        fr.valid = false;
        fr.errors.push(format!(
            "[prop] version '{}' is not a valid semantic version (expected: vX.Y.Z, optionally with pre-release/build metadata)",
            version
        ));
    }
}

/// 检查文件引用是否存在
fn check_file_references(kam_toml: &KamToml, project_dir: &Path, fr: &mut FileResult) {
    // 检查 [mmrl.repo] 中的文件引用
    if let Some(mmrl) = &kam_toml.mmrl
        && let Some(repo) = &mmrl.repo
    {
        check_file_exists(
            project_dir,
            &repo.license_file,
            "[mmrl.repo] license_file",
            fr,
        );
        check_file_exists(
            project_dir,
            &repo.readme_file,
            "[mmrl.repo] readme_file",
            fr,
        );
        check_file_exists(
            project_dir,
            &repo.changelog_file,
            "[mmrl.repo] changelog_file",
            fr,
        );

        // 检查图标文件
        if let Some(icon) = &repo.icon
            && !icon.is_empty()
            && !project_dir.join(icon).exists()
        {
            fr.warnings
                .push(format!("[mmrl.repo] icon file '{}' not found", icon));
        }
    }
}

/// 检查单个文件是否存在
fn check_file_exists(base: &Path, file: &Option<String>, name: &str, fr: &mut FileResult) {
    if let Some(f) = file
        && !f.is_empty()
    {
        let file_path = base.join(f);
        if !file_path.exists() {
            fr.valid = false;
            fr.errors.push(format!("{} '{}' not found", name, f));
        } else if file_path.metadata().map(|m| m.len() == 0).unwrap_or(false) {
            fr.warnings.push(format!("{} '{}' is empty", name, f));
        }
    }
}

/// 检查配置一致性
fn check_config_consistency(kam_toml: &KamToml, project_dir: &Path, fr: &mut FileResult) {
    // 检查源码目录
    if let Some(build) = &kam_toml.kam.build {
        if let Some(source_dir) = &build.source_dir {
            let src_path = project_dir.join(source_dir);
            if !src_path.exists() {
                fr.warnings.push(format!(
                    "[kam.build] source_dir '{}' does not exist",
                    source_dir
                ));
            }
        } else {
            // 使用默认路径
            let default_src = project_dir.join("src").join(&kam_toml.prop.id);
            if !default_src.exists() {
                fr.warnings.push(format!(
                    "[kam.build] default source directory '{}' does not exist",
                    default_src.display()
                ));
            }
        }

        // 检查 hooks 目录
        if let Some(hooks_dir) = &build.hooks_dir
            && hooks_dir != "hooks"
        {
            let hooks_path = project_dir.join(hooks_dir);
            if !hooks_path.exists() {
                fr.warnings.push(format!(
                    "[kam.build] hooks_dir '{}' does not exist",
                    hooks_dir
                ));
            }
        }
    } else {
        // 没有 build 配置，检查默认源码目录
        let default_src = project_dir.join("src").join(&kam_toml.prop.id);
        if !default_src.exists() {
            fr.warnings.push(format!(
                "Default source directory '{}' does not exist",
                default_src.display()
            ));
        }
    }

    // 检查 [mmrl.repo] 中的 license 字段
    if let Some(mmrl) = &kam_toml.mmrl
        && let Some(repo) = &mmrl.repo
        && repo.license.as_deref().unwrap_or("").is_empty()
    {
        fr.warnings
            .push("[mmrl.repo] license is recommended".to_string());
    }
}

/// 从错误消息中提取行号信息，使错误信息更友好
fn extract_line_number(err_msg: &str, content: &str) -> String {
    // TOML 错误通常包含 "line X" 或 "at line X"
    let line_re = Regex::new(r"(?i)(?:at\s+)?line\s+(\d+)").unwrap();
    if let Some(cap) = line_re.captures(err_msg)
        && let Ok(line_num) = cap[1].parse::<usize>()
    {
        // 尝试获取该行的内容
        if let Some(line_content) = content.lines().nth(line_num.saturating_sub(1)) {
            return format!("{} (line {}: {})", err_msg, line_num, line_content.trim());
        }
        return format!("{} (line {})", err_msg, line_num);
    }
    err_msg.to_string()
}
