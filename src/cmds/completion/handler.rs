use clap::CommandFactory;
use clap_complete::{generate, shells};
use std::fs;
use std::path::Path;

use crate::cli::Cli;
use crate::errors::KamError;

use super::args::CompletionArgs;

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
    use crate::cmds::completion::args::ShellArg;
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
