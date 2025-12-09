use clap::Args;

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Path to the project directory (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Output results as JSON
    #[arg(long, default_value_t = false)]
    pub json: bool,

    /// Try to automatically fix/format files
    #[arg(long, default_value_t = false)]
    pub fix: bool,
}
