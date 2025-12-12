use clap::Args;

#[derive(Args)]
pub struct ValidateArgs {
    /// Path to the project directory (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,
}
