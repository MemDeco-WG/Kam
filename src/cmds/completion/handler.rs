use clap::CommandFactory;
use clap_complete::{generate, shells};
use std::fs;
use std::path::Path;

use crate::cli::Cli;
use crate::errors::KamError;

use super::args::CompletionArgs;

// 生成shell补全脚本
// 支持bash、zsh、fish、powershell、elvish等
pub fn run(args: CompletionArgs) -> Result<(), KamError> {
    // 从顶层CLI构建clap命令
    let mut cmd = Cli::command();
    let shell = args.shell.to_shell();

    if let Some(outpath) = args.out {
        // 输出到文件
        let p = Path::new(&outpath);
        // 确保父目录存在（如果指定了的话）
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::new();
        // 根据shell类型生成对应的补全脚本
        match shell {
            shells::Shell::Bash => generate(shells::Bash, &mut cmd, "kam", &mut buf),
            shells::Shell::Zsh => generate(shells::Zsh, &mut cmd, "kam", &mut buf),
            shells::Shell::Fish => generate(shells::Fish, &mut cmd, "kam", &mut buf),
            shells::Shell::PowerShell => generate(shells::PowerShell, &mut cmd, "kam", &mut buf),
            shells::Shell::Elvish => generate(shells::Elvish, &mut cmd, "kam", &mut buf),
            _ => generate(shells::Bash, &mut cmd, "kam", &mut buf),  // 默认用bash
        }
        fs::write(p, &buf)?;
    } else {
        // 输出到stdout（方便用户直接重定向）
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
