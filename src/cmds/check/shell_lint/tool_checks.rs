use crate::cmds::check::file::FileResult;
use crate::errors::KamError;
use regex::Regex;
use serde_json;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use tree_sitter;
use tree_sitter::Parser;
use tree_sitter_bash;

static SETPROP_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"\bsetprop\b").unwrap_or_else(|e| panic!("setprop regex failed to compile: {e}"))
});

fn command_installed(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok()
}

// 检查shell脚本
// 如果有shellcheck就用它（更准确），否则用自定义的Rust检查（简单但够用）
// 另外，对所有脚本统一执行基于文件名的特判和基于 AST 的高危命令检测（补充 shellcheck 的检测）
///
/// # Errors
///
/// Returns `KamError` when file I/O (read/write), invocation of external tools
/// (e.g., `shellcheck`, `shfmt`), or parser/analysis operations fail.
pub fn check_sh(path: &Path, do_fix: bool) -> Result<FileResult, KamError> {
    // 首先运行已有的检查实现（shellcheck 优先，其次自定义）
    let mut fr = if command_installed("shellcheck") {
        match check_sh_with_tool(path, do_fix) {
            Ok(f) => f,
            Err(e) => {
                // 如果 shellcheck 执行失败，生成一个基本结果并记录警告
                FileResult {
                    path: path.to_string_lossy().to_string(),
                    kind: "sh".to_string(),
                    valid: true,
                    errors: Vec::new(),
                    warnings: vec![format!("shellcheck execution failed: {e}")],
                    fixed: false,
                }
            }
        }
    } else {
        check_sh_custom(path, do_fix)?
    };

    // 读取文件内容以便做额外检查（文件名相关检查 / AST 检查 / 换行格式检查等）
    let s = fs::read_to_string(path)?;

    // 基于文件名的特殊规则（例如 install.sh / post-fs-data.sh 的提示）
    apply_sh_filename_checks(path, &s, &mut fr);

    // AST-aware checks for command substitutions, arithmetic expansions, and backticks.
    // Run these even if shellcheck is installed so we catch unbalanced constructs
    // that shellcheck may miss or when shellcheck isn't available.
    for err in detect_unbalanced_shell_constructs(&s) {
        if !fr.errors.contains(&err) {
            fr.valid = false;
            fr.errors.push(err);
        }
    }

    // 基于树形语法树的高危指令检测（如果 parser 可用）
    let mut parser = Parser::new();
    let language = tree_sitter::Language::new(tree_sitter_bash::LANGUAGE);
    if parser.set_language(&language).is_ok()
        && let Some(tree) = parser.parse(&s, None)
    {
        detect_dangerous_commands(tree.root_node(), &s, &mut fr, path);
    }

    // 通用换行规范：确保使用 UNIX LF（如果要求修复，则写回）
    if s.contains('\r') {
        fr.warnings
            .push("CR/CRLF line endings detected; use UNIX LF line endings".to_string());
        if do_fix {
            let normalized = s.replace("\r\n", "\n").replace('\r', "\n");
            fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)?
                .write_all(normalized.as_bytes())?;
            fr.fixed = true;
        }
    }

    Ok(fr)
}

