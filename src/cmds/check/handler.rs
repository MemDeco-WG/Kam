use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
#[cfg(test)]
use std::fs;
use std::io::IsTerminal;
use std::io::Read;
#[cfg(test)]
use std::io::Write;
use std::path::{Path, PathBuf};

use super::args::CheckArgs;
use super::file::{FileResult, check_file};
use crate::errors::KamError;
use crate::types::kam_toml::RuleConfig;

fn collect_project_files(
    project_path: &Path,
    skip_dirs: &[String],
    is_kam_project: bool,
) -> Vec<PathBuf> {
    // Read top-level .gitignore and compile a basic include/exclude list.
    // This supports simple patterns and negations (lines starting with '!').
    let gitignore_file = project_path.join(".gitignore");
    let mut gi_patterns: Vec<String> = Vec::new();
    let mut gi_whitelist: Vec<String> = Vec::new();
    if gitignore_file.exists()
        && let Ok(contents) = std::fs::read_to_string(&gitignore_file)
    {
        for raw in contents.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(stripped) = line.strip_prefix('!') {
                let pat = stripped.trim();
                if !pat.is_empty() {
                    gi_whitelist.push(pat.to_string());
                }
            } else {
                gi_patterns.push(line.to_string());
            }
        }
    }

    // Traverse project files using ignore::WalkBuilder so that .gitignore and VCS ignores are respected.
    // Also apply our default skip directory filter for common build/artifact dirs.
    let mut files: Vec<PathBuf> = Vec::new();
    let skip_clone = skip_dirs.to_owned();

    let walker = ignore::WalkBuilder::new(project_path)
        .git_ignore(true)
        .hidden(false)
        .filter_entry(move |entry| {
            // Keep root
            if entry.depth() == 0 {
                return true;
            }
            if entry.file_type().map(|f| f.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy();
                return !skip_clone.iter().any(|s| s == name.as_ref());
            }
            true
        })
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path == project_path || path.is_dir() {
            continue;
        }

        // Relative path from project root
        let rel_path = match path.strip_prefix(project_path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => continue,
        };

        let file_name_opt = path.file_name().and_then(|n| n.to_str());

        // Apply top-level .gitignore patterns (if any)
        let mut ignored = false;
        for pat in &gi_patterns {
            if crate::utils::pattern_matches(pat, &rel_path, file_name_opt) {
                ignored = true;
                break;
            }
        }
        if ignored {
            // negations (!) in .gitignore can re-include files
            for w in &gi_whitelist {
                if crate::utils::pattern_matches(w, &rel_path, file_name_opt) {
                    ignored = false;
                    break;
                }
            }
        }
        if ignored {
            continue;
        }

        // kam.toml is handled separately; don't include it in the generic list
        if is_kam_project && path.file_name().and_then(|n| n.to_str()) == Some("kam.toml") {
            continue;
        }
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext.to_lowercase().as_str() {
                "json" | "yml" | "yaml" | "toml" | "sh" | "bash" | "md" => {
                    files.push(path.to_path_buf());
                }
                _ => {}
            }
        } else {
            // No extension: attempt to detect shell scripts via shebang
            // Read a small prefix of the file and look for common shell shebangs
            if let Ok(mut fh) = std::fs::File::open(path) {
                let mut buf = [0u8; 256];
                if let Ok(n) = fh.read(&mut buf) {
                    let header = String::from_utf8_lossy(&buf[..n]).to_lowercase();
                    if header.starts_with("#!")
                        && (header.contains("sh")
                            || header.contains("bash")
                            || header.contains("dash")
                            || header.contains("env sh")
                            || header.contains("env bash"))
                    {
                        files.push(path.to_path_buf());
                    }
                }
            }
        }
    }

    files
}

