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
}
