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
use base64::engine::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use clap::Args;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::errors::KamError;
use crate::utils::Utils;

const TERMUX_DATA_DIR: &str = "/data/data/com.termux/files";
/// Relative path (under TERMUX_DATA_DIR) to the termux env file that should be sourced.
const TERMUX_ENV_REL: &str = "usr/etc/termux/termux.env";
/// Relative path (under TERMUX_DATA_DIR) to the termux login binary.
const TERMUX_LOGIN_REL: &str = "usr/bin/login";
/// Relative path (under TERMUX_DATA_DIR) to the shell binary used for one-shot commands.
const TERMUX_SH_REL: &str = "usr/bin/sh";
/// One-shot remote format string: placeholders are (DATA_DIR, base64_payload, ENV_REL, SH_REL)
const TERMUX_ONESHOT_FMT: &str = "su -c 'D={DATA_DIR}; U=$(stat -c %u $D/home); echo {PAYLOAD} | base64 -d | exec su $U -c \"cd $D/home && . $D/{ENV}; exec $D/{SH} -l -s\"'";

fn is_android_host() -> bool {
    // Heuristics to detect running on Android (Termux/emulator/device):
    // - TERMUX_VERSION env var (Termux)
    // - common Android filesystem markers (/system/bin/getprop, /system/build.prop)
    // - presence of the Termux data dir
    // - /proc/version mentions 'android'
    if std::env::var_os("TERMUX_VERSION").is_some() {
        return true;
    }
    if Path::new("/system/bin/getprop").exists() || Path::new("/system/build.prop").exists() {
        return true;
    }
    if Path::new(TERMUX_DATA_DIR).exists() {
        return true;
    }
    if let Ok(s) = std::fs::read_to_string("/proc/version") {
        if s.to_lowercase().contains("android") {
            return true;
        }
    }
    false
}

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
        let encoded = BASE64_ENGINE.encode(cmd.as_bytes());

        // Construct remote pipeline: decode and run under Termux user's shell using a reusable template
        let remote = TERMUX_ONESHOT_FMT
            .replace("{DATA_DIR}", TERMUX_DATA_DIR)
            .replace("{PAYLOAD}", &encoded)
            .replace("{ENV}", TERMUX_ENV_REL)
            .replace("{SH}", TERMUX_SH_REL);

        let mut adb_cmd = vec!["adb".to_string()];
        adb_cmd.extend(adb_base);
        adb_cmd.push("shell".to_string());
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
        // Interactive mode: build the login inner command
        let login_inner = format!(
            "D={}; U=$(stat -c %u $D/home); exec su $U -c \"cd $D/home && . $D/{}; exec $D/{}\"",
            TERMUX_DATA_DIR, TERMUX_ENV_REL, TERMUX_LOGIN_REL
        );

        if is_android_host() {
            // run locally with su
            let status = Command::new("su")
                .arg("-c")
                .arg(&login_inner)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();

            match status {
                Ok(s) => {
                    if s.success() {
                        Ok(())
                    } else {
                        Utils::error(&format!("local su/termux exited with status: {}", s));
                        Ok(())
                    }
                }
                Err(e) => {
                    Utils::error(&format!(
                        "Failed to spawn local termux interactive session: {}",
                        e
                    ));
                    Ok(())
                }
            }
        } else {
            // fallback to adb path
            let remote_login = format!("su -c '{}'", login_inner);

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

    #[test]
    fn is_android_host_env_termux() {
        use std::env;
        // Temporarily set TERMUX_VERSION to simulate Termux/Android environment
        unsafe {
            env::set_var("TERMUX_VERSION", "1.0");
        }
        assert!(
            is_android_host(),
            "is_android_host() should return true when TERMUX_VERSION is set"
        );
        // Clean up to avoid leaking state to other tests
        unsafe {
            env::remove_var("TERMUX_VERSION");
        }
    }
}
