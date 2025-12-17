use clap::CommandFactory;
use clap_complete::{generate, shells};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::Cli;
use crate::errors::KamError;

use super::args::CompletionArgs;

// 生成shell补全脚本并可选择将其安装到合适的补全目录
// 支持bash、zsh、fish、powershell、elvish等
pub fn run(args: CompletionArgs) -> Result<(), KamError> {
    // 从顶层CLI构建clap命令
    let mut cmd = Cli::command();
    let shell = args.shell.to_shell();

    // 先生成到内存缓冲
    let mut buf = Vec::new();
    match shell {
        shells::Shell::Bash => generate(shells::Bash, &mut cmd, "kam", &mut buf),
        shells::Shell::Zsh => generate(shells::Zsh, &mut cmd, "kam", &mut buf),
        shells::Shell::Fish => generate(shells::Fish, &mut cmd, "kam", &mut buf),
        shells::Shell::PowerShell => generate(shells::PowerShell, &mut cmd, "kam", &mut buf),
        shells::Shell::Elvish => generate(shells::Elvish, &mut cmd, "kam", &mut buf),
        _ => generate(shells::Bash, &mut cmd, "kam", &mut buf),
    }

    // 如果用户指定了 -o/--out，先写到指定文件
    if let Some(outpath) = args.out.clone() {
        let p = Path::new(&outpath);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(p, &buf)?;
    }

    // 如果用户指定了 --install，则尝试把补全脚本安装到系统/用户的补全目录中
    if args.install {
        match install_completion(shell, &buf) {
            Ok(p) => println!("Installed completion to: {}", p.display()),
            Err(e) => {
                // 将直接的 IO 错误转换为 KamError 以保持一致的错误处理行为
                return Err(KamError::Io(e));
            }
        }
    }

    // 如果既没有 -o 也没有 --install，则输出到 stdout（便于重定向）
    if args.out.is_none() && !args.install {
        use std::io::Write;
        std::io::stdout().write_all(&buf)?;
    }

    Ok(())
}

/// 依据 shell 生成候选安装路径并尝试逐一写入，成功则返回写入的路径
fn install_completion(shell: shells::Shell, buf: &[u8]) -> Result<PathBuf, std::io::Error> {
    let candidates = default_completion_candidates(shell);

    let mut last_err: Option<std::io::Error> = None;
    for path in candidates {
        // 尝试创建父目录（若需要）
        if let Some(parent) = path.parent() {
            match fs::create_dir_all(parent) {
                Ok(_) => {}
                Err(e) => {
                    // 无法创建父目录（很可能是权限问题），记录并尝试下一个候选
                    last_err = Some(e);
                    continue;
                }
            }
        }

        // 尝试写入文件
        match fs::write(&path, buf) {
            Ok(_) => return Ok(path),
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        std::io::Error::other(
            "no completion installation candidates available",
        )
    }))
}

