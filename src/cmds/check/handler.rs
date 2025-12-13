use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
#[cfg(test)]
use std::fs;
use std::io::IsTerminal;
#[cfg(test)]
use std::io::Write;
use std::path::Path;

use super::args::CheckArgs;
use super::file::{FileResult, check_file};
use crate::errors::KamError;

pub fn run(args: CheckArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);
    if !project_path.exists() {
        return Err(KamError::InvalidDirectory(format!(
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
    if is_kam_project {
        if let Ok(kam_toml) = crate::types::kam_toml::KamToml::load_from_dir(project_path) {
            // 如果配置了 source_dir，可以优先检查该目录
            // 但这里我们仍然检查整个项目，只是跳过一些不必要的目录
            if let Some(build) = &kam_toml.kam.build {
                // 可以添加基于 build 配置的智能过滤逻辑
                // 例如：如果配置了 exclude，可以跳过这些文件
            }
        }
    }
    // 虽然可能漏掉一些，但至少能跳过大部分不需要检查的目录
    // 第一遍：统计匹配支持扩展名的文件总数
    // 这样进度条才能显示准确的进度
    let mut total_files: usize = 0;
    for entry in walkdir::WalkDir::new(project_path)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                return !skip_dirs.iter().any(|s| s == name.as_ref());
            }
            true
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext.to_lowercase().as_str() {
                "json" | "yml" | "yaml" | "toml" | "sh" | "bash" | "md" => total_files += 1,
                _ => {}
            }
        }
    }

    // 只在非JSON输出且是终端时显示进度条
    // JSON输出时进度条会干扰输出格式
    let show_progress = !args.json && std::io::stdout().is_terminal();
    let pb = if show_progress && total_files > 0 {
        let pb = ProgressBar::new(total_files as u64);
        let style = ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
        )
        .unwrap()
        .progress_chars("#>-");  // 进度条字符，看起来比较好看
        pb.set_style(style);
        Some(pb)
    } else {
        None
    };

    for entry in walkdir::WalkDir::new(project_path)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                return !skip_dirs.iter().any(|s| s == name.as_ref());
            }
            true
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let kind = match ext.to_lowercase().as_str() {
                "json" => "json",
                "yml" | "yaml" => "yaml",
                "toml" => "toml",
                "sh" | "bash" => "sh",
                "md" => "markdown",
                _ => continue,
            };

            // 如果是 Kam 项目且已经检查过 kam.toml，跳过它
            if is_kam_project && path.file_name().and_then(|n| n.to_str()) == Some("kam.toml") {
                continue;
            }
            
            let res = check_file(path, kind, args.fix)?;
            results.push(res);
            if let Some(ref p) = pb {
                p.set_message(format!("{}", path.display()));
                p.inc(1);
            }
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
            (true, true) => println!("{} {} (fixed)", "✓".green(), path),
            (true, false) => println!("{} {}", "✓".green(), path),
            (false, true) => {
                any_errors = true;
                println!("{} {} (fixed)", "✕".yellow(), path)
            }
            (false, false) => {
                any_errors = true;
                println!("{} {}", "✕".red(), path)
            }
        }

        if !r.errors.is_empty() {
            println!("  {} {}", "✗".red().bold(), "Errors:".red().bold());
            for e in &r.errors {
                println!("    {} {}", "→".red().dimmed(), e.red());
            }
        }
        if !r.warnings.is_empty() {
            println!("  {} {}", "!".yellow().bold(), "Warnings:".yellow().bold());
            for w in &r.warnings {
                println!("    {} {}", "→".yellow().dimmed(), w.yellow());
            }
        }
    }

    if any_errors {
        println!("\n{} {}", "✕".red().bold(), "Some issues found.".red());
    } else {
        println!("\n{} {}", "✓".green().bold(), "No issues found.".green());
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
}
