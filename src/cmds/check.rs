use clap::Args;
use colored::*;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::errors::KamError;

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Path to the project directory (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Output results as JSON
    #[arg(long, default_value_t = false)]
    pub json: bool,

    /// Try to automatically fix/format files
    #[arg(long, default_value_t = false)]
    pub fix: bool,
}

#[derive(Serialize, Debug)]
struct FileResult {
    path: String,
    kind: String,
    valid: bool,
    errors: Vec<String>,
    warnings: Vec<String>,
    fixed: bool,
}

pub fn run(args: CheckArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);
    if !project_path.exists() {
        return Err(KamError::InvalidDirectory(format!(
            "Path does not exist: {}",
            args.path
        )));
    }

    let mut results: Vec<FileResult> = Vec::new();

    let skip_dirs = [
        "target",
        ".git",
        "dist",
        "node_modules",
        "templates",
        "tmpl",
    ];
    for entry in walkdir::WalkDir::new(project_path)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                return !skip_dirs.contains(&name.as_ref());
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
                "md" => "markdown",
                _ => continue,
            };

            let res = check_file(path, kind, args.fix)?;
            results.push(res);
        }
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
            println!("  {} Errors:", "x".red());
            for e in &r.errors {
                println!("    - {}", e);
            }
        }
        if !r.warnings.is_empty() {
            println!("  {} Warnings:", "!".yellow());
            for w in &r.warnings {
                println!("    - {}", w);
            }
        }
    }

    if any_errors {
        println!("\n{} Some issues found.", "✕".red());
    } else {
        println!("\n{} No issues found.", "✓".green());
    }

    Ok(())
}

fn check_file(path: &Path, kind: &str, do_fix: bool) -> Result<FileResult, KamError> {
    let s = fs::read_to_string(path)?;
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
            match serde_json::from_str::<serde_json::Value>(&s) {
                Ok(v) => {
                    // If fix, reformat
                    if do_fix {
                        let pretty = serde_json::to_string_pretty(&v).unwrap_or_default();
                        if pretty != s {
                            fs::OpenOptions::new()
                                .write(true)
                                .truncate(true)
                                .open(path)?
                                .write_all(pretty.as_bytes())?;
                            fr.fixed = true;
                        }
                    }
                }
                Err(e) => {
                    fr.valid = false;
                    fr.errors.push(format!("JSON parse error: {}", e));
                    // Try to offer a fix by running a pretty attempt if do_fix: try to recover with serde_json::from_str? No.
                }
            }
        }
        "yaml" => match serde_yaml::from_str::<serde_yaml::Value>(&s) {
            Ok(v) => {
                if do_fix {
                    let pretty = serde_yaml::to_string(&v).unwrap_or_default();
                    if pretty != s {
                        fs::OpenOptions::new()
                            .write(true)
                            .truncate(true)
                            .open(path)?
                            .write_all(pretty.as_bytes())?;
                        fr.fixed = true;
                    }
                }
            }
            Err(e) => {
                fr.valid = false;
                fr.errors.push(format!("YAML parse error: {}", e));
            }
        },
        "markdown" => {
            // Minimal checks for markdown: frontmatter YAML, CRLF, trailing spaces
            let mut content = s.clone();
            // Check CRLF
            if content.contains("\r\n") {
                fr.warnings.push("CRLF line endings detected".to_string());
                if do_fix {
                    content = content.replace("\r\n", "\n");
                    fr.fixed = true;
                }
            }
            // Check trailing spaces
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

            // Front matter
            if content.starts_with("---\n") {
                if let Some(pos) = content[4..].find("\n---") {
                    let fm = &content[4..(4 + pos)];
                    match serde_yaml::from_str::<serde_yaml::Value>(fm) {
                        Ok(v) => {
                            if do_fix {
                                // Re-serialize frontmatter to normalized YAML
                                let pretty = serde_yaml::to_string(&v).unwrap_or_default();
                                let new =
                                    format!("---\n{}---\n{}", pretty, &content[(4 + pos + 5)..]);
                                if new != content {
                                    fs::OpenOptions::new()
                                        .write(true)
                                        .truncate(true)
                                        .open(path)?
                                        .write_all(new.as_bytes())?;
                                    fr.fixed = true;
                                }
                            }
                        }
                        Err(e) => {
                            fr.valid = false;
                            fr.errors
                                .push(format!("Markdown frontmatter YAML parse error: {}", e));
                        }
                    }
                } else {
                    fr.valid = false;
                    fr.errors.push(
                        "Markdown frontmatter start detected but end marker missing".to_string(),
                    );
                }
            }
            // Empty file
            if content.trim().is_empty() {
                fr.warnings.push("Empty markdown file".to_string());
            }
        }
        _ => {}
    }

    Ok(fr)
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
}
