use crate::cmds::check::file::FileResult;
use crate::errors::KamError;
use serde_json;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tree_sitter;
use tree_sitter::Parser;
use tree_sitter_bash;

fn command_installed(cmd: &str) -> bool {
    match Command::new(cmd).arg("--version").output() {
        Ok(_) => true,
        Err(_) => false,
    }
}

// 检查shell脚本
// 如果有shellcheck就用它（更准确），否则用自定义的Rust检查（简单但够用）
pub fn check_sh(path: &Path, do_fix: bool) -> Result<FileResult, KamError> {
    if command_installed("shellcheck") {
        // shellcheck可用，用它检查（更专业）
        return check_sh_with_tool(path, do_fix);
    }
    // 没有shellcheck，用自定义检查
    check_sh_custom(path, do_fix)
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
        .arg("--format=json")
        .arg(path)
        .output()
    {
        Ok(output) => {
            if !output.stdout.is_empty() {
                // 解析JSON输出，提取错误和警告
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if let Some(comments) = v.get("comments").and_then(|x| x.as_array()) {
                        for c in comments {
                            // 提取shellcheck的检查结果
                            let file = c.get("file").and_then(|x| x.as_str()).unwrap_or("");
                            let line = c.get("line").and_then(|x| x.as_u64()).unwrap_or(0);
                            let column = c.get("column").and_then(|x| x.as_u64()).unwrap_or(0);
                            let level = c.get("level").and_then(|x| x.as_str()).unwrap_or("");
                            let code = c.get("code").and_then(|x| x.as_u64()).unwrap_or(0);
                            let message = c.get("message").and_then(|x| x.as_str()).unwrap_or("");
                            let msg = format!(
                                "shellcheck [{}] {}:{}:{} {}",
                                code, file, line, column, message
                            );
                            // 根据级别分类：error或warning
                            if level.eq_ignore_ascii_case("error") {
                                fr.valid = false;
                                fr.errors.push(msg.clone());
                            } else {
                                fr.warnings.push(msg.clone());
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {
            // shellcheck运行失败，加个警告
            fr.warnings.push("Failed to run shellcheck".to_string());
        }
    }
    // 如果要求修复且shfmt可用，就用shfmt格式化
    if do_fix && command_installed("shfmt") {
        let before = s.clone();
        let status = Command::new("shfmt").arg("-w").arg(path).status();
        if status.is_ok() && status.unwrap().success() {
            // 重新读取文件，比较是否有变化
            let after = fs::read_to_string(path)?;
            if after != before {
                fr.fixed = true;
            }
        }
    }
    Ok(fr)
}

// 自定义的shell脚本检查（没有shellcheck时用）
// 只做基本的检查：引号匹配、括号匹配、尾随空格、CRLF等
// 虽然不如shellcheck全面，但至少能发现一些明显的问题
fn check_sh_custom(path: &Path, do_fix: bool) -> Result<FileResult, KamError> {
    let s = fs::read_to_string(path)?;
    let mut fr = FileResult {
        path: path.to_string_lossy().to_string(),
        kind: "sh".to_string(),
        valid: true,
        errors: Vec::new(),
        warnings: Vec::new(),
        fixed: false,
    };
    // 基本检查：不匹配的引号、不匹配的括号（排除引号内的）、尾随空格、CRLF
    // 这个检查比较简单，但能发现一些明显的问题
    let mut single_open = false;
    let mut double_open = false;
    let mut escaped = false;
    let mut paren_depth: i64 = 0;
    // 逐字符扫描，跟踪引号和括号的状态
    for ch in s.chars() {
        if escaped {
            // 转义字符，跳过下一个字符的特殊处理
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        // 单引号（只在不在双引号内时处理）
        if ch == '\'' && !double_open {
            single_open = !single_open;
            continue;
        }
        // 双引号（只在不在单引号内时处理）
        if ch == '"' && !single_open {
            double_open = !double_open;
            continue;
        }
        // 括号匹配（只在不在引号内时处理）
        if !single_open && !double_open {
            if ch == '(' {
                paren_depth += 1;
            } else if ch == ')' {
                paren_depth -= 1;
            }
        }
    }
    // 检查引号是否匹配
    if single_open || double_open {
        fr.valid = false;
        fr.errors.push("Unbalanced quotes detected".to_string());
    }
    // 检查括号是否匹配
    if paren_depth != 0 {
        fr.valid = false;
        fr.errors
            .push("Unbalanced parentheses in script".to_string());
    }
    // 用tree-sitter解析：如果解析器可用，检测语法错误
    // 虽然可能有点慢，但能发现更多问题
    let mut parser = Parser::new();
    // Some tree-sitter-bash bindings export `LANGUAGE` const instead of `language()` function.
    let language = tree_sitter::Language::new(tree_sitter_bash::LANGUAGE);
    if parser.set_language(&language).is_ok() {
        if let Some(tree) = parser.parse(&s, None) {
            // Walk nodes to find ERROR nodes
            fn walk_node(node: tree_sitter::Node, fr: &mut FileResult) {
                if node.kind() == "ERROR" {
                    fr.valid = false;
                    let pos = node.start_position();
                    fr.errors
                        .push(format!("Parse error at {}:{}", pos.row + 1, pos.column + 1));
                }
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i as u32) {
                        walk_node(child, fr);
                    }
                }
            }
            let root = tree.root_node();
            walk_node(root, &mut fr);
        }
    }
    // 基本内容修复（类似markdown）：CRLF、尾随空格、文件末尾换行
    // 这些虽然不影响功能，但统一格式比较好
    let mut content = s.clone();
    if content.contains("\r\n") {
        fr.warnings.push("CRLF line endings detected".to_string());
        if do_fix {
            content = content.replace("\r\n", "\n");
            fr.fixed = true;
        }
    }
    // 去除每行的尾随空格
    let trimmed_lines: Vec<String> = content
        .lines()
        .map(|l| {
            if l.ends_with(' ') {
                if do_fix {
                    fr.fixed = true;
                    l.trim_end().to_string()
                } else {
                    l.to_string()
                }
            } else {
                l.to_string()
            }
        })
        .collect();
    // 确保文件末尾有换行符
    let new_content = trimmed_lines.join("\n") + "\n";
    if new_content != content {
        if do_fix {
            // 写回修复后的内容
            fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)?
                .write_all(new_content.as_bytes())?;
            fr.fixed = true;
        } else {
            fr.warnings
                .push("Trailing spaces or missing newline at EOF".to_string());
        }
    }
    // Other suggestions
    if s.contains("eval ") {
        fr.warnings
            .push("Usage of eval detected; consider alternatives".to_string());
    }
    Ok(fr)
}
