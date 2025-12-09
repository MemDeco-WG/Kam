use colored::*;
use glob::glob;
use std::path::Path;
use std::time::Instant;

use super::args::BuildArgs;
use super::build_project::build_project;
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

struct BuildResult {
    member: String,
    success: bool,
    error: Option<String>,
}

fn build_workspace_member(project_path: &Path, member: &str, args: &BuildArgs) -> BuildResult {
    let member_path = project_path.join(member);
    if !member_path.exists() {
        Utils::warn(&format!("workspace member {} not found", member));
        return BuildResult {
            member: member.to_string(),
            success: false,
            error: Some("not found".to_string()),
        };
    }
    if !member_path.join("kam.toml").exists() {
        if !args.quiet {
            Utils::info(&format!("Skipping {}: no kam.toml found", member));
        }
        return BuildResult {
            member: member.to_string(),
            success: false,
            error: Some("no kam.toml found".to_string()),
        };
    }
    if !args.quiet {
        Utils::banner(&format!("Building workspace member: {}", member));
    }
    let original_cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            Utils::error(&format!("Failed to get current dir: {}", e));
            return BuildResult {
                member: member.to_string(),
                success: false,
                error: Some(format!("failed to get current dir: {}", e)),
            };
        }
    };
    if let Err(e) = std::env::set_current_dir(&member_path) {
        Utils::error(&format!(
            "Failed to change to {}: {}",
            member_path.display(),
            e
        ));
        return BuildResult {
            member: member.to_string(),
            success: false,
            error: Some(format!("failed to change directory: {}", e)),
        };
    }

    let result = match KamToml::load_from_dir(".") {
        Ok(kt) => match build_project(std::path::Path::new("."), args, Some(kt)) {
            Ok(_) => BuildResult {
                member: member.to_string(),
                success: true,
                error: None,
            },
            Err(e) => {
                Utils::error(&format!("Failed to build {}: {}", member, e));
                BuildResult {
                    member: member.to_string(),
                    success: false,
                    error: Some(e.to_string()),
                }
            }
        },
        Err(e) => {
            Utils::warn(&format!(
                "Skipping {}: failed to load kam.toml: {}",
                member, e
            ));
            BuildResult {
                member: member.to_string(),
                success: false,
                error: Some(format!("failed to load kam.toml: {}", e)),
            }
        }
    };

    if let Err(e) = std::env::set_current_dir(original_cwd) {
        Utils::warn(&format!("Failed to restore cwd: {}", e));
    }

    result
}

pub fn run_build_all(project_path: &Path, args: &BuildArgs) -> Result<(), KamError> {
    let start_time = Instant::now();
    let root_kam_toml = KamToml::load_from_dir(project_path)?;
    let workspace = root_kam_toml
        .kam
        .workspace
        .as_ref()
        .ok_or_else(|| KamError::InvalidConfig("No workspace section found".to_string()))?;

    let mut results = Vec::new();

    if let Some(members) = &workspace.members {
        // Expand glob patterns in workspace members
        let mut expanded_members = Vec::new();

        for member_pattern in members {
            // Check if pattern contains glob characters
            if member_pattern.contains('*')
                || member_pattern.contains('?')
                || member_pattern.contains('[')
            {
                // It's a glob pattern - expand it
                let pattern_path = project_path.join(member_pattern);
                let pattern_str = pattern_path.to_string_lossy();

                match glob(&pattern_str) {
                    Ok(paths) => {
                        for entry in paths.flatten() {
                            // Normalize the entry path to absolute
                            let abs_entry = if entry.is_absolute() {
                                entry.clone()
                            } else {
                                project_path.join(&entry)
                            };

                            // Only include directories that have kam.toml
                            if abs_entry.is_dir() && abs_entry.join("kam.toml").exists() {
                                if let Ok(rel_path) = abs_entry.strip_prefix(project_path) {
                                    let rel_str = rel_path.to_string_lossy().to_string();
                                    expanded_members.push(rel_str);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("Warning: invalid glob pattern '{}': {}", member_pattern, e);
                    }
                }
            } else {
                // Not a glob pattern - use as-is
                expanded_members.push(member_pattern.clone());
            }
        }

        if expanded_members.is_empty() {
            return Err(KamError::InvalidConfig(
                "No workspace members found after expanding patterns".to_string(),
            ));
        }

        for member in expanded_members {
            let result = build_workspace_member(project_path, &member, args);
            results.push(result);
        }
    } else {
        build_project(project_path, args, None)?;
        return Ok(());
    }

    let total_duration = start_time.elapsed();

    // Print summary
    if !args.quiet {
        println!();
        Utils::banner("Workspace Build Summary");
    }

    let success_count = results.iter().filter(|r| r.success).count();
    let failed_count = results.len() - success_count;

    if !args.quiet {
        for result in &results {
            if result.success {
                Utils::success(&result.member);
            } else {
                Utils::error(&format!(
                    "{} - {}",
                    result.member,
                    result
                        .error
                        .as_ref()
                        .unwrap_or(&"unknown error".to_string())
                ));
            }
        }
    }

    if !args.quiet {
        println!();
        Utils::kv(
            "Total",
            &format!(
                "{} | Success: {} | Failed: {}",
                results.len(),
                success_count,
                failed_count
            ),
        );
        Utils::kv(
            "Total time",
            &format!("{:.2}s", total_duration.as_secs_f64()),
        );
        println!("{}", "═".repeat(60).dimmed());
    }

    if failed_count > 0 {
        return Err(KamError::CommandFailed(format!(
            "{} workspace member(s) failed to build",
            failed_count
        )));
    }

    Ok(())
}
