use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::io::IsTerminal;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::args::CheckArgs;
use super::file::{FileResult, check_file};
use crate::errors::KamError;
use crate::types::kam_toml::RuleConfig;

fn collect_project_files(
    project_path: &Path,
    skip_dirs: &[String],
    respect_gitignore: bool,
) -> Vec<PathBuf> {
    // Optionally read top-level .gitignore and compile a basic include/exclude list.
    // This supports simple patterns and negations (lines starting with '!').
    // NOTE: we no longer respect .gitignore by default; callers must pass
    // `respect_gitignore = true` when they want .gitignore processing.
    let mut gi_patterns: Vec<String> = Vec::new();
    let mut gi_whitelist: Vec<String> = Vec::new();
    if respect_gitignore {
        let gitignore_file = project_path.join(".gitignore");
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
    }

    // Traverse project files using ignore::WalkBuilder so that .gitignore and VCS ignores are respected.
    // Also apply our default skip directory filter for common build/artifact dirs.
    let mut files: Vec<PathBuf> = Vec::new();
    let skip_clone = skip_dirs.to_owned();

    let walker = ignore::WalkBuilder::new(project_path)
        .git_ignore(respect_gitignore)
        .hidden(false)
        .filter_entry(move |entry| {
            // Keep root
            if entry.depth() == 0 {
                return true;
            }
            if entry.file_type().is_some_and(|f| f.is_dir()) {
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
        if project_path.join("kam.toml").exists()
            && path.file_name().and_then(|n| n.to_str()) == Some("kam.toml")
        {
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

/// # Errors
///
/// Returns `KamError` when I/O operations, parsing errors, or external tool
/// invocations fail while running the check command.
///
/// # Notes
///
/// This function is currently large and handles several responsibilities
/// (glob expansion, pre-checks, collection, result aggregation, output).
/// Consider splitting it into smaller helpers in a follow-up refactor.
#[allow(clippy::too_many_lines)] // TODO: split this function into smaller helpers
pub fn run(args: &CheckArgs) -> Result<(), KamError> {
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
                    crate::utils::Utils::info(format!("shellcheck detected: {ver}"));
                } else {
                    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    crate::utils::Utils::warn(format!(
                        "shellcheck present but '--version' returned non-zero: {err}"
                    ));
                }
            }
            Err(e) => {
                crate::utils::Utils::warn(format!(
                    "shellcheck not found or cannot be executed: {e}"
                ));
            }
        }
    }

    let mut results: Vec<FileResult> = Vec::new();

    // 获取默认排除目录，再加几个常见的
    // 这些目录通常不需要检查（比如构建产物、模板等）
    let mut skip_dirs = crate::utils::default_exclude_dir_names();
    for d in &["dist", "templates", "tmpl"] {
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
                            let mut respect_gitignore = false;
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
                                    respect_gitignore = kt
                                        .kam
                                        .build
                                        .as_ref()
                                        .and_then(|b| b.respect_gitignore)
                                        .unwrap_or(false);
                                }
                            }
                            let dir_files =
                                collect_project_files(&entry, &skip_dirs, respect_gitignore);
                            collected_files.extend(dir_files);
                        } else if entry.is_file() {
                            collected_files.push(entry);
                        }
                    }
                }
                Err(e) => {
                    crate::utils::Utils::warn(format!("Invalid glob pattern '{p}': {e}"));
                }
            }
        } else {
            let pathp = Path::new(&p);
            if pathp.exists() {
                if pathp.is_dir() {
                    let mut respect_gitignore = false;
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
                            respect_gitignore = kt
                                .kam
                                .build
                                .as_ref()
                                .and_then(|b| b.respect_gitignore)
                                .unwrap_or(false);
                        }
                    }
                    let dir_files = collect_project_files(pathp, &skip_dirs, respect_gitignore);
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
        .unwrap_or_else(|_| ProgressStyle::default_bar())
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

        let project_rules = find_project_rules(path, &project_rules_cache);
        let res = check_file(path, kind, args.fix, project_rules)?;
        results.push(res);
        if let Some(ref p) = pb {
            p.set_message(path.display().to_string());
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
            // Compact one-line JSON for minimal token/output size
            serde_json::to_string(&jv).unwrap_or_else(|_| "{}".into())
        };
        println!("{out}");

        // Compute totals so JSON mode honors fail-on-error / fail-on-warning flags.
        let mut total_errors: usize = 0;
        let mut total_warnings: usize = 0;
        for r in &results {
            total_errors += r.errors.len();
            total_warnings += r.warnings.len();
        }

        if args.fail_on_error && total_errors > 0 {
            return Err(KamError::CommandFailed(format!(
                "check failed: {total_errors} error(s) found"
            )));
        }
        if args.fail_on_warning && total_warnings > 0 {
            return Err(KamError::CommandFailed(format!(
                "check failed: {total_warnings} warning(s) found"
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
                println!("{} {}", "✕".yellow(), trf!("common.file_fixed", path));
            }
            (false, false) => {
                any_errors = true;
                println!("{} {}", "✕".color(crate::utils::Utils::error_color()), path);
            }
        }

        if !r.errors.is_empty() {
            let c = crate::utils::Utils::error_color();
            println!(
                "  {} {}",
                "✗".color(c).bold(),
                crate::i18n::tr("check.errors.header").color(c).bold()
            );
            for e in &r.errors {
                println!("    {} {}", "→".color(c).dimmed(), e.color(c));
            }
        }
        if !r.warnings.is_empty() {
            println!(
                "  {} {}",
                "!".yellow().bold(),
                crate::i18n::tr("check.warnings.header").yellow().bold()
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
            crate::i18n::tr("check.some_issues_found").color(c)
        );
    } else {
        println!(
            "\n{} {}",
            "✓".green().bold(),
            crate::i18n::tr("check.no_issues_found").green()
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
            "check failed: {total_errors} error(s) found"
        )));
    }
    if args.fail_on_warning && total_warnings > 0 {
        return Err(KamError::CommandFailed(format!(
            "check failed: {total_warnings} warning(s) found"
        )));
    }

    Ok(())
}