fn render_json_results(results: &Vec<FileResult>, verbose: bool) -> serde_json::Value {
    if verbose {
        serde_json::to_value(results).unwrap_or_else(|_| serde_json::json!([]))
    } else {
        let mut errors: Vec<serde_json::Value> = Vec::new();
        let mut warnings: Vec<serde_json::Value> = Vec::new();
        let mut error_count: usize = 0;
        let mut warning_count: usize = 0;
        for r in results {
            if !r.errors.is_empty() {
                error_count += r.errors.len();
                errors.push(serde_json::json!({"path": r.path, "messages": r.errors}));
            }
            if !r.warnings.is_empty() {
                warning_count += r.warnings.len();
                warnings.push(serde_json::json!({"path": r.path, "messages": r.warnings}));
            }
        }
        serde_json::json!({
            "errors": errors,
            "warnings": warnings,
            "summary": {"error_count": error_count, "warning_count": warning_count},
        })
    }
}

pub fn run(args: CheckArgs) -> Result<(), KamError> {
    // Determine input paths: use positional `PATHS` (files/dirs/globs) when provided,
    // otherwise fall back to the configured `--project-path` (`args.path`, default ".").
    let input_paths: Vec<String> = if args.paths.is_empty() {
        vec![args.path.clone()]
    } else {
        args.paths.clone()
    };

    // Report shellcheck availability early so users can tell whether .sh files will
    // be checked by shellcheck (preferred) or by the built-in Rust check fallback.
    // Include any failure detail as a user-visible warning.
    if !args.json {
        match std::process::Command::new("shellcheck")
            .arg("--version")
            .output()
        {
            Ok(out) => {
                if out.status.success() {
                    let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    crate::utils::Utils::info(&format!("shellcheck detected: {}", ver));
                } else {
                    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    crate::utils::Utils::warn(&format!(
                        "shellcheck present but '--version' returned non-zero: {}",
                        err
                    ));
                }
            }
            Err(e) => {
                crate::utils::Utils::warn(&format!(
                    "shellcheck not found or cannot be executed: {}",
                    e
                ));
            }
        }
    }

    let mut results: Vec<FileResult> = Vec::new();

    // 获取默认排除目录，再加几个常见的
    // 这些目录通常不需要检查（比如构建产物、模板等）
    let mut skip_dirs = crate::utils::default_exclude_dir_names();
    for d in ["dist", "templates", "tmpl"].iter() {
        if !skip_dirs.iter().any(|s| s == d) {
            skip_dirs.push(d.to_string());
        }
    }

    // Expand input paths (support files, directories, and glob patterns).
    // For any directory that looks like a Kam project (has kam.toml) we
    // perform a pre-check of its kam.toml and then collect files under it.
    let mut collected_files: Vec<PathBuf> = Vec::new();
    let mut prechecked_kam_count: usize = 0;
    // Cache of per-project rule configurations (keyed by project directory).
    // Each entry maps to an optional rules map parsed from that project's kam.toml.
    let mut project_rules_cache: std::collections::HashMap<
        std::path::PathBuf,
        Option<std::collections::HashMap<String, RuleConfig>>,
    > = std::collections::HashMap::new();

    for p in input_paths {
        // Treat common glob characters as a hint to expand via glob crate.
        let looks_like_glob = p.contains('*') || p.contains('?') || p.contains('[');
        if looks_like_glob {
            match glob::glob(&p) {
                Ok(entries) => {
                    for entry in entries.filter_map(Result::ok) {
                        if entry.is_dir() {
                            if entry.join("kam.toml").exists() {
                                if let Ok(res) = super::file::check_file(
                                    &entry.join("kam.toml"),
                                    "toml",
                                    args.fix,
                                    None,
                                ) {
                                    results.push(res);
                                    prechecked_kam_count += 1;
                                }
                                // Try to parse and cache the project's rules config for later files.
                                if let Ok(kt) = crate::types::kam_toml::KamToml::load_from_file(
                                    entry.join("kam.toml"),
                                ) {
                                    let key =
                                        entry.canonicalize().unwrap_or_else(|_| entry.clone());
                                    project_rules_cache.insert(key, kt.rules);
                                }
                            }
                            let dir_files = collect_project_files(
                                &entry,
                                &skip_dirs,
                                entry.join("kam.toml").exists(),
                            );
                            collected_files.extend(dir_files);
                        } else if entry.is_file() {
                            collected_files.push(entry);
                        }
                    }
                }
                Err(e) => {
                    crate::utils::Utils::warn(&format!("Invalid glob pattern '{}': {}", p, e));
                }
            }
        } else {
            let pathp = Path::new(&p);
            if pathp.exists() {
                if pathp.is_dir() {
                    if pathp.join("kam.toml").exists() {
                        if let Ok(res) =
                            super::file::check_file(&pathp.join("kam.toml"), "toml", args.fix, None)
                        {
                            results.push(res);
                            prechecked_kam_count += 1;
                        }
                        if let Ok(kt) =
                            crate::types::kam_toml::KamToml::load_from_file(pathp.join("kam.toml"))
                        {
                            let key = pathp.canonicalize().unwrap_or_else(|_| pathp.to_path_buf());
                            project_rules_cache.insert(key, kt.rules);
                        }
                    }
                    let dir_files =
                        collect_project_files(pathp, &skip_dirs, pathp.join("kam.toml").exists());
                    collected_files.extend(dir_files);
                } else if pathp.is_file() {
                    collected_files.push(pathp.to_path_buf());
                }
            } else {
                return Err(KamError::InvalidDirectory(trf!(
                    "Path does not exist: {}",
                    p
                )));
            }
        }
    }

    // Deduplicate files (canonicalized where possible).
    use std::collections::HashSet;
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut files: Vec<PathBuf> = Vec::new();
    for f in collected_files {
        let canon = f.canonicalize().unwrap_or_else(|_| f.clone());
        if seen.insert(canon.clone()) {
            files.push(canon);
        }
    }

    // NOTE:
    // Project-level kam.toml checks and single-project collection were handled
    // earlier while expanding the provided PATH/GLOB targets. We no longer need
    // the leftover per-project collection here, and references to
    // `project_path`/`is_kam_project` would be invalid in multi-target mode.
    // (This block intentionally left out.)

    // 如果检测到 shell 脚本文件，确保 shellcheck 已安装并可执行；否则直接报错
    let mut has_sh = false;
    for p in &files {
        let path = p.as_path();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext.to_lowercase().as_str() {
                "sh" | "bash" => {
                    has_sh = true;
                    break;
                }
                _ => {}
            }
        } else if let Ok(mut f) = std::fs::File::open(path) {
            let mut buf = [0u8; 256];
            if let Ok(n) = f.read(&mut buf) {
                let header = String::from_utf8_lossy(&buf[..n]).to_lowercase();
                if header.starts_with("#!")
                    && (header.contains("sh")
                        || header.contains("bash")
                        || header.contains("dash")
                        || header.contains("env sh")
                        || header.contains("env bash"))
                {
                    has_sh = true;
                    break;
                }
            }
        }
    }
    if has_sh
        && std::process::Command::new("shellcheck")
            .arg("--version")
            .output()
            .is_err()
    {
        return Err(KamError::ShellcheckMissing);
    }

    // 文件总数（包含已预检的 kam.toml）
    let total_files: usize = files.len() + prechecked_kam_count;

    // 只在非JSON输出且是终端时显示进度条
    let show_progress = !args.json && std::io::stdout().is_terminal();
    let pb = if show_progress && total_files > 0 {
        let pb = ProgressBar::new(total_files as u64);
        let style = ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
        )
        .unwrap()
        .progress_chars("#>-"); // 进度条字符，看起来比较好看
        pb.set_style(style);
        // 如果已经预检过一些 kam.toml，则把进度条先推进相应的数量
        if prechecked_kam_count > 0 {
            pb.inc(prechecked_kam_count as u64);
        }
        Some(pb)
    } else {
        None
    };

    for path in files {
        let path = path.as_path();
        let kind = if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext.to_lowercase().as_str() {
                "json" => "json",
                "yml" | "yaml" => "yaml",
                "toml" => "toml",
                "sh" | "bash" => "sh",
                "md" => "markdown",
                _ => continue,
            }
        } else {
            // No extension: try to detect a shell shebang in the first few bytes
            // so scripts without '.sh' extension are still checked.
            if let Ok(mut f) = std::fs::File::open(path) {
                let mut buf = [0u8; 256];
                if let Ok(n) = f.read(&mut buf) {
                    let s = String::from_utf8_lossy(&buf[..n]);
                    if s.starts_with("#!") {
                        let lower = s.to_lowercase();
                        if lower.contains("sh")
                            || lower.contains("bash")
                            || lower.contains("dash")
                            || lower.contains("env sh")
                            || lower.contains("env bash")
                        {
                            "sh"
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            } else {
                continue;
            }
        };

        // Determine per-project rule configuration (if any) by walking up to the nearest
        // ancestor directory that had a cached kam.toml.
        fn find_project_rules<'a>(
            p: &std::path::Path,
            cache: &'a std::collections::HashMap<
                std::path::PathBuf,
                Option<std::collections::HashMap<String, RuleConfig>>,
            >,
        ) -> Option<&'a std::collections::HashMap<String, RuleConfig>> {
            let mut cur = p;
            while let Some(parent) = cur.parent() {
                if let Ok(canon) = parent.canonicalize() {
                    if let Some(opt) = cache.get(&canon) {
                        return opt.as_ref();
                    }
                } else if let Some(opt) = cache.get(parent) {
                    return opt.as_ref();
                }
                cur = parent;
            }
            None
        }
        let project_rules = find_project_rules(path, &project_rules_cache);
        let res = check_file(path, kind, args.fix, project_rules)?;
        results.push(res);
        if let Some(ref p) = pb {
            p.set_message(format!("{}", path.display()));
            p.inc(1);
        }
    }

    if let Some(ref p) = pb {
        p.finish_and_clear();
    }

    // Output
    if args.json {
        // Compact by default to reduce token/output size; use -v / --verbose to get full pretty-printed results
        let jv = render_json_results(&results, args.verbose);
        let out = if args.verbose {
            serde_json::to_string_pretty(&jv).unwrap_or_else(|_| "[]".into())
        } else {
            // Compact one-line JSON for minimal token consumption
            serde_json::to_string(&jv).unwrap_or_else(|_| "{}".into())
        };
        println!("{}", out);

        // Compute totals so JSON mode honors fail-on-error / fail-on-warning flags.
        let mut total_errors: usize = 0;
        let mut total_warnings: usize = 0;
        for r in &results {
            total_errors += r.errors.len();
            total_warnings += r.warnings.len();
        }

        if args.fail_on_error && total_errors > 0 {
            return Err(KamError::CommandFailed(format!(
                "check failed: {} error(s) found",
                total_errors
            )));
        }
        if args.fail_on_warning && total_warnings > 0 {
            return Err(KamError::CommandFailed(format!(
                "check failed: {} warning(s) found",
                total_warnings
            )));
        }

        return Ok(());
    }

    // Human-friendly output
    let mut any_errors = false;
    for r in &results {
        let path = &r.path;
        match (r.valid, r.fixed) {
            (true, true) => println!("{} {}", "✓".green(), trf!("common.file_fixed", path)),
            (true, false) => println!("{} {}", "✓".green(), path),
            (false, true) => {
                any_errors = true;
                println!("{} {}", "✕".yellow(), trf!("common.file_fixed", path))
            }
            (false, false) => {
                any_errors = true;
                println!("{} {}", "✕".color(crate::utils::Utils::error_color()), path)
            }
        }

        if !r.errors.is_empty() {
            let c = crate::utils::Utils::error_color();
            println!(
                "  {} {}",
                "✗".color(c).bold(),
                crate::i18n::tr_key("check.errors.header").color(c).bold()
            );
            for e in &r.errors {
                println!("    {} {}", "→".color(c).dimmed(), e.color(c));
            }
        }
        if !r.warnings.is_empty() {
            println!(
                "  {} {}",
                "!".yellow().bold(),
                crate::i18n::tr_key("check.warnings.header").yellow().bold()
            );
            for w in &r.warnings {
                println!("    {} {}", "→".yellow().dimmed(), w.yellow());
            }
        }
    }

    if any_errors {
        let c = crate::utils::Utils::error_color();
        println!(
            "\n{} {}",
            "✕".color(c).bold(),
            crate::i18n::tr_key("check.some_issues_found").color(c)
        );
    } else {
        println!(
            "\n{} {}",
            "✓".green().bold(),
            crate::i18n::tr_key("check.no_issues_found").green()
        );
    }

    // Compute totals and respect fail-on-* flags so callers (CI) can opt-in to non-zero exit codes.
    let mut total_errors: usize = 0;
    let mut total_warnings: usize = 0;
    for r in &results {
        total_errors += r.errors.len();
        total_warnings += r.warnings.len();
    }

    if args.fail_on_error && total_errors > 0 {
        return Err(KamError::CommandFailed(format!(
            "check failed: {} error(s) found",
            total_errors
        )));
    }
    if args.fail_on_warning && total_warnings > 0 {
        return Err(KamError::CommandFailed(format!(
            "check failed: {} warning(s) found",
            total_warnings
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn check_invalid_json_and_fix() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        let mut f = File::create(&path).unwrap();
        writeln!(f, "{{ \"a\":1,\"b\":2,}}").unwrap();
        let args = CheckArgs {
            path: ".".to_string(),
            paths: vec![dir.path().to_string_lossy().to_string()],
            json: true,
            verbose: false,
            fix: false,
            fail_on_error: false,
            fail_on_warning: false,
        };
        // Run
        let result = run(args);
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn error_when_shell_scripts_present_and_shellcheck_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("script.sh");
        let mut f = File::create(&path).unwrap();
        writeln!(f, "#!/bin/sh").unwrap();
        writeln!(f, "echo hi").unwrap();

        // Temporarily clear PATH to ensure shellcheck isn't found.
        let old_path = std::env::var("PATH").ok();
        unsafe {
            std::env::set_var("PATH", "");
        }

        let args = CheckArgs {
            path: ".".to_string(),
            paths: vec![dir.path().to_string_lossy().to_string()],
            json: true,
            verbose: false,
            fix: false,
            fail_on_error: false,
            fail_on_warning: false,
        };

        let res = run(args);

        // Restore PATH
        if let Some(p) = old_path {
            unsafe {
                std::env::set_var("PATH", p);
            }
        } else {
            unsafe {
                std::env::remove_var("PATH");
            }
        }

        assert!(res.is_err());
        let err = format!("{}", res.unwrap_err());
        assert!(err.contains("请安装shellcheck"));
    }

    #[test]
    fn collects_shebang_scripts_without_extension() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("myscript");
        let mut f = File::create(&path).unwrap();
        writeln!(f, "#!/bin/sh").unwrap();
        writeln!(f, "echo hi").unwrap();
        // Ensure the file is collected as a shell script even without .sh extension
        let files = collect_project_files(dir.path(), &[], false);
        // Debug: print directory contents and collected files to help diagnose failures
        eprintln!(
            "DEBUG: dir entries: {:?}",
            std::fs::read_dir(dir.path())
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect::<Vec<_>>()
        );
        eprintln!("DEBUG: collected files: {:?}", files);
        assert!(
            files
                .iter()
                .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("myscript")),
            "collected files: {:?}",
            files
        );
    }

    #[test]
    fn check_json_fix_applies() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("obj.json");
        let mut f = File::create(&path).unwrap();
        // intentionally compact/unformatted JSON
        writeln!(f, "{{\"a\":1,\"b\":2}} ").unwrap();
        let args = CheckArgs {
            path: ".".to_string(),
            paths: vec![dir.path().to_string_lossy().to_string()],
            json: false,
            verbose: false,
            fix: true,
            fail_on_error: false,
            fail_on_warning: false,
        };
        // Run
        let result = run(args);
        assert!(result.is_ok());
        // The file should now be pretty-printed
        let out = fs::read_to_string(&path).unwrap();
        assert!(out.contains("\n  \"a\": 1,\n  \"b\": 2\n"));
    }

    #[test]
    fn check_invalid_toml_and_fix() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        let mut f = File::create(&path).unwrap();
        writeln!(f, "a = 1 b = 2").unwrap();
        let args = CheckArgs {
            path: ".".to_string(),
            paths: vec![dir.path().to_string_lossy().to_string()],
            json: true,
            verbose: false,
            fix: false,
            fail_on_error: false,
            fail_on_warning: false,
        };
        // Run
        let result = run(args);
        assert!(result.is_ok());
    }

    #[test]
    fn check_toml_fix_applies() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("obj.toml");
        let mut f = File::create(&path).unwrap();
        // intentionally compact/unformatted TOML
        writeln!(f, "a=1\nb=2").unwrap();
        let args = CheckArgs {
            path: ".".to_string(),
            paths: vec![dir.path().to_string_lossy().to_string()],
            json: false,
            verbose: false,
            fix: true,
            fail_on_error: false,
            fail_on_warning: false,
        };
        // Run
        let result = run(args);
        assert!(result.is_ok());
        // The file should now be pretty-printed (spaces and newlines)
        let out = fs::read_to_string(&path).unwrap();
        assert!(out.contains("a = 1"));
        assert!(out.contains("b = 2"));
    }

    #[test]
    fn check_markdown_fix_applies() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("doc.md");
        let mut f = File::create(&path).unwrap();
        // CRLF line endings + trailing spaces + missing EOF newline + YAML frontmatter
        write!(f, "---\r\ntitle:foo\r\n---\r\nLine with space \r\n").unwrap();
        let args = CheckArgs {
            path: ".".to_string(),
            paths: vec![dir.path().to_string_lossy().to_string()],
            json: false,
            verbose: false,
            fix: true,
            fail_on_error: false,
            fail_on_warning: false,
        };
        let result = run(args);
        assert!(result.is_ok());
        let out = fs::read_to_string(&path).unwrap();
        // No CRLF
        assert!(!out.contains("\r\n"));
        // Trailing space removed
        assert!(!out.contains("Line with space "));
        // Contains frontmatter
        assert!(out.starts_with("---\n"));
        // End with newline
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn collect_project_files_respects_gitignore() {
        let dir = tempdir().unwrap();
        // Ignore "ignored.json" via .gitignore
        let gi = dir.path().join(".gitignore");
        fs::write(&gi, "ignored.json\n").unwrap();

        // Files
        let ignored = dir.path().join("ignored.json");
        fs::write(&ignored, "{\"a\":1,").unwrap(); // invalid json should be ignored
        let good = dir.path().join("good.json");
        fs::write(&good, "{\"a\":1}").unwrap();

        let skip_dirs = crate::utils::default_exclude_dir_names();
        let files = collect_project_files(dir.path(), &skip_dirs, false);
        let file_names: Vec<String> = files
            .iter()
            .filter_map(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
            })
            .collect();

        assert!(file_names.contains(&"good.json".to_string()));
        assert!(!file_names.contains(&"ignored.json".to_string()));
    }

    #[test]
    fn collect_project_files_skips_default_excluded_dirs() {
        let dir = tempdir().unwrap();
        let skip_dirs = crate::utils::default_exclude_dir_names();

        let dist_dir = dir.path().join("dist");
        std::fs::create_dir_all(&dist_dir).unwrap();
        let bad = dist_dir.join("bad.json");
        fs::write(&bad, "{\"a\":1,").unwrap();

        let files = collect_project_files(dir.path(), &skip_dirs, false);
        assert!(files.is_empty());
    }

    #[test]
    fn render_json_compact_only_shows_issues() {
        let r1 = FileResult {
            path: "a.json".to_string(),
            kind: "json".to_string(),
            valid: false,
            errors: vec!["err1".to_string()],
            warnings: vec![],
            fixed: false,
        };
        let r2 = FileResult {
            path: "b.json".to_string(),
            kind: "json".to_string(),
            valid: true,
            errors: vec![],
            warnings: vec!["warn1".to_string()],
            fixed: false,
        };
        let results = vec![r1, r2];
        let v = render_json_results(&results, false);
        let errors = v.get("errors").and_then(|e| e.as_array()).unwrap();
        let warnings = v.get("warnings").and_then(|w| w.as_array()).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            v.get("summary")
                .and_then(|s| s.get("error_count"))
                .and_then(|c| c.as_u64()),
            Some(1)
        );
        assert_eq!(
            v.get("summary")
                .and_then(|s| s.get("warning_count"))
                .and_then(|c| c.as_u64()),
            Some(1)
        );
    }

    #[test]
    fn render_json_verbose_returns_full_results() {
        let r = FileResult {
            path: "x.json".to_string(),
            kind: "json".to_string(),
            valid: false,
            errors: vec!["e".to_string()],
            warnings: vec!["w".to_string()],
            fixed: false,
        };
        let arr = vec![r];
        let v_verbose = render_json_results(&arr, true);
        let v_full = serde_json::to_value(&arr).unwrap();
        assert_eq!(v_verbose, v_full);
    }

    #[test]
    fn fail_on_error_causes_nonzero_exit() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        let mut f = File::create(&path).unwrap();
        writeln!(f, "{{ \"a\":1,\"b\":2,}}").unwrap();

        let args = CheckArgs {
            path: ".".to_string(),
            paths: vec![dir.path().to_string_lossy().to_string()],
            json: true,
            verbose: false,
            fix: false,
            fail_on_error: true,
            fail_on_warning: false,
        };

        let res = run(args);
        assert!(
            res.is_err(),
            "Expected run to return Err when errors found and --fail-on-error is set"
        );
    }

    #[test]
    fn fail_on_warning_causes_nonzero_exit() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("kam.toml");
        let content = r#"[prop]
