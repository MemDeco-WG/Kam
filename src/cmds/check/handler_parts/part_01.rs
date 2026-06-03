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

