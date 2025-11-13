use clap::Args;
use colored::Colorize;

use comrak::{Arena, format_commonmark, markdown_to_html, parse_document, Options};
use ignore::WalkBuilder;
use serde_json;
use serde_yaml;
use std::fs;
use std::path::Path;
use toml;

use crate::errors::KamError;

/// Arguments for the check command
#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Automatically fix issues where possible
    #[arg(long)]
    fix: bool,
    /// Specific files to check (if not specified, check all non-hidden files)
    #[arg()]
    files: Vec<String>,
}

/// Check result for a file
#[derive(Debug)]
struct CheckResult {
    file: String,
    issues: Vec<String>,
    fixed_count: usize,
}

/// Run the check command
pub fn run(args: CheckArgs) -> Result<(), KamError> {
    println!("{} Checking project files...", "→".cyan());

    let mut results = Vec::new();

    if args.files.is_empty() {
        // Check all non-hidden files
        let walker = WalkBuilder::new(".")
            .git_ignore(true)
            .hidden(true) // Ignore hidden files by default
            .build();

        for entry in walker {
            let entry = entry.map_err(|e| KamError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
            let path = entry.path();
            if path.is_file() {
                // Skip files in .git directory
                if path.components().any(|c| c.as_os_str() == ".git") {
                    continue;
                }
                let res = check_file(path, args.fix)?;
                if !res.issues.is_empty() {
                    results.push(res);
                }
            }
        }
    } else {
        // Check specific files
        for file in &args.files {
            let path = std::path::Path::new(file);
            if path.exists() && path.is_file() {
                let res = check_file(path, args.fix)?;
                if !res.issues.is_empty() {
                    results.push(res);
                }
            } else {
                println!("{} File not found: {}", "!".yellow(), file);
            }
        }
    }

    let total_issues: usize = results.iter().map(|r| r.issues.len()).sum();
    let total_fixed: usize = results.iter().map(|r| r.fixed_count).sum();
    let remaining_issues = total_issues - total_fixed;

    if results.is_empty() {
        println!("{} No issues found.", "✓".green());
    } else {
        println!("{} Found {} issues in {} files.", "Summary:".yellow(), total_issues, results.len());
        if args.fix {
            println!("{} Fixed {} issues, {} remaining.", "✓".green(), total_fixed, remaining_issues);
        }

        for res in &results {
            println!("{} {}", "File:".yellow(), res.file);
            for issue in &res.issues {
                println!("  - {}", issue);
            }
        }

        if !args.fix {
            println!("\n{} Run with --fix to automatically fix issues.", "Hint:".dimmed());
        }
    }

    Ok(())
}

/// Check a single file
fn check_file(path: &Path, fix: bool) -> Result<CheckResult, KamError> {
    let mut issues = Vec::new();
    let mut fixed_count = 0;
    let content = fs::read(path).map_err(KamError::Io)?;

    // Check encoding (assume UTF-8, check if valid)
    if std::str::from_utf8(&content).is_err() {
        issues.push("File is not valid UTF-8".to_string());
        // Cannot fix automatically
    }

    // Check line endings
    let content_str = String::from_utf8_lossy(&content);
    if content_str.contains("\r\n") {
        issues.push("Line endings are CRLF instead of LF".to_string());
        if fix {
            let fixed = content_str.replace("\r\n", "\n");
            fs::write(path, fixed.as_bytes()).map_err(KamError::Io)?;
            fixed_count += 1;
        }
    } else if content_str.contains('\r') {
        issues.push("Line endings contain CR".to_string());
        if fix {
            let fixed = content_str.replace('\r', "");
            fs::write(path, fixed.as_bytes()).map_err(KamError::Io)?;
            fixed_count += 1;
        }
    }

    // Check syntax based on extension
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext {
            "toml" => {
                if toml::from_str::<toml::Value>(&content_str).is_err() {
                    issues.push("Invalid TOML syntax".to_string());
                }
            }
            "json" => {
                if serde_json::from_str::<serde_json::Value>(&content_str).is_err() {
                    issues.push("Invalid JSON syntax".to_string());
                }
            }
            "yaml" | "yml" => {
                if serde_yaml::from_str::<serde_yaml::Value>(&content_str).is_err() {
                    issues.push("Invalid YAML syntax".to_string());
                }
            }
            "md" => {
                // Check Markdown syntax using comrak
                let mut options = Options::default();
                options.extension.table = true;
                options.extension.footnotes = true;
                options.extension.strikethrough = true;
                options.extension.tasklist = true;
                // comrak parses and renders to HTML; if parsing succeeds, assume syntax is valid
                // comrak doesn't report syntax errors explicitly, but ensures valid CommonMark parsing
                let _html = markdown_to_html(&content_str, &options);

                // Check if Markdown needs reformatting
                let arena = Arena::new();
                let root = parse_document(&arena, &content_str, &options);
                let mut output = String::new();
                format_commonmark(root, &options, &mut output).unwrap();
                if output != content_str {
                    issues.push("Markdown file needs reformatting".to_string());
                    if fix {
                        fs::write(path, output.as_bytes()).map_err(KamError::Io)?;
                        fixed_count += 1;
                    }
                }
            }
            _ => {} // Skip other files
        }
    }

    Ok(CheckResult {
        file: path.display().to_string(),
        issues,
        fixed_count,
    })
}
