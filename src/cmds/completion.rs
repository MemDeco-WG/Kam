use crate::cli::Cli;
use clap::Args;
use clap::CommandFactory;
use clap_complete::{generate, shells};
use std::fs;
use std::path::Path;

use crate::errors::KamError;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ShellArg {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

impl ShellArg {
    fn to_shell(&self) -> shells::Shell {
        match self {
            ShellArg::Bash => shells::Shell::Bash,
            ShellArg::Zsh => shells::Shell::Zsh,
            ShellArg::Fish => shells::Shell::Fish,
            ShellArg::PowerShell => shells::Shell::PowerShell,
            ShellArg::Elvish => shells::Shell::Elvish,
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
}

pub fn run(args: CompletionArgs) -> Result<(), KamError> {
    // Build clap command from the top-level CLI
    let mut cmd = Cli::command();
    let shell = args.shell.to_shell();

    if let Some(outpath) = args.out {
        let p = Path::new(&outpath);
        // Ensure parent dir exists if specified
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::new();
        match shell {
            shells::Shell::Bash => generate(shells::Bash, &mut cmd, "kam", &mut buf),
            shells::Shell::Zsh => generate(shells::Zsh, &mut cmd, "kam", &mut buf),
            shells::Shell::Fish => generate(shells::Fish, &mut cmd, "kam", &mut buf),
            shells::Shell::PowerShell => generate(shells::PowerShell, &mut cmd, "kam", &mut buf),
            shells::Shell::Elvish => generate(shells::Elvish, &mut cmd, "kam", &mut buf),
            _ => generate(shells::Bash, &mut cmd, "kam", &mut buf),
        }
        fs::write(p, &buf)?;
    } else {
        let mut stdout = std::io::stdout();
        match shell {
            shells::Shell::Bash => generate(shells::Bash, &mut cmd, "kam", &mut stdout),
            shells::Shell::Zsh => generate(shells::Zsh, &mut cmd, "kam", &mut stdout),
            shells::Shell::Fish => generate(shells::Fish, &mut cmd, "kam", &mut stdout),
            shells::Shell::PowerShell => generate(shells::PowerShell, &mut cmd, "kam", &mut stdout),
            shells::Shell::Elvish => generate(shells::Elvish, &mut cmd, "kam", &mut stdout),
            _ => generate(shells::Bash, &mut cmd, "kam", &mut stdout),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn can_generate_bash_completion_to_file() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("kam.bash");
        let args = CompletionArgs {
            shell: ShellArg::Bash,
            out: Some(out.to_string_lossy().to_string()),
        };
        assert!(run(args).is_ok());
        assert!(out.exists());
        let content = std::fs::read_to_string(out).unwrap();
        assert!(content.contains("_kam()"));
    }
}
