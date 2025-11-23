use crate::errors::KamError;
use crate::template::TemplateVariableProcessor;
use crate::types::modules::KamModule;
use crate::types::modules::ModuleBackend;
use crate::types::source::Source;
use crate::venv::{KamVenv, VenvType};
use clap::Args;
use colored::Colorize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
pub struct SyncArgs {
    /// Path to the project (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Include development dependencies
    #[arg(short, long)]
    pub dev: bool,

    /// Force re-sync even if up-to-date
    #[arg(short, long)]
    pub force: bool,
}

/// Run the sync command
///
/// ## Steps
///
/// 1. Load `kam.toml` configuration
/// 2. Ensure virtual environment exists
pub fn run(args: SyncArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);

    // Load kam.toml
    let kam_toml = crate::types::kam_toml::KamToml::load_from_dir(project_path)?;
    println!(
        "  {} {}",
        "✓".green(),
        format!("Loaded kam.toml for '{}'", kam_toml.prop.id).dimmed()
    );

    // Ensure virtual environment exists and is up-to-date.
    // Per project policy, `kam sync` should always ensure the venv is present
    // and refreshed. The dedicated `kam venv` command remains available for
    // manual management.
    println!();
    let venv_type = if args.dev { VenvType::Development } else { VenvType::Runtime };
    let venv_path = project_path.join(".kam_venv");
    let mut replacements = HashMap::new();
    replacements.insert("id".to_string(), kam_toml.prop.id.clone());
    replacements.insert("name".to_string(), 
        kam_toml.prop.name.get("en").unwrap_or(&kam_toml.prop.id).clone());
    replacements.insert("version".to_string(), kam_toml.prop.version.clone());
    replacements.insert("author".to_string(), kam_toml.prop.author.clone());

    let venv = if venv_path.exists() {
        let loaded = KamVenv::load(&venv_path)?;
        // Refresh the venv with current project properties
        KamVenv::create_with_replacements(&venv_path, venv_type, Some(replacements.clone()))?;
        loaded
    } else {
        // Create fresh venv
        KamVenv::create_with_replacements(&venv_path, venv_type, Some(replacements.clone()))?
    };
    println!(
        "  {} {}",
        "✓".green(),
        format!("Venv: {}", venv_path.display()).dimmed()
    );

    // Since the cache system has been removed, sync now only ensures the venv exists
    // Dependencies are no longer managed through the cache system
    println!();
    println!(
        "  {} {}",
        "ℹ".blue(),
        "Virtual environment sync completed (dependencies no longer managed via cache)".dimmed()
    );

    // Print activation instructions for the always-managed venv
    println!();
    println!("{} To activate the virtual environment:", "•".dimmed());
    println!("  {}: source .kam_venv/activate", "Unix".yellow());
    println!("  {}: .kam_venv\\activate.bat", "Windows".yellow());
    println!("  {}: .kam_venv\\activate.ps1", "PowerShell".yellow());

    Ok(())
}