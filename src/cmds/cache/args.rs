use clap::{Args, Subcommand};
use std::path::PathBuf;

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

    /// Manage module index cache (index_*.json and modules/<id>.json)
    Modules(ModuleCacheArgs),
}

#[derive(Args)]
pub struct ModuleCacheArgs {
    #[command(subcommand)]
    pub command: ModuleCacheCommands,
}

#[derive(Subcommand)]
pub enum ModuleCacheCommands {
    /// List module index and module detail cache files
    List,
    /// Clean module cache (remove index_*.json and modules/ directory)
    Clean,
    /// Show module cache directory path
    Path,
    /// Remove a specific cache file (exact filename or module id for modules/<id>.json)
    Remove {
        /// Filename or module id to remove
        name: String,
    },
}
