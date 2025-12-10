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

pub fn check_sh(path: &Path, do_fix: bool) -> Result<FileResult, KamError> {
    // If shellcheck available, use it; otherwise use custom rust-based checks
    if command_installed("shellcheck") {
        return check_sh_with_tool(path, do_fix);
    }
    check_sh_custom(path, do_fix)
}

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
    // Run shellcheck with JSON output
    match Command::new("shellcheck")
        .arg("--format=json")
        .arg(path)
        .output()
    {
        Ok(output) => {
            if !output.stdout.is_empty() {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if let Some(comments) = v.get("comments").and_then(|x| x.as_array()) {
                        for c in comments {
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
            fr.warnings.push("Failed to run shellcheck".to_string());
        }
    }
    // Try to format with shfmt if present
    if do_fix && command_installed("shfmt") {
        let before = s.clone();
        let status = Command::new("shfmt").arg("-w").arg(path).status();
        if status.is_ok() && status.unwrap().success() {
            // read again and compare
            let after = fs::read_to_string(path)?;
            if after != before {
                fr.fixed = true;
            }
        }
    }
    Ok(fr)
}

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
    // Basic checks: unbalanced quotes, unbalanced parentheses (excluding quoted sections), trailing whitespace, CRLF
    let mut single_open = false;
    let mut double_open = false;
    let mut escaped = false;
    let mut paren_depth: i64 = 0;
    for ch in s.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' && !double_open {
            single_open = !single_open;
            continue;
        }
        if ch == '"' && !single_open {
            double_open = !double_open;
            continue;
        }
        if !single_open && !double_open {
            if ch == '(' {
                paren_depth += 1;
            } else if ch == ')' {
                paren_depth -= 1;
            }
        }
    }
    if single_open || double_open {
        fr.valid = false;
        fr.errors.push("Unbalanced quotes detected".to_string());
    }
    if paren_depth != 0 {
        fr.valid = false;
        fr.errors
            .push("Unbalanced parentheses in script".to_string());
    }
    // Tree-sitter parse: detect syntax errors if parser is available
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
    // Basic content fixes (like markdown): CRLF / trailing spaces / EOF newline
    let mut content = s.clone();
    if content.contains("\r\n") {
        fr.warnings.push("CRLF line endings detected".to_string());
        if do_fix {
            content = content.replace("\r\n", "\n");
            fr.fixed = true;
        }
    }
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
    let new_content = trimmed_lines.join("\n") + "\n";
    if new_content != content {
        if do_fix {
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
