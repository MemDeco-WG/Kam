use clap::Args;

#[derive(Args, Debug)]
#[command(trailing_var_arg = true)]
pub struct CheckArgs {
    /// Path to use as project directory fallback when no PATHS are supplied
    /// (default: current directory). Use positional PATHS for files/globs.
    #[arg(long = "project-path", default_value = ".")]
    pub path: String,

    /// Paths or globs to check (file(s), directories, or glob patterns).
    /// If omitted, `path` (project directory) will be checked.
    #[arg(value_name = "PATHS", num_args = 0..)]
    pub paths: Vec<String>,

    /// Output results as JSON (compact by default; use -v/--verbose for detailed JSON)
    #[arg(long, default_value_t = false)]
    pub json: bool,

    /// Verbose JSON output (include full results). When using --json, this enables detailed output.
    #[arg(short = 'v', long, default_value_t = false)]
    pub verbose: bool,

    /// Try to automatically fix/format files
    #[arg(long, default_value_t = false)]
    pub fix: bool,

    /// Return a non-zero exit status when any errors are found (useful for CI)
    #[arg(long = "fail-on-error", default_value_t = false)]
    pub fail_on_error: bool,

    /// Return a non-zero exit status when any warnings are present (in addition to errors)
    #[arg(long = "fail-on-warning", default_value_t = false)]
    pub fail_on_warning: bool,
}
