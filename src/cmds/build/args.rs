use clap::Args;

#[derive(Args, Debug)]
pub struct BuildArgs {
    /// Path to the project (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Build all workspace members
    #[arg(short, long)]
    pub all: bool,

    /// Output directory (default: dist)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Enable KAM_BUMP_ENABLED environment variable (set to 1)
    #[arg(short, long)]
    pub bump: bool,

    /// Enable KAM_RELEASE_ENABLED environment variable (set to 1)
    #[arg(short, long)]
    pub release: bool,
}
