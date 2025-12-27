use clap::Args;

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Path to use as project directory fallback when no PATHS are supplied
    /// (default: current directory). Use positional PATHS for files/globs.
    #[arg(long = "project-path", default_value = ".")]
    pub path: String,

    /// Paths or globs to check (file(s), directories, or glob patterns).
    /// If omitted, `path` (project directory) will be checked.
    #[arg(value_name = "PATHS", num_args = 0.., last = true)]
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

#[cfg(test)]
mod tests {
    use crate::cli::Cli;
    use crate::cli::Commands;
    use clap::Parser;

    #[test]
    fn default_flags_are_false() {
        let res = Cli::try_parse_from(["kam", "check"]);
        assert!(res.is_ok());
        let cli = res.unwrap();
        match cli.command {
            Some(Commands::Check(args)) => {
                assert!(!args.fail_on_error, "fail_on_error should default to false");
                assert!(
                    !args.fail_on_warning,
                    "fail_on_warning should default to false"
                );
            }
            _ => panic!("expected check subcommand"),
        }
    }

    #[test]
    fn parse_fail_on_error_flag() {
        let res = crate::cli::Cli::try_parse_from(["kam", "check", "--fail-on-error"]);
        assert!(res.is_ok());
        let cli = res.unwrap();
        match cli.command {
            Some(Commands::Check(args)) => {
                assert!(args.fail_on_error);
                assert!(!args.fail_on_warning);
            }
            _ => panic!("expected check subcommand"),
        }
    }

    #[test]
    fn parse_fail_on_warning_flag() {
        let res = crate::cli::Cli::try_parse_from(["kam", "check", "--fail-on-warning"]);
        assert!(res.is_ok());
        let cli = res.unwrap();
        match cli.command {
            Some(Commands::Check(args)) => {
                assert!(args.fail_on_warning);
                assert!(!args.fail_on_error);
            }
            _ => panic!("expected check subcommand"),
        }
    }

    #[test]
    fn parse_both_flags() {
        let res = crate::cli::Cli::try_parse_from([
            "kam",
            "check",
            "--fail-on-error",
            "--fail-on-warning",
        ]);
        assert!(res.is_ok());
        let cli = res.unwrap();
        match cli.command {
            Some(Commands::Check(args)) => {
                assert!(args.fail_on_error);
                assert!(args.fail_on_warning);
            }
            _ => panic!("expected check subcommand"),
        }
    }

    #[test]
    fn parse_paths_none_by_default() {
        let res = crate::cli::Cli::try_parse_from(["kam", "check"]);
        assert!(res.is_ok());
        let cli = res.unwrap();
        match cli.command {
            Some(Commands::Check(args)) => {
                assert!(args.paths.is_empty(), "paths should be empty when omitted");
                assert_eq!(
                    args.path, ".",
                    "path should default to current dir when omitted"
                );
            }
            _ => panic!("expected check subcommand"),
        }
    }

    #[test]
    fn parse_single_path() {
        let res = crate::cli::Cli::try_parse_from(["kam", "check", "file.json"]);
        assert!(res.is_ok(), "parse failed: {}", res.unwrap_err());
        let cli = res.unwrap();
        match cli.command {
            Some(Commands::Check(args)) => {
                assert_eq!(args.paths, vec!["file.json".to_string()]);
            }
            _ => panic!("expected check subcommand"),
        }
    }

    #[test]
    fn parse_multiple_paths_and_glob() {
        let res = crate::cli::Cli::try_parse_from(["kam", "check", "a.json", "*.md"]);
        assert!(res.is_ok(), "parse failed: {}", res.unwrap_err());
        let cli = res.unwrap();
        match cli.command {
            Some(Commands::Check(args)) => {
                assert_eq!(args.paths, vec!["a.json".to_string(), "*.md".to_string()]);
            }
            _ => panic!("expected check subcommand"),
        }
    }
}
