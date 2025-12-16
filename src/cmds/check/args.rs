use clap::Args;

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Path to the project directory (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Output results as JSON (compact by default; use -v/--verbose for detailed JSON)
    #[arg(long, default_value_t = false)]
    pub json: bool,

    /// Verbose JSON output (include full results). When using --json, this enables detailed output.
    #[arg(short = 'v', long, default_value_t = false)]
    pub verbose: bool,

    /// Try to automatically fix/format files
    #[arg(long, default_value_t = false)]
    pub fix: bool,
}
