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

fn command_installed(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok()
}

// 检查shell脚本
// 如果有shellcheck就用它（更准确），否则用自定义的Rust检查（简单但够用）
// 另外，对所有脚本统一执行基于文件名的特判和基于 AST 的高危命令检测（补充 shellcheck 的检测）
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
                    warnings: vec![format!("shellcheck execution failed: {}", e)],
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
        .arg("--format=json")
        .arg(path)
        .output()
    {
        Ok(output) => {
            if !output.stdout.is_empty()
                && let Ok(v) = serde_json::from_slice::<serde_json::Value>(&output.stdout)
                && let Some(comments) = v.get("comments").and_then(|x| x.as_array())
            {
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
        Err(e) => {
            // shellcheck运行失败，加个警告，包含错误信息以便排查
            fr.warnings.push(format!("Failed to run shellcheck: {}", e));
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

#[cfg(test)]
mod shell_tests {
    use super::*;
    use serial_test::serial;
    use std::fs::File;
    use std::io::{Read, Write};
    use tempfile::tempdir;

    #[cfg(unix)]
    #[test]
    #[serial]
    fn shellcheck_invoked_if_present() {
        // Create a temporary directory to host a fake `shellcheck` and a test script.
        let dir = tempdir().unwrap();

        // Create fake `shellcheck` script that responds to `--version` and `--format=json <path>`.
        let sc_path = dir.path().join("shellcheck");
        {
            let mut f = File::create(&sc_path).unwrap();
            writeln!(f, "#!/bin/sh").unwrap();
            writeln!(f, "if [ \"$1\" = \"--version\" ]; then").unwrap();
            writeln!(f, "  echo \"shellcheck mock\"").unwrap();
            writeln!(f, "  exit 0").unwrap();
            writeln!(f, "fi").unwrap();
            writeln!(f, "if [ \"$1\" = \"--format=json\" ]; then").unwrap();
            writeln!(f, "  shift").unwrap();
            writeln!(f, "  p=\"$1\"").unwrap();
            f.write_all(b"  echo \"SHELLCHECK-MOCK-RUN $p\" >&2\n")
                .unwrap();
            f.write_all(b"  printf '{\"comments\":[{\"file\":\"%s\",\"line\":1,\"column\":1,\"level\":\"warning\",\"code\":9999,\"message\":\"fake-warning\"}]}' \"$p\"\n").unwrap();
            writeln!(f, "  exit 0").unwrap();
            writeln!(f, "fi").unwrap();
            writeln!(f, "exit 0").unwrap();
        }
        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&sc_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&sc_path, perms).unwrap();
        }

        // Prepend fake shellcheck to PATH for this test
        let old_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", dir.path().display(), old_path);
        unsafe {
            std::env::set_var("PATH", &new_path);
        }

        // Debug info to make failures easier to triage
        eprintln!("DEBUG: PATH={}", std::env::var("PATH").unwrap_or_default());
        match std::process::Command::new("shellcheck")
            .arg("--version")
            .output()
        {
            Ok(out) => {
                eprintln!(
                    "DEBUG: shellcheck --version status={:?}, stdout={}, stderr={}",
                    out.status,
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
            }
            Err(e) => {
                eprintln!("DEBUG: failed to run 'shellcheck --version': {}", e);
            }
        }

        // Create a test script (no .sh extension) with a shell shebang
        let script_path = dir.path().join("myscript");
        {
            let mut s = File::create(&script_path).unwrap();
            writeln!(s, "#!/bin/sh").unwrap();
            writeln!(s, "echo hello").unwrap();
        }

        // Print the script content for debugging
        match std::fs::read_to_string(&script_path) {
            Ok(content) => eprintln!("DEBUG: script content: {}", content),
            Err(e) => eprintln!("DEBUG: failed to read script content: {}", e),
        }

        // Try invoking shellcheck directly to see what it returns (debug)
        match std::process::Command::new("shellcheck")
            .arg("--format=json")
            .arg(&script_path)
            .output()
        {
            Ok(out) => eprintln!(
                "DEBUG: shellcheck --format=json returned status={:?}, stdout={}, stderr={}",
                out.status,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
            Err(e) => eprintln!("DEBUG: failed to run 'shellcheck --format=json': {}", e),
        }

        // Call check_sh which should detect our fake shellcheck via PATH and parse its JSON output
        let fr = check_sh(&script_path, false).unwrap();

        // Debug warnings/errors returned by the checker
        eprintln!("DEBUG: check_sh warnings: {:?}", fr.warnings);
        eprintln!("DEBUG: check_sh errors: {:?}", fr.errors);

        // Restore PATH
        unsafe {
            std::env::set_var("PATH", &old_path);
        }

        // Ensure the fake warning from our mock shellcheck was recorded
        assert!(fr.warnings.iter().any(|w| w.contains("fake-warning")));
    }

    #[test]
    fn debug_shebang_detection() {
        // Create a temporary directory with a script that has no extension but a shebang
        let dir = tempdir().unwrap();
        let path = dir.path().join("myscript");
        let mut f = File::create(&path).unwrap();
        writeln!(f, "#!/bin/sh").unwrap();
        writeln!(f, "echo hi").unwrap();

        // Emulate the shebang-detection logic and print debug info
        let mut found = false;
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                let mut fh = std::fs::File::open(&path).unwrap();
                let mut buf = [0u8; 256];
                let n = fh.read(&mut buf).unwrap_or(0);
                let header = String::from_utf8_lossy(&buf[..n]);
                eprintln!(
                    "DEBUG: file {:?} header: {:?}",
                    path.file_name().and_then(|n| n.to_str()),
                    header.lines().next()
                );
                if header.starts_with("#!") {
                    let lower = header.to_lowercase();
                    if lower.contains("sh")
                        || lower.contains("bash")
                        || lower.contains("dash")
                        || lower.contains("env sh")
                        || lower.contains("env bash")
                    {
                        eprintln!("DEBUG: detected shell script {:?}", path.file_name());
                        found = true;
                    }
                }
            }
        }
        assert!(found, "shebang script should be detected by heuristic");
    }

    #[test]
    fn check_sh_custom_fallback_when_no_shellcheck() {
        // Ensure that even if shellcheck is not present, the Rust fallback runs and returns a FileResult.
        let dir = tempdir().unwrap();
        let script_path = dir.path().join("myscript");
        let mut s = File::create(&script_path).unwrap();
        writeln!(s, "#!/bin/sh").unwrap();
        writeln!(s, "echo hello").unwrap();

        // Temporarily clear PATH to ensure shellcheck is not found for deterministic behavior
        let old_path = std::env::var("PATH").unwrap_or_default();
        unsafe {
            std::env::set_var("PATH", "");
        }

        let fr = check_sh(&script_path, false).unwrap();

        // Debugging info for fallback path
        eprintln!("DEBUG fallback fr.warnings: {:?}", fr.warnings);
        eprintln!("DEBUG fallback fr.errors: {:?}", fr.errors);

        // Restore PATH
        unsafe {
            std::env::set_var("PATH", &old_path);
        }

        // The result should describe a shell check result
        assert_eq!(fr.kind, "sh");
    }
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
            "post-fs-data.sh" => {
                let re = Regex::new(r"\bsetprop\b").unwrap();
                if re.is_match(content) {
                    fr.warnings.push(
                        "WARNING: Using setprop will deadlock the boot process! Please use resetprop -n <prop_name> <prop_value> instead."
                            .to_string(),
                    );
                }
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
                    && node_text_low.contains("|")
                {
                    // Prefer to show a concise snippet (the line containing the pipeline) instead of dumping the whole node
                    let mut snippet: Option<String> = None;
                    for line in node_text_all.lines() {
                        let ll = line.to_lowercase();
                        if (ll.contains("curl") || ll.contains("wget") || ll.contains("|"))
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
                    // Fallback: extract around the pipe character
                    if snippet.is_none() {
                        if let Some(pipe_pos) = node_text_all.find('|') {
                            let start = node_text_all[..pipe_pos]
                                .rfind('\n')
                                .map(|i| i + 1)
                                .unwrap_or(0);
                            let end = node_text_all[pipe_pos..]
                                .find('\n')
                                .map(|i| pipe_pos + i)
                                .unwrap_or(node_text_all.len());
                            snippet = Some(node_text_all[start..end].trim().to_string());
                        } else {
                            snippet = Some(node_text_all.trim().to_string());
                        }
                    }
                    let snippet = snippet.unwrap_or_else(|| "<snippet unavailable>".to_string());
                    let msg = format!(
                        "Piped shell install detected (download | shell): {}",
                        snippet
                    );
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
                && node_text_low.contains("|")
            {
                let mut snippet: Option<String> = None;
                for line in node_text_all.lines() {
                    let ll = line.to_lowercase();
                    if (ll.contains("curl") || ll.contains("wget") || ll.contains("|"))
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
                        let start = node_text_all[..pipe_pos]
                            .rfind('\n')
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        let end = node_text_all[pipe_pos..]
                            .find('\n')
                            .map(|i| pipe_pos + i)
                            .unwrap_or(node_text_all.len());
                        snippet = Some(node_text_all[start..end].trim().to_string());
                    } else {
                        snippet = Some(node_text_all.trim().to_string());
                    }
                }
                let snippet = snippet.unwrap_or_else(|| "<snippet unavailable>".to_string());
                let msg = format!(
                    "Piped shell install detected (download | shell): {}",
                    snippet
                );
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
                "rm" => {
                    if node_text_low.contains("-rf")
                        || (node_text_low.contains("-r") && node_text_low.contains("-f"))
                    {
                        // More precise check: examine positional arguments for absolute literal paths (skip variables)
                        let args: Vec<&str> = txt.split_whitespace().skip(1).collect();
                        let mut dangerous_abs = false;
                        for a in args {
                            let a_trim = a.trim_matches('"').trim_matches('\'');
                            // Skip variable references and command substitutions
                            if a_trim.starts_with('/')
                                && !a_trim.contains('$')
                                && !a_trim.contains('`')
                            {
                                dangerous_abs = true;
                                break;
                            }
                        }
                        if dangerous_abs {
                            fr.valid = false;
                            let msg = format!(
                                "Dangerous rm -rf usage detected (possible removal of root files): {}",
                                node_text
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
                }
                "dd" => {
                    if node_text_low.contains("/dev/") {
                        fr.valid = false;
                        fr.errors.push(format!(
                            "Potential destructive 'dd' on device: {}",
                            node_text
                        ));
                    }
                }
                _ if cmd_l.starts_with("mkfs") => {
                    fr.valid = false;
                    fr.errors.push(format!(
                        "Filesystem formatting command found: {}",
                        node_text
                    ));
                }
                "reboot" | "shutdown" | "poweroff" | "halt" => {
                    fr.warnings
                        .push("Command will reboot or shutdown the device".to_string());
                }
                "chmod" => {
                    if node_text_low.contains("777") && node_text_low.contains("-r") {
                        fr.warnings
                            .push(format!("Potentially unsafe 'chmod 777 -R': {}", node_text));
                    }
                }
                "chown" => {
                    if node_text_low.contains("/system") || node_text_low.contains("/data") {
                        fr.warnings
                            .push(format!("'chown' on system/data detected: {}", node_text));
                    }
                }
                "setprop" => {
                    // If this is not the post-fs-data special-case (already warned), add a general warning
                    if !path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s == "post-fs-data.sh")
                        .unwrap_or(false)
                    {
                        fr.warnings.push(format!("Usage of 'setprop' detected; prefer 'resetprop -n' where appropriate: {}", node_text));
                    }
                }
                "eval" => {
                    let msg = format!("Use of 'eval' detected: {}", node_text);
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
        if let Some(child) = node.child(i as u32) {
            detect_dangerous_commands(child, src, fr, path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    // Helper to run the custom checker on temporary content
    fn run_check_on_content(content: &str) -> FileResult {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("t.sh");
        let mut f = File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        check_sh_custom(&path, false).unwrap()
    }

    // Ensure comments are ignored (no detection on commented commands)
    #[test]
    fn test_detect_dangerous_commands_ignores_comments() {
        let content = r#"
# This is a comment showing a dangerous command
# rm -rf / -- not executed
# curl http://example.org | sh
"#;
        let fr = run_check_on_content(content);
        assert!(!fr.errors.iter().any(|e| e.contains("Dangerous rm -rf")));
        assert!(
            !fr.warnings
                .iter()
                .any(|w| w.contains("Piped shell install"))
        );
    }

    // Ensure real commands are detected (rm -rf and eval detection)
    #[test]
    fn test_detect_dangerous_commands_flags_rm_rf_and_eval() {
        let content = r#"
# benign comment
rm -rf /tmp/somewhere
eval set -- "$opt"
"#;
        let fr = run_check_on_content(content);
        assert!(fr.errors.iter().any(|e| e.contains("Dangerous rm -rf")));
        assert!(
            fr.warnings
                .iter()
                .any(|w| w.contains("Use of 'eval' detected"))
        );
    }

    #[test]
    fn test_piped_curl_warning_snippet() {
        let tmp = TempDir::new().unwrap();
        // Use a non-canonical installer filename so content-based detection should trigger
        let path = tmp.path().join("script.sh");
        std::fs::write(
            &path,
            "#!/bin/bash\n# comment\ncurl -sSL https://example.com/install.sh | sh\n",
        )
        .unwrap();
        let fr = check_sh_custom(&path, false).unwrap();
        assert!(
            fr.warnings
                .iter()
                .any(|w| w.to_lowercase().contains("piped shell install")
                    && w.to_lowercase().contains("curl")),
            "Expected piped curl warning with command snippet"
        );
    }

    #[test]
    fn test_shebang_only_no_piped_warning() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("install.sh");
        std::fs::write(&path, "#!/bin/bash\n# Just a comment\n").unwrap();
        let fr = check_sh_custom(&path, false).unwrap();
        assert!(
            !fr.warnings
                .iter()
                .any(|w| w.contains("Piped shell install"))
        );
    }

    #[test]
    fn test_install_sh_no_piped_warning_even_with_pipeline() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("install.sh");
        std::fs::write(
            &path,
            "#!/bin/bash\ncurl -sSL https://sh.rustup.rs | sh -s -- -y\n",
        )
        .unwrap();
        let fr = check_sh_custom(&path, false).unwrap();
        // install.sh should not trigger a piped-download warning
        assert!(
            !fr.warnings
                .iter()
                .any(|w| w.contains("Piped shell install")),
            "install.sh should not trigger piped-download warning"
        );
    }
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
                if let Some(child) = node.child(i as u32) {
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

#[test]
fn test_install_sh_suggest_rename() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("install.sh");
    fs::write(&path, "echo hi\n").unwrap();
    let fr = check_sh(&path, false).unwrap();
    assert!(
        fr.warnings
            .iter()
            .any(|w| w.contains("install.sh") || w.contains("customize.sh")),
        "Expected a warning suggesting rename to customize.sh"
    );
}

#[test]
fn test_post_fs_data_setprop_warning() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("post-fs-data.sh");
    fs::write(&path, "setprop sys.boot_completed 1\n").unwrap();
    let fr = check_sh(&path, false).unwrap();
    assert!(
        fr.warnings
            .iter()
            .any(|w| w.to_lowercase().contains("setprop")),
        "Expected a setprop warning for post-fs-data.sh"
    );
}

#[test]
fn test_detect_rm_rf_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("danger.sh");
    fs::write(&path, "rm -rf /\n").unwrap();
    let fr = check_sh(&path, false).unwrap();
    assert!(
        fr.errors
            .iter()
            .any(|e| e.to_lowercase().contains("rm -rf")),
        "Expected detection of dangerous rm -rf usage"
    );
}

#[test]
fn test_piped_curl_warning() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("installme.sh");
    fs::write(&path, "curl -sSL https://example.com/install.sh | sh\n").unwrap();
    let fr = check_sh(&path, false).unwrap();
    assert!(
        fr.warnings
            .iter()
            .any(|w| w.to_lowercase().contains("piped shell install")
                || w.to_lowercase().contains("piped")),
        "Expected a warning for piped download to shell"
    );
}
