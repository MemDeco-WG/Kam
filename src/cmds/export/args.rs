use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ExportArgs {
    /// Export format: prop, json, update, repo, track, config
    #[arg(value_enum)]
    pub format: Option<ExportFormat>,
    /// Output file path (default: write to a format-specific filename in the current directory)
    pub output: Option<PathBuf>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ExportFormat {
    Prop,
    Json,
    Repo,
    Track,
    Config,
    Update,
}
