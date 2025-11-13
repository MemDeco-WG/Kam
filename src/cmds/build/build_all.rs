use std::path::Path;

use super::args::BuildArgs;
use super::build_project::build_project;
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;

fn build_workspace_member(project_path: &Path, member: &str, args: &BuildArgs) {
    let member_path = project_path.join(member);
    println!("DEBUG: member_path {} exists: {}", member_path.display(), member_path.exists());
    if !member_path.exists() {
        println!("Warning: workspace member {} not found", member);
        return;
    }
    if !member_path.join("kam.toml").exists() {
        println!("Skipping {}: no kam.toml found", member);
        return;
    }
    println!("Building workspace member: {}", member);
    let original_cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            println!("Failed to get current dir: {}", e);
            return;
        }
    };
    if let Err(e) = std::env::set_current_dir(&member_path) {
        println!("Failed to change to {}: {}", member_path.display(), e);
        return;
    }
    match KamToml::load_from_dir(".") {
        Ok(kt) => {
            if let Err(e) = build_project(std::path::Path::new("."), args, Some(kt)) {
                println!("Failed to build {}: {}", member, e);
            }
        }
        Err(e) => {
            println!("Skipping {}: failed to load kam.toml: {}", member, e);
        }
    }
    if let Err(e) = std::env::set_current_dir(original_cwd) {
        println!("Failed to restore cwd: {}", e);
    }
}

pub fn run_build_all(project_path: &Path, args: &BuildArgs) -> Result<(), KamError> {
    let root_kam_toml = KamToml::load_from_dir(project_path)?;
    println!("DEBUG: root_kam_toml.kam.workspace: {:?}", root_kam_toml.kam.workspace);
    let workspace = root_kam_toml
        .kam
        .workspace
        .as_ref()
        .ok_or_else(|| KamError::InvalidConfig("No workspace section found".to_string()))?;
    if let Some(members) = &workspace.members {
        for member in members {
            build_workspace_member(project_path, member, args);
        }
    } else {
        build_project(project_path, args, None)?;
    }
    Ok(())
}
