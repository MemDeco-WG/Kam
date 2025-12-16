use clap::Args;

/// Print this message or the help of the given subcommand(s)
#[derive(Args, Debug, Clone)]
pub struct HelpArgs {
    /// Subcommand path to show help for (e.g. `tmpl import`).
    /// Provide zero or more names; when omitted the top-level help is shown.
    #[arg(value_name = "SUBCOMMAND", num_args = 0..)]
    pub subcommand: Vec<String>,
}
