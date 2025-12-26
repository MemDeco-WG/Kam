use clap::Args;
use clap::ValueEnum;
use clap_complete::shells;

#[derive(ValueEnum, Clone, Debug)]
pub enum ShellArg {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

impl ShellArg {
    pub const fn to_shell(&self) -> shells::Shell {
        match self {
            Self::Bash => shells::Shell::Bash,
            Self::Zsh => shells::Shell::Zsh,
            Self::Fish => shells::Shell::Fish,
            Self::PowerShell => shells::Shell::PowerShell,
            Self::Elvish => shells::Shell::Elvish,
        }
    }
}

#[derive(Args, Debug)]
pub struct CompletionArgs {
    /// Shell type for completion (bash,zsh,fish,powershell,elvish)
    #[arg(value_enum)]
    pub shell: ShellArg,

    /// Output file. If omitted, prints to STDOUT.
    #[arg(short, long)]
    pub out: Option<String>,

    /// Install the generated completion into the standard shell completion directory.
    /// When set, kam will attempt to write the generated completion into the
    /// system- or user-level completion directory appropriate for the chosen shell.
    #[arg(long = "install")]
    pub install: bool,
}
