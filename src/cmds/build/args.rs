use clap::Args;

#[allow(clippy::struct_excessive_bools)] // TODO: refactor BuildArgs: group boolean flags into enums/state and reduce number of bool fields
#[derive(Args, Debug, Clone)]
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

    /// Enable `KAM_BUMP_ENABLED` environment variable (set to 1)
    #[arg(short, long)]
    pub bump: bool,

    /// Enable `KAM_RELEASE_ENABLED` environment variable (set to 1)
    #[arg(short, long)]
    pub release: bool,

    /// Enable `KAM_SIGN_ENABLE` environment variable (set to 1)
    #[arg(short = 's', long)]
    pub sign: bool,

    /// Run build interactively; ask for confirmation when performing potentially destructive actions
    #[arg(short = 'i', long = "interactive")]
    pub interactive: bool,

    /// Enable `KAM_PRE_RELEASE` environment variable (set to 1)
    #[arg(short = 'P', long = "pre-release")]
    pub pre_release: bool,

    /// Suppress most output; show only warnings and errors
    #[arg(short, long)]
    pub quiet: bool,

    /// Number of parallel jobs (default: number of CPU cores)
    #[arg(short = 'j', long = "jobs")]
    pub jobs: Option<usize>,
}