id = "com.example.test"
name = "Example"
version = "v1.2.3"
versionCode = 1
description = "Example module"
"#;
        fs::write(&path, content).unwrap();

        let args = CheckArgs {
            path: ".".to_string(),
            paths: vec![dir.path().to_string_lossy().to_string()],
            json: true,
            verbose: false,
            fix: false,
            fail_on_error: false,
            fail_on_warning: true,
        };
        let res = run(args);
        assert!(
            res.is_err(),
            "Expected run to return Err when warnings found and fail_on_warning=true"
        );
    }

    #[test]
    fn check_single_file_positional() {
        // Ensure passing a single file via positional PATH works.
        let dir = tempdir().unwrap();
        let path = dir.path().join("single_bad.json");
        let mut f = File::create(&path).unwrap();
        // invalid JSON
        writeln!(f, "{{ \"a\":1, }}").unwrap();

        let args = CheckArgs {
            path: ".".to_string(),
            paths: vec![path.to_string_lossy().to_string()],
            json: true,
            verbose: false,
            fix: false,
            fail_on_error: true,
            fail_on_warning: false,
        };
        let res = run(args);
        assert!(
            res.is_err(),
            "Expected run to return Err when errors found and --fail-on-error is set"
        );
    }

    #[test]
    fn check_glob_pattern_matches_files() {
        // Ensure glob patterns match files and they get checked.
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.json");
        let mut fa = File::create(&a).unwrap();
        writeln!(fa, "{{ \"a\":1, }}").unwrap(); // invalid

        let b = dir.path().join("b.json");
        let mut fb = File::create(&b).unwrap();
        writeln!(fb, "{{ \"a\": 1 }}").unwrap(); // valid

        let pattern = format!("{}/{}", dir.path().to_string_lossy(), "*.json");
        let args = CheckArgs {
            path: ".".to_string(),
            paths: vec![pattern],
            json: true,
            verbose: false,
            fix: false,
            fail_on_error: true,
            fail_on_warning: false,
        };
        let res = run(args);
        assert!(
            res.is_err(),
            "Expected run to return Err when glob matches files with errors and --fail-on-error is set"
        );
    }
}