// 用shellcheck工具检查（如果可用）
fn check_sh_with_tool(path: &Path, do_fix: bool) -> Result<FileResult, KamError> {
    let s = fs::read_to_string(path)?;
    let mut fr = FileResult {
        path: path.to_string_lossy().to_string(),
        kind: "sh".to_string(),
        valid: true,
        errors: Vec::new(),
        warnings: Vec::new(),
        fixed: false,
    };
    // 运行shellcheck，输出JSON格式（方便解析）
    match Command::new("shellcheck")
        .arg("-x")
        .arg("--format=json")
        .arg(path)
        .output()
    {
        Ok(output) => {
            // Prefer parsing JSON from stdout; handle both legacy {"comments":[...]} and modern array form.
            if !output.stdout.is_empty() {
                match serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    Ok(v) => {
                        // helper to extract one issue entry into our FileResult
                        let mut push_issue = |c: &serde_json::Value| {
                            let file = c.get("file").and_then(|x| x.as_str()).unwrap_or("");
                            let line = c
                                .get("line")
                                .and_then(serde_json::Value::as_u64)
                                .unwrap_or(0);
                            let column = c
                                .get("column")
                                .and_then(serde_json::Value::as_u64)
                                .unwrap_or(0);
                            let level = c.get("level").and_then(|x| x.as_str()).unwrap_or("");
                            let code = c
                                .get("code")
                                .and_then(serde_json::Value::as_u64)
                                .unwrap_or(0);
                            let message = c.get("message").and_then(|x| x.as_str()).unwrap_or("");
                            let msg =
                                format!("shellcheck [{code}] {file}:{line}:{column} {message}");
                            if level.eq_ignore_ascii_case("error") {
                                fr.valid = false;
                                fr.errors.push(msg);
                            } else {
                                fr.warnings.push(msg);
                            }
                        };

                        // Legacy format: { "comments": [...] }
                        if let Some(comments) = v.get("comments").and_then(|x| x.as_array()) {
                            for c in comments {
                                push_issue(c);
                            }
                        // Modern format: an array of issue objects
                        } else if let Some(arr) = v.as_array() {
                            for c in arr {
                                push_issue(c);
                            }
                        } else {
                            fr.warnings.push(format!(
                                "shellcheck returned unexpected JSON structure: {}",
                                String::from_utf8_lossy(&output.stdout)
                            ));
                        }
                    }
                    Err(e) => {
                        fr.warnings
                            .push(format!("Failed to parse shellcheck JSON: {e}"));
                    }
                }
            } else if !output.stderr.is_empty() {
                let stde = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if !stde.is_empty() {
                    fr.warnings.push(format!("shellcheck stderr: {stde}"));
                }
            }
        }
        Err(e) => {
            // shellcheck运行失败，加个警告，包含错误信息以便排查
            fr.warnings.push(format!("Failed to run shellcheck: {e}"));
        }
    }
    // 如果要求修复且shfmt可用，就用shfmt格式化
    if do_fix && command_installed("shfmt") {
        let before = s;
        let status_res = Command::new("shfmt").arg("-w").arg(path).status();
        if matches!(status_res, Ok(s) if s.success()) {
            // 重新读取文件，比较是否有变化
            let after = fs::read_to_string(path)?;
            if after != before {
                fr.fixed = true;
            }
        }
    }
    Ok(fr)
}

/// Apply file-name based special checks and warnings for common hook scripts and other special files.
/// Examples:
/// - If file is `install.sh`: suggest renaming to `customize.sh`.
/// - If file is `post-fs-data.sh` and contains `setprop`: warning recommending `resetprop` usage.
fn apply_sh_filename_checks(path: &Path, content: &str, fr: &mut FileResult) {
    if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
        match fname {
            "install.sh" => {
                fr.warnings.push(
                    "Found 'install.sh' - consider renaming to 'customize.sh' to avoid ambiguous behavior"
                        .to_string(),
                );
            }
            "post-fs-data.sh" if SETPROP_RE.is_match(content) => {
                fr.warnings.push(
                    "WARNING: Using setprop will deadlock the boot process! Please use resetprop -n <prop_name> <prop_value> instead."
                        .to_string(),
                );
            }
            "post-mount.sh" => {
                fr.warnings
                    .push("This script will be executed in post-mount stage".to_string());
            }
            "service.sh" => {
                fr.warnings
                    .push("This script will be executed as a late_start service".to_string());
            }
            "boot-completed.sh" => {
                fr.warnings
                    .push("This script will be executed on boot completed".to_string());
            }
            "uninstall.sh" => {
                fr.warnings.push(
                    "This script will be executed when KernelSU removes your module".to_string(),
                );
            }
            "action.sh" => {
                fr.warnings
                    .push("This script will be executed when user clicks the Action button in KernelSU app".to_string());
            }
            "system.prop" => {
                fr.warnings.push(
                    "Properties in this file will be loaded as system properties by resetprop"
                        .to_string(),
                );
            }
            "sepolicy.rule" => {
                fr.warnings.push(
                    "SEPolicy rules may affect device security; review carefully".to_string(),
                );
            }
            _ => {}
        }
    }
}

