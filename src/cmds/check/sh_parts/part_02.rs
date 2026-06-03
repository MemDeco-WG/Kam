/// Walk Tree-sitter AST and detect suspicious / high-risk commands.
///
/// Basic heuristics implemented:
/// - rm -rf ... on absolute paths -> error
/// - dd to /dev/* -> error
/// - mkfs* -> error
/// - chmod 777 -R -> warning
/// - chown on /system or /data -> warning
/// - reboot/shutdown/poweroff -> warning
/// - piped download to shell (curl|wget ... | sh) -> warning
/// - setprop usage (generic warning unless already handled specially)
#[allow(clippy::too_many_lines)] // TODO: split into smaller helpers to reduce complexity
fn detect_dangerous_commands(node: tree_sitter::Node, src: &str, fr: &mut FileResult, path: &Path) {
    // Skip comment nodes (do not analyze commented-out code)
    if node.kind() == "comment" {
        return;
    }

    // Detect piped download-to-shell patterns only within command-like nodes (command/pipeline),
    // to avoid false positives coming from comments or other non-command nodes.
    if (node.kind() == "pipeline" || node.kind() == "command")
        && let Ok(node_text_all) = node.utf8_text(src.as_bytes())
    {
        // If this file is the canonical 'install.sh' script, deliberately skip piped-download
        // detection because installers commonly use `curl | sh` patterns and we don't want to flag them.
        if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
            if fname.eq_ignore_ascii_case("install.sh") {
                // Skip piped-download warning for install.sh
            } else {
                let node_text_low = node_text_all.to_lowercase();
                // Simple substring-based detection: look for curl/wget + pipe
                if (node_text_low.contains("curl") || node_text_low.contains("wget"))
                    && node_text_low.contains('|')
                {
                    // Prefer to show a concise snippet (the line containing the pipeline) instead of dumping the whole node
                    let mut snippet: Option<String> = None;
                    for line in node_text_all.lines() {
                        let ll = line.to_lowercase();
                        if (ll.contains("curl") || ll.contains("wget") || ll.contains('|'))
                            && (ll.contains(" sh")
                                || ll.contains('|')
                                || ll.contains(" bash")
                                || ll.contains("| bash")
                                || ll.contains("sh ")
                                || ll.contains("bash "))
                        {
                            snippet = Some(line.trim().to_string());
                            break;
                        }
                    }
                    // Fallback: extract around the pipe character
                    if snippet.is_none() {
                        if let Some(pipe_pos) = node_text_all.find('|') {
                            let start = node_text_all[..pipe_pos].rfind('\n').map_or(0, |i| i + 1);
                            let end = node_text_all[pipe_pos..]
                                .find('\n')
                                .map_or(node_text_all.len(), |i| pipe_pos + i);
                            snippet = Some(node_text_all[start..end].trim().to_string());
                        } else {
                            snippet = Some(node_text_all.trim().to_string());
                        }
                    }
                    let snippet = snippet.unwrap_or_else(|| "<snippet unavailable>".to_string());
                    let msg = format!("Piped shell install detected (download | shell): {snippet}");
                    if !fr
                        .warnings
                        .iter()
                        .any(|w| w.contains("Piped shell install detected"))
                    {
                        fr.warnings.push(msg);
                    }
                }
            }
        } else {
            // No filename available; fall back to content-based detection as before
            let node_text_low = node_text_all.to_lowercase();
            if (node_text_low.contains("curl") || node_text_low.contains("wget"))
                && node_text_low.contains('|')
            {
                let mut snippet: Option<String> = None;
                for line in node_text_all.lines() {
                    let ll = line.to_lowercase();
                    if (ll.contains("curl") || ll.contains("wget") || ll.contains('|'))
                        && (ll.contains(" sh")
                            || ll.contains("| sh")
                            || ll.contains(" bash")
                            || ll.contains("| bash")
                            || ll.contains("sh ")
                            || ll.contains("bash "))
                    {
                        snippet = Some(line.trim().to_string());
                        break;
                    }
                }
                if snippet.is_none() {
                    if let Some(pipe_pos) = node_text_all.find('|') {
                        let start = node_text_all[..pipe_pos].rfind('\n').map_or(0, |i| i + 1);
                        let end = node_text_all[pipe_pos..]
                            .find('\n')
                            .map_or(node_text_all.len(), |i| pipe_pos + i);
                        snippet = Some(node_text_all[start..end].trim().to_string());
                    } else {
                        snippet = Some(node_text_all.trim().to_string());
                    }
                }
                let snippet = snippet.unwrap_or_else(|| "<snippet unavailable>".to_string());
                let msg = format!("Piped shell install detected (download | shell): {snippet}");
                if !fr
                    .warnings
                    .iter()
                    .any(|w| w.contains("Piped shell install detected"))
                {
                    fr.warnings.push(msg);
                }
            }
        }
    }

    // If this node is a command, try to extract the command name & full text and apply heuristics
    if node.kind() == "command"
        && node.child(0).is_some()
        && let Ok(node_text) = node.utf8_text(src.as_bytes())
    {
        let txt = node_text.trim();
        let mut iter = txt.split_whitespace();
        if let Some(cmd) = iter.next() {
            let cmd_l = cmd.to_lowercase();
            let node_text_low = node_text.to_lowercase();

            match cmd_l.as_str() {
                "rm" if node_text_low.contains("-rf")
                    || (node_text_low.contains("-r") && node_text_low.contains("-f")) =>
                {
                    // More precise check: examine positional arguments for absolute literal paths (skip variables)
                    let args: Vec<&str> = txt.split_whitespace().skip(1).collect();
                    let mut dangerous_abs = false;
                    for a in args {
                        let a_trim = a.trim_matches('"').trim_matches('\'');
                        // Skip variable references and command substitutions
                        if a_trim.starts_with('/') && !a_trim.contains('$') && !a_trim.contains('`')
                        {
                            dangerous_abs = true;
                            break;
                        }
                    }
                    if dangerous_abs {
                        fr.valid = false;
                        let msg = format!(
                            "Dangerous rm -rf usage detected (possible removal of root files): {node_text}"
                        );
                        if !fr.errors.contains(&msg) {
                            fr.errors.push(msg);
                        }
                    } else {
                        let warn_msg =
                            "rm -rf usage detected; ensure this is intentional".to_string();
                        if !fr.warnings.contains(&warn_msg) {
                            fr.warnings.push(warn_msg);
                        }
                    }
                }
                "dd" if node_text_low.contains("/dev/") => {
                    fr.valid = false;
                    fr.errors
                        .push(format!("Potential destructive 'dd' on device: {node_text}"));
                }
                _ if cmd_l.starts_with("mkfs") => {
                    fr.valid = false;
                    fr.errors
                        .push(format!("Filesystem formatting command found: {node_text}"));
                }
                "reboot" | "shutdown" | "poweroff" | "halt" => {
                    fr.warnings
                        .push("Command will reboot or shutdown the device".to_string());
                }
                "chmod" if node_text_low.contains("777") && node_text_low.contains("-r") => {
                    fr.warnings
                        .push(format!("Potentially unsafe 'chmod 777 -R': {node_text}"));
                }
                "chown" if node_text_low.contains("/system") || node_text_low.contains("/data") => {
                    let msg = format!("'chown' on system/data detected: {node_text}");
                    if !fr.warnings.contains(&msg) {
                        fr.warnings.push(msg);
                    }
                }
                "setprop"
                    if path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_none_or(|s| s != "post-fs-data.sh") =>
                {
                    // If this is not the post-fs-data special-case (already warned), add a general warning
                    let msg = format!(
                        "Usage of 'setprop' detected; prefer 'resetprop -n' where appropriate: {node_text}"
                    );
                    if !fr.warnings.contains(&msg) {
                        fr.warnings.push(msg);
                    }
                }
                "eval" => {
                    let msg = format!("Use of 'eval' detected: {node_text}");
                    if !fr.warnings.contains(&msg) {
                        fr.warnings.push(msg);
                    }
                }
                _ => {
                    // For 'command' nodes we already handled many checks; leave room for heuristics.
                }
            }
        }
    }

    // Recurse children
    for i in 0..node.child_count() {
        if let Ok(idx) = u32::try_from(i)
            && let Some(child) = node.child(idx)
        {
            detect_dangerous_commands(child, src, fr, path);
        }
    }
}
// 自定义的shell脚本检查（没有shellcheck时用）
// 只做基本的检查：引号匹配、括号匹配、尾随空格、CRLF等
// 虽然不如shellcheck全面，但至少能发现一些明显的问题
#[allow(clippy::too_many_lines)] // TODO: split this function into smaller helpers to reduce complexity
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
    // 基本检查：不匹配的引号、尾随空格、CRLF
    // 这个检查比较简单，但能发现一些明显的问题
    // 注：不再将不匹配的括号作为错误；某些 Shell 结构（如 case 模式中的 `pattern)`）会包含单独的右括号，原先的简单计数会导致误报。
    let mut single_open = false;
    let mut double_open = false;
    let mut escaped = false;
    // 逐字符扫描，仅跟踪引号的状态（括号匹配检测已移除以避免误报）
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
        }
        // 双引号（只在不在单引号内时处理）
        if ch == '"' && !single_open {
            double_open = !double_open;
        }
    }
    // 检查引号是否匹配
    if single_open || double_open {
        fr.valid = false;
        fr.errors.push("Unbalanced quotes detected".to_string());
    }

    // AST-aware checks for command substitutions, arithmetic expansions, and backticks.
    // Only check constructs that are syntactic (e.g., $(), $(( )), backticks) so that we
    // avoid false positives from shell constructs like `case` patterns that contain `)`.
    for err in detect_unbalanced_shell_constructs(&s) {
        if !fr.errors.contains(&err) {
            fr.valid = false;
            fr.errors.push(err);
        }
    }
    // 用tree-sitter解析：如果解析器可用，检测语法错误
    // 虽然可能有点慢，但能发现更多问题
    let mut parser = Parser::new();
    // Some tree-sitter-bash bindings export `LANGUAGE` const instead of `language()` function.
    let language = tree_sitter::Language::new(tree_sitter_bash::LANGUAGE);
    if parser.set_language(&language).is_ok()
        && let Some(tree) = parser.parse(&s, None)
    {
        // Walk nodes to find ERROR nodes
        fn walk_node(node: tree_sitter::Node, fr: &mut FileResult) {
            if node.kind() == "ERROR" {
                fr.valid = false;
                let pos = node.start_position();
                fr.errors
                    .push(format!("Parse error at {}:{}", pos.row + 1, pos.column + 1));
            }
            for i in 0..node.child_count() {
                if let Ok(idx) = u32::try_from(i)
                    && let Some(child) = node.child(idx)
                {
                    walk_node(child, fr);
                }
            }
        }
        let root = tree.root_node();
        walk_node(root, &mut fr);

        // Also run the AST-based dangerous command detection so the custom checker
        // reports the same high-risk commands as the main `check_sh` wrapper.
        detect_dangerous_commands(root, &s, &mut fr, path);
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

//
// AST-aware unbalanced-construct detection helpers
//

