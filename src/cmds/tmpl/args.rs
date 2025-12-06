use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Args)]
pub struct TmplArgs {
    #[command(subcommand)]
    pub command: TmplCommands,
}

#[derive(Subcommand)]
pub enum TmplCommands {
    /// List all available templates
    List,

    /// Import template(s) from file
    Import {
        /// Path to template archive (.tar.gz for single template, .zip for multiple templates)
        path: PathBuf,

        /// Template name (optional, will use filename if not provided)
        #[arg(short, long)]
        name: Option<String>,

        /// Force overwrite if template already exists
        #[arg(short, long)]
        force: bool,
    },

    /// Export template(s) to file
    Export {
        /// Template name(s) to export (can specify multiple)
        templates: Vec<String>,

        /// Output file path (.tar.gz for single template, .zip for multiple templates)
        #[arg(short, long)]
        output: PathBuf,

        /// Force overwrite if output file already exists
        #[arg(short, long)]
        force: bool,
    },

    /// Remove a template from cache
    Remove {
        /// Template name to remove
        name: String,
    },

    /// Show template cache directory path
    Path,
}