/// 根据 shell 返回有序的候选安装路径（倾向于先尝试系统位置，失败时回退到用户目录）
fn default_completion_candidates(shell: shells::Shell) -> Vec<PathBuf> {
    let mut c: Vec<PathBuf> = Vec::new();

    let home = dirs::home_dir();
    let xdg_data = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
    let xdg_config = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| home.as_ref().map(|h| h.join(".config")));

    match shell {
        shells::Shell::Bash => {
            c.push(PathBuf::from("/usr/share/bash-completion/completions/kam"));
            c.push(PathBuf::from("/etc/bash_completion.d/kam"));
            if let Some(ref d) = xdg_data {
                c.push(d.join("bash-completion/completions/kam"));
            }
            if let Some(ref h) = home {
                c.push(h.join(".bash_completion.d/kam"));
            }
        }
        shells::Shell::Zsh => {
            c.push(PathBuf::from("/usr/share/zsh/site-functions/_kam"));
            c.push(PathBuf::from("/usr/local/share/zsh/site-functions/_kam"));
            if let Some(ref d) = xdg_data {
                c.push(d.join("zsh/site-functions/_kam"));
            }
            if let Some(ref h) = home {
                c.push(h.join(".zsh/completion/_kam"));
            }
        }
        shells::Shell::Fish => {
            c.push(PathBuf::from(
                "/usr/share/fish/vendor_completions.d/kam.fish",
            ));
            c.push(PathBuf::from("/usr/share/fish/completions/kam.fish"));
            if let Some(ref d) = xdg_data {
                c.push(d.join("fish/completions/kam.fish"));
            }
            if let Some(ref xcfg) = xdg_config {
                c.push(xcfg.join("fish/completions/kam.fish"));
            }
            if let Some(ref h) = home {
                c.push(h.join(".config/fish/completions/kam.fish"));
            }
        }
        shells::Shell::PowerShell => {
            // Windows 用户模块路径: $HOME/Documents/PowerShell/Modules/kam/kam.psm1
            if cfg!(windows) {
                if let Some(ref h) = home {
                    c.push(h.join("Documents/PowerShell/Modules/kam/kam.psm1"));
                }
            } else {
                // Unix 风格 user path
                if let Some(ref d) = xdg_data {
                    c.push(d.join("powershell/Modules/kam/kam.psm1"));
                }
                if let Some(ref h) = home {
                    c.push(h.join(".local/share/powershell/Modules/kam/kam.psm1"));
                }
            }
        }
        shells::Shell::Elvish => {
            c.push(PathBuf::from("/usr/share/elvish/lib/kam"));
            if let Some(ref h) = home {
                c.push(h.join(".elvish/lib/kam"));
            }
            if let Some(ref xcfg) = xdg_config {
                c.push(xcfg.join("elvish/lib/kam"));
            }
        }
        _ => {
            // fallback 到 bash 风格位置
            c.push(PathBuf::from("/usr/share/bash-completion/completions/kam"));
            if let Some(ref d) = xdg_data {
                c.push(d.join("bash-completion/completions/kam"));
            }
            if let Some(ref h) = home {
                c.push(h.join(".bash_completion.d/kam"));
            }
        }
    }

    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmds::completion::args::ShellArg;
    use serial_test::serial;
    use std::env;
    use tempfile::tempdir;
    use walkdir::WalkDir;

    #[test]
    fn can_generate_bash_completion_to_file() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("kam.bash");
        let args = CompletionArgs {
            shell: ShellArg::Bash,
            out: Some(out.to_string_lossy().to_string()),
            install: false,
        };
        assert!(run(args).is_ok());
        assert!(out.exists());
        let content = std::fs::read_to_string(out).unwrap();
        assert!(content.contains("_kam()"));
    }

    #[test]
    #[serial(env)]
    fn can_install_bash_completion_with_install_flag() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path().to_path_buf();

        // Preserve original HOME and set a fake HOME for the duration of the test.
        let orig_home = env::var("HOME").ok();
        unsafe { env::set_var("HOME", home_dir.to_str().unwrap()) };

        // Ensure dirs::home_dir reflects our change; else skip to avoid touching real home.
        if dirs::home_dir().as_deref() != Some(home_dir.as_path()) {
            if let Some(o) = orig_home {
                unsafe { env::set_var("HOME", o) };
            } else {
                unsafe { env::remove_var("HOME") };
            }
            return;
        }

        let args = CompletionArgs {
            shell: ShellArg::Bash,
            out: None,
            install: true,
        };
        assert!(run(args).is_ok());

        // Search recursively for any file under HOME that contains the bash completion
        // function `_kam()`, so we don't rely on a specific filename.
        let mut found_path: Option<std::path::PathBuf> = None;
        for entry in walkdir::WalkDir::new(&home_dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            if let Ok(text) = std::fs::read_to_string(entry.path()) {
                if text.contains("_kam()") {
                    found_path = Some(entry.into_path());
                    break;
                }
            }
        }
        assert!(
            found_path.is_some(),
            "installed bash completion containing '_kam()' not found under HOME"
        );

        // restore HOME
        if let Some(o) = orig_home {
            unsafe { env::set_var("HOME", o) };
        } else {
            unsafe { env::remove_var("HOME") };
        }
    }

    #[test]
    #[serial(env)]
    fn can_install_fish_completion_with_install_flag() {
        let tmp = tempdir().unwrap();
        let home_dir = tmp.path().to_path_buf();

        // Preserve original HOME and set a fake HOME for the duration of the test.
        let orig_home = env::var("HOME").ok();
        unsafe { env::set_var("HOME", home_dir.to_str().unwrap()) };

        // Ensure dirs::home_dir reflects our change; else skip to avoid touching real home.
        if dirs::home_dir().as_deref() != Some(home_dir.as_path()) {
            if let Some(o) = orig_home {
                unsafe { env::set_var("HOME", o) };
            } else {
                unsafe { env::remove_var("HOME") };
            }
            return;
        }

        let args = CompletionArgs {
            shell: ShellArg::Fish,
            out: None,
            install: true,
        };
        assert!(run(args).is_ok());

        // Search recursively for an installed kam.fish file under the fake HOME and accept the first match.
        let mut found: Option<std::path::PathBuf> = None;
        for entry in WalkDir::new(&home_dir).into_iter().filter_map(Result::ok) {
            if entry.file_type().is_file() && entry.file_name() == "kam.fish" {
                found = Some(entry.into_path());
                break;
            }
        }
        let path = found.expect("installed kam.fish not found under HOME");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("complete -c kam") || content.contains("kam.fish"));

        // restore HOME
        if let Some(o) = orig_home {
            unsafe { env::set_var("HOME", o) };
        } else {
            unsafe { env::remove_var("HOME") };
        }
    }

    #[test]
    #[serial(env)]
    fn can_install_and_write_out_together() {
        // Prepare a fake HOME and a separate output file path, then ensure
        // `--install` and `--out` can be used together: both files should be written.
        let tmp_home = tempdir().unwrap();
        let home_dir = tmp_home.path().to_path_buf();

        let out_dir = tempdir().unwrap();
        let out = out_dir.path().join("kam.bash");

        // Preserve original HOME and set a fake HOME for the duration of the test.
        let orig_home = env::var("HOME").ok();
        unsafe { env::set_var("HOME", home_dir.to_str().unwrap()) };

        // Ensure dirs::home_dir reflects our change; else skip to avoid touching real home.
        if dirs::home_dir().as_deref() != Some(home_dir.as_path()) {
            if let Some(o) = orig_home {
                unsafe { env::set_var("HOME", o) };
            } else {
                unsafe { env::remove_var("HOME") };
            }
            return;
        }

        let args = CompletionArgs {
            shell: ShellArg::Bash,
            out: Some(out.to_string_lossy().to_string()),
            install: true,
        };
        assert!(run(args).is_ok());

        // The explicit out file should exist and contain the completion.
        assert!(out.exists(), "expected explicit out file to exist");
        let out_content = std::fs::read_to_string(&out).unwrap();
        assert!(out_content.contains("_kam()"));

        // The installation path (under the fake HOME) should also contain the completion.
        let mut found: Option<PathBuf> = None;
        for entry in walkdir::WalkDir::new(&home_dir)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(entry.path()) {
                if text.contains("_kam()") {
                    found = Some(entry.into_path());
                    break;
                }
            }
        }
        assert!(
            found.is_some(),
            "installed `kam` completion not found under fake HOME when using --install"
        );

        // restore HOME
        if let Some(o) = orig_home {
            unsafe { env::set_var("HOME", o) };
        } else {
            unsafe { env::remove_var("HOME") };
        }
    }
}
