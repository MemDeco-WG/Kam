use colored::*;
use std::path::Path;
use std::time::Instant;

use super::args::BuildArgs;
use super::build_project::build_project;
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;

struct BuildResult {
    member: String,
    success: bool,
    error: Option<String>,
}

fn build_workspace_member(project_path: &Path, member: &str, args: &BuildArgs) -> BuildResult {
    let member_path = project_path.join(member);
    if !member_path.exists() {
        println!("Warning: workspace member {} not found", member);
        return BuildResult {
            member: member.to_string(),
            success: false,
            error: Some("not found".to_string()),
        };
    }
    if !member_path.join("kam.toml").exists() {
        println!("Skipping {}: no kam.toml found", member);
        return BuildResult {
            member: member.to_string(),
            success: false,
            error: Some("no kam.toml found".to_string()),
        };
    }
    println!();
    println!(
        "{}",
        format!("Building workspace member: {}", member)
            .bold()
            .cyan()
    );
    let original_cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            println!("Failed to get current dir: {}", e);
            return BuildResult {
                member: member.to_string(),
                success: false,
                error: Some(format!("failed to get current dir: {}", e)),
            };
        }
    };
    if let Err(e) = std::env::set_current_dir(&member_path) {
        println!("Failed to change to {}: {}", member_path.display(), e);
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
                println!("Failed to build {}: {}", member, e);
                BuildResult {
                    member: member.to_string(),
                    success: false,
                    error: Some(e.to_string()),
                }
            }
        },
        Err(e) => {
            println!("Skipping {}: failed to load kam.toml: {}", member, e);
            BuildResult {
                member: member.to_string(),
                success: false,
                error: Some(format!("failed to load kam.toml: {}", e)),
            }
        }
    };

    if let Err(e) = std::env::set_current_dir(original_cwd) {
        println!("Failed to restore cwd: {}", e);
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
        for member in members {
            let result = build_workspace_member(project_path, member, args);
            results.push(result);
        }
    } else {
        build_project(project_path, args, None)?;
        return Ok(());
    }

    let total_duration = start_time.elapsed();

    // Print summary
    println!();
    println!("{}", "═".repeat(60).dimmed());
    println!("{}", "Workspace Build Summary".bold());
    println!("{}", "═".repeat(60).dimmed());

    let success_count = results.iter().filter(|r| r.success).count();
    let failed_count = results.len() - success_count;

    for result in &results {
        if result.success {
            println!("  {} {}", "✓".green(), result.member);
        } else {
            println!(
                "  {} {} - {}",
                "✗".red(),
                result.member,
                result
                    .error
                    .as_ref()
                    .unwrap_or(&"unknown error".to_string())
                    .dimmed()
            );
        }
    }

    println!();
    println!(
        "  {} Total: {} | {} Success: {} | {} Failed: {}",
        "•".cyan(),
        results.len(),
        "✓".green(),
        success_count,
        "✗".red(),
        failed_count
    );
    println!(
        "  {} Total time: {:.2}s",
        "•".cyan(),
        total_duration.as_secs_f64()
    );
    println!("{}", "═".repeat(60).dimmed());

    if failed_count > 0 {
        return Err(KamError::CommandFailed(format!(
            "{} workspace member(s) failed to build",
            failed_count
        )));
    }

    Ok(())
}
