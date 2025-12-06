use clap::{Args, Subcommand};
use colored::*;
use std::path::PathBuf;

use crate::errors::KamError;
use crate::template::TemplateCacheManager;

#[derive(Args)]
pub struct CacheArgs {
    #[command(subcommand)]
    pub command: CacheCommands,
}

#[derive(Subcommand)]
pub enum CacheCommands {
    /// List cached templates
    List,
    /// Clean all cached templates
    Clean,
    /// Add a template to cache from a local directory or archive
    Add {
        /// Name of the template to register
        name: String,
        /// Path to the template source (directory or archive)
        path: PathBuf,
    },
    /// Remove a template from cache
    Remove {
        /// Name of the template to remove
        name: String,
    },
    /// Show cache directory path
    Path,
}

pub fn run(args: CacheArgs) -> Result<(), KamError> {
    match args.command {
        CacheCommands::List => {
            let templates = TemplateCacheManager::list_local_templates()?;
            if templates.is_empty() {
                println!("No templates found in local cache.");
            } else {
                println!("Local cached templates:");
                for tmpl in templates {
                    println!("  {} {}", "•".cyan(), tmpl);
                }
            }
        }
        CacheCommands::Clean => {
            let cache_dir = TemplateCacheManager::get_cache_dir()?;
            if cache_dir.exists() {
                // We remove the contents, or the dir itself and recreate it
                std::fs::remove_dir_all(&cache_dir).map_err(KamError::Io)?;
                std::fs::create_dir_all(&cache_dir).map_err(KamError::Io)?;
                println!("{} Cache cleaned successfully.", "✓".green());
            } else {
                println!("Cache directory is already empty or does not exist.");
            }
        }
        CacheCommands::Add { name, path } => {
            TemplateCacheManager::install_template(&name, &path)?;
            println!(
                "{} Template '{}' added to cache from {}",
                "✓".green(),
                name,
                path.display()
            );
        }
        CacheCommands::Remove { name } => {
            TemplateCacheManager::remove_template(&name)?;
            println!("{} Template '{}' removed from cache.", "✓".green(), name);
        }
        CacheCommands::Path => {
            let cache_dir = TemplateCacheManager::get_cache_dir()?;
            if let Some(root) = cache_dir.parent() {
                println!("{}", root.display());
            } else {
                println!("{}", cache_dir.display());
            }
        }
    }

    Ok(())
}
