use std::path::Path;

use colored::*;

use super::build_project::run_command;
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;

pub fn handle_post_build_hook(kam_toml: &KamToml, project_path: &Path) -> Result<(), KamError> {
    // Run post-build hook
    if let Some(build_config) = &kam_toml.kam.build {
        if let Some(post_build) = &build_config.post_build {
            println!();
            println!("{}", "Running post-build hook...".yellow());
            run_command(post_build, project_path)?;
        }
    }
    Ok(())
}
