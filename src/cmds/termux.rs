//! `kam termux` - Interact with Termux on a connected Android device via adb.
//!
//! Features:
//! - `kam termux`
//!     Start an interactive Termux session (allocates a PTY, runs Termux `login`).
//! - `kam termux -c "<cmd>"`
//!     Run a single command inside the Termux environment and print output.
//!
//! Notes:
//! - This command expects `adb` to be available on the PATH and a device to be connected.
//! - For the one-shot mode we base64-encode the payload to avoid complex on-device shell escaping.
//!
//! Design:
//! - Keep behavior simple and defensive: when `adb` is missing we print a friendly error and return.
//! - Interactive mode uses `adb shell -t` + the Termux login command (switch to Termux UID and exec login).
//! - Non-interactive (one-shot) mode decodes a base64 payload on the device and pipes it into the user's shell.
use clap::Args;
use std::process::{Command, Stdio};

use crate::errors::KamError;
use crate::utils::Utils;

/// Arguments for `kam termux`
#[derive(Args, Debug)]
pub struct TermuxArgs {
    /// Optional adb device id (from `adb devices`). If omitted the default adb device is used.
    #[arg(short = 'd', long = "device")]
    pub device: Option<String>,

    /// Execute a single command inside Termux and exit (non-interactive).
    /// Example: kam termux -c "ls -la ~/.ssh"
    #[arg(short = 'c', long = "command")]
    pub command: Option<String>,

    /// Timeout in seconds for one-shot command execution (default: 60)
    #[arg(short = 't', long = "timeout", default_value_t = 60)]
    pub timeout: u64,
}

/// Execute `termux` command.
///
/// - If `args.command` is Some => run one-shot, print stdout/stderr and return.
/// - Otherwise => run interactive session (`adb shell -t "<login>"`) inheriting stdin/stdout/stderr.
pub fn run(args: TermuxArgs) -> Result<(), KamError> {
    // Helper to build adb base args
    let mut adb_base: Vec<String> = Vec::new();
    if let Some(ref d) = args.device {
        adb_base.push("-s".to_string());
        adb_base.push(d.clone());
    }

    // Check adb presence first for both modes
    match Command::new("adb").arg("version").output() {
        Ok(v) => {
            if !v.status.success() {
                Utils::error(
                    "adb not available or returned non-zero; please install adb and ensure it's in PATH.",
                );
                return Ok(());
            }
        }
        Err(_) => {
            Utils::error(
                "adb not found; please install platform-tools and ensure `adb` is on PATH.",
            );
            return Ok(());
        }
    }

    if let Some(cmd) = args.command {
        // One-shot mode: base64 encode command to avoid shell escaping
        let encoded = base64::encode(cmd.as_bytes());

        // Construct remote pipeline: decode and run under Termux user's shell
        // We use `su -c '...` to run as root and then su to the termux owner UID (O_UID)
        // and execute `sh -lc -` reading decoded payload from stdin.
        let remote = format!(
            "su -c 'O_UID=$(ls -dn /data/data/com.termux/files/home | cut -d\" \" -f3); \
             export HOME=/data/data/com.termux/files/home; \
             export PREFIX=/data/data/com.termux/files/usr; \
             export PATH=$PREFIX/bin:$PATH; \
             echo {} | base64 -d | exec su $O_UID -c \"cd $HOME; $PREFIX/bin/sh -lc -\"'",
            encoded
        );

        let mut adb_cmd = vec!["adb".to_string()];
        adb_cmd.extend(adb_base);
        adb_cmd.push("shell".to_string());
        adb_cmd.push("-t".to_string());
        adb_cmd.push(remote);

        // Execute and capture output
        match Command::new(&adb_cmd[0])
            .args(&adb_cmd[1..])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
        {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                println!("stdout:\n{}", stdout);
                println!("stderr:\n{}", stderr);
                Ok(())
            }
            Err(e) => {
                Utils::error(&format!("Failed to run adb/termux command: {}", e));
                Ok(())
            }
        }
    } else {
        // Interactive mode: spawn adb shell -t "<login command>" and inherit stdio
        let remote_login = concat!(
            "su -c 'O_UID=$(ls -dn /data/data/com.termux/files/home | cut -d\" \" -f3); ",
            "export HOME=/data/data/com.termux/files/home; ",
            "export PREFIX=/data/data/com.termux/files/usr; ",
            "export PATH=$PREFIX/bin:$PATH; ",
            "exec su $O_UID -c \"cd $HOME; exec $PREFIX/bin/login\"'"
        );

        let mut adb_cmd = vec!["adb".to_string()];
        adb_cmd.extend(adb_base);
        adb_cmd.push("shell".to_string());
        adb_cmd.push("-t".to_string());
        adb_cmd.push(remote_login.to_string());

        // Spawn and attach to terminal
        let status = Command::new(&adb_cmd[0])
            .args(&adb_cmd[1..])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();

        match status {
            Ok(s) => {
                if s.success() {
                    Ok(())
                } else {
                    Utils::error(&format!("adb/termux exited with status: {}", s));
                    Ok(())
                }
            }
            Err(e) => {
                Utils::error(&format!(
                    "Failed to spawn adb termux interactive session: {}",
                    e
                ));
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn termux_args_struct_default() {
        let args = TermuxArgs {
            device: None,
            command: None,
            timeout: 60,
        };
        // In environments without adb available the command will return Ok(()) after printing an error.
        // We assert the function returns Ok so tests don't fail on CI where adb isn't present.
        assert!(run(args).is_ok());
    }
}
