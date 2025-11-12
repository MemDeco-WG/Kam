use clap::Args;

#[derive(Args, Debug)]
pub struct BuildArgs {
    /// Path to the project (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Build all workspace members
    #[arg(long)]
    pub all: bool,

    /// Output directory (default: dist)
    #[arg(short, long)]
    pub output: Option<String>,
}
