use std::path::Path;

use colored::*;

use super::build_project::run_command;
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;

pub fn handle_pre_build_hook(kam_toml: &KamToml, project_path: &Path) -> Result<(), KamError> {
    if let Some(pre_build) = kam_toml
        .kam
        .build
        .as_ref()
        .and_then(|b| b.pre_build.as_ref())
        .filter(|s| !s.trim().is_empty())
    {
        println!("{}", "Running pre-build hook...".yellow());
        run_command(pre_build, project_path)?;
        println!();
    }
    Ok(())
}
