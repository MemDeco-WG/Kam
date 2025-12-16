use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
#[cfg(test)]
use std::fs;
use std::io::IsTerminal;
#[cfg(test)]
use std::io::Write;
use std::path::{Path, PathBuf};

use super::args::CheckArgs;
use super::file::{FileResult, check_file};
use crate::errors::KamError;

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
        }
    }

    files
}

pub fn run(args: CheckArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);
    if !project_path.exists() {
        return Err(KamError::InvalidDirectory(trf!(
            "Path does not exist: {}",
            args.path
        )));
    }

    let mut results: Vec<FileResult> = Vec::new();

    // 检测项目类型：是否是 Kam 项目
    let is_kam_project = project_path.join("kam.toml").exists();

    // 如果是 Kam 项目，优先检查 kam.toml
    if is_kam_project {
        let kam_toml_path = project_path.join("kam.toml");
        if let Ok(res) = super::file::check_file(&kam_toml_path, "toml", args.fix) {
            results.push(res);
        }
    }

    // 获取默认排除目录，再加几个常见的
    // 这些目录通常不需要检查（比如构建产物、模板等）
    let mut skip_dirs = crate::utils::default_exclude_dir_names();
    for d in ["dist", "templates", "tmpl"].iter() {
        if !skip_dirs.iter().any(|s| s == d) {
            skip_dirs.push(d.to_string());
        }
    }

    // 如果是 Kam 项目，根据配置调整检查范围
    if is_kam_project
        && let Ok(_kam_toml) = crate::types::kam_toml::KamToml::load_from_dir(project_path)
    {
        // 如果配置了 source_dir，可以优先检查该目录
        // 但这里我们仍然检查整个项目，只是跳过一些不必要的目录
        // 可以添加基于 build 配置的智能过滤逻辑
        // 例如：如果配置了 exclude，可以跳过这些文件
    }
    // 虽然可能漏掉一些，但至少能跳过大部分不需要检查的目录
    // Collect files while respecting .gitignore and our skip list
    let files = collect_project_files(project_path, &skip_dirs, is_kam_project);

    // 文件总数（包含已单独检查的 kam.toml）
    let mut total_files: usize = files.len();
    if is_kam_project {
        total_files += 1;
    }

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
        // 如果已经检查过 kam.toml，进度条从 1 开始
        if is_kam_project {
            pb.inc(1);
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
            continue;
        };

        let res = check_file(path, kind, args.fix)?;
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
        let out = serde_json::to_string_pretty(&results).unwrap_or_else(|_| "[]".into());
        println!("{}", out);
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
                println!("{} {}", "✕".red(), path)
            }
        }

        if !r.errors.is_empty() {
            println!(
                "  {} {}",
                "✗".red().bold(),
                crate::i18n::tr_key("check.errors.header").red().bold()
            );
            for e in &r.errors {
                println!("    {} {}", "→".red().dimmed(), e.red());
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
        println!(
            "\n{} {}",
            "✕".red().bold(),
            crate::i18n::tr_key("check.some_issues_found").red()
        );
    } else {
        println!(
            "\n{} {}",
            "✓".green().bold(),
            crate::i18n::tr_key("check.no_issues_found").green()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn check_invalid_json_and_fix() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        let mut f = File::create(&path).unwrap();
        writeln!(f, "{{\"a\":1,\"b\":2,").unwrap();
        let args = CheckArgs {
            path: dir.path().to_string_lossy().to_string(),
            json: true,
            fix: false,
        };
        // Run
        let result = run(args);
        assert!(result.is_ok());
    }

    #[test]
    fn check_json_fix_applies() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("obj.json");
        let mut f = File::create(&path).unwrap();
        // intentionally compact/unformatted JSON
        writeln!(f, "{{\"a\":1,\"b\":2}} ").unwrap();
        let args = CheckArgs {
            path: dir.path().to_string_lossy().to_string(),
            json: false,
            fix: true,
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
            path: dir.path().to_string_lossy().to_string(),
            json: true,
            fix: false,
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
            path: dir.path().to_string_lossy().to_string(),
            json: false,
            fix: true,
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
            path: dir.path().to_string_lossy().to_string(),
            json: false,
            fix: true,
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
}
