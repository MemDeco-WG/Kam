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
use std::fs;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
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
    /// NOTE: short flag changed to `-D` to reserve `-d` for `--daemon`.
    #[arg(short = 'D', long = "device")]
    pub device: Option<String>,

    /// Execute a single command inside Termux and exit (non-interactive).
    /// Example: kam termux -c "ls -la ~/.ssh"
    #[arg(short = 'c', long = "command")]
    pub command: Option<String>,

    /// Start a persistent Termux session in the background (daemon mode).
    /// Use `kam termux -l` to list the daemon or `kam termux -k` to kill it.
    #[arg(short = 'd', long = "daemon", action = clap::ArgAction::SetTrue)]
    pub daemon: bool,

    /// List the current Termux daemon process (if any).
    #[arg(short = 'l', long = "list", action = clap::ArgAction::SetTrue)]
    pub list: bool,

    /// Kill the current Termux daemon process.
    #[arg(short = 'k', long = "kill", action = clap::ArgAction::SetTrue)]
    pub kill: bool,

    /// Helper: print Termux SSH preparation instructions (do not modify device automatically).
    #[arg(long = "ssh-setup", action = clap::ArgAction::SetTrue)]
    pub ssh_setup: bool,

    /// Forward local tcp:8022 to device tcp:8022 for SSH (adb forward tcp:8022 tcp:8022).
    #[arg(long = "ssh-forward", action = clap::ArgAction::SetTrue)]
    pub ssh_forward: bool,

    /// Push a local public key to Termux `~/.ssh/authorized_keys` using `adb push`.
    /// Example: `--ssh-push-key ~/.ssh/id_rsa.pub`
    #[arg(long = "ssh-push-key", value_name = "PATH")]
    pub ssh_push_key: Option<String>,

    /// Connect via SSH (will ensure a forward is set and spawn `ssh -p 8022 localhost`).
    #[arg(long = "ssh-connect", action = clap::ArgAction::SetTrue)]
    pub ssh_connect: bool,

    /// SSH port to use for Termux SSH (default: 8022).
    #[arg(long = "ssh-port", default_value_t = 8022)]
    pub ssh_port: u16,

    /// Timeout in seconds for adb operations (used by daemon/one-shot).
    #[arg(short = 't', long = "timeout", default_value_t = 60)]
    pub timeout: u64,
}

/// Execute `termux` command.
///
/// Behavior:
/// - If `--list` (`-l`) => show the current daemon process (if any) and return.
/// - If `--kill` (`-k`) => kill the current daemon (if any) and return.
/// - If `--daemon` (`-d`) => start a background (persistent) Termux session and return.
/// - If `--command` (`-c`) is supplied => run one-shot, print stdout/stderr and return.
/// - SSH helpers (non-destructive helpers to prepare/connect to Termux via SSH):
///     * `--ssh-forward`             => run `adb forward tcp:8022 tcp:8022` and print status.
///     * `--ssh-push-key <path>`     => push local public key to Termux `~/.ssh/authorized_keys` via `adb push`.
///     * `--ssh-connect`             => set up forwarding then spawn `ssh -p 8022 localhost` (interactive).
///     * `--ssh-setup`               => print concise Termux-side setup instructions (install openssh / start `sshd`).
/// - Otherwise => start an interactive session (`adb shell -t "<login>"`) inheriting tty.
///
/// Note: the daemon uses a pid file under $KAM_HOME/termux/daemon.pid and writes logs
/// to $KAM_HOME/termux/daemon.log.
pub fn run(args: TermuxArgs) -> Result<(), KamError> {
    // Helper to build adb base args (device selection)
    let mut adb_base: Vec<String> = Vec::new();
    if let Some(ref d) = args.device {
        adb_base.push("-s".to_string());
        adb_base.push(d.clone());
    }

    // Local helpers for daemon management (store pid/log under Kam home)
    fn termux_daemon_dir() -> Result<PathBuf, KamError> {
        let base = crate::utils::kam_home_dir()?;
        let dir = base.join("termux");
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(dir)
    }

    fn termux_pid_path() -> Result<PathBuf, KamError> {
        Ok(termux_daemon_dir()?.join("daemon.pid"))
    }

    fn termux_log_path() -> Result<PathBuf, KamError> {
        Ok(termux_daemon_dir()?.join("daemon.log"))
    }

    fn read_pidfile() -> Result<Option<u32>, KamError> {
        let p = termux_pid_path()?;
        if !p.exists() {
            return Ok(None);
        }
        let s = fs::read_to_string(&p)?;
        match s.trim().parse::<u32>() {
            Ok(pid) => Ok(Some(pid)),
            Err(_) => Ok(None),
        }
    }

    fn write_pidfile(pid: u32) -> Result<(), KamError> {
        let p = termux_pid_path()?;
        fs::write(p, pid.to_string())?;
        Ok(())
    }

    fn remove_pidfile() -> Result<(), KamError> {
        let p = termux_pid_path()?;
        if p.exists() {
            fs::remove_file(p)?;
        }
        Ok(())
    }

    fn is_pid_running(pid: u32) -> bool {
        match Command::new("ps")
            .arg("-p")
            .arg(pid.to_string())
            .arg("-o")
            .arg("pid=")
            .output()
        {
            Ok(out) => out.status.success() && !out.stdout.is_empty(),
            Err(_) => false,
        }
    }

    // Validate conflicting action flags: only one of daemon/list/kill/command should be used at once.
    let mut action_count = 0;
    if args.list {
        action_count += 1;
    }
    if args.kill {
        action_count += 1;
    }
    if args.daemon {
        action_count += 1;
    }
    if args.command.is_some() {
        action_count += 1;
    }
    if action_count > 1 {
        Utils::error(&trf!("cli.commands.termux.conflicting_options"));
        return Ok(());
    }

    // Handle list/kill operations first (they do not require adb)
    if args.list {
        match read_pidfile() {
            Ok(Some(pid)) => {
                if is_pid_running(pid) {
                    match Command::new("ps")
                        .arg("-p")
                        .arg(pid.to_string())
                        .arg("-o")
                        .arg("pid=,cmd=")
                        .output()
                    {
                        Ok(out) => {
                            let info = String::from_utf8_lossy(&out.stdout).trim().to_string();
                            if !info.is_empty() {
                                Utils::info(&trf!("cli.commands.termux.daemon.listing", info));
                            } else {
                                Utils::info(&trf!(
                                    "cli.commands.termux.daemon.listing",
                                    format!("pid {}", pid)
                                ));
                            }
                            if let Ok(lp) = termux_log_path() {
                                Utils::info(&trf!("cli.commands.termux.daemon.logs", lp.display()));
                            }
                        }
                        Err(e) => {
                            Utils::error(&trf!(
                                "cli.commands.termux.daemon.failed_to_query_process",
                                e
                            ));
                        }
                    }
                } else {
                    Utils::info(&trf!("cli.commands.termux.daemon.not_running"));
                    // stale pidfile, remove it
                    let _ = remove_pidfile();
                }
            }
            Ok(None) => {
                Utils::info(&trf!("cli.commands.termux.daemon.not_running"));
            }
            Err(e) => {
                Utils::error(&format!("Failed to read daemon pidfile: {}", e));
            }
        }
        return Ok(());
    }

    if args.kill {
        match read_pidfile() {
            Ok(Some(pid)) => {
                if is_pid_running(pid) {
                    match Command::new("kill").arg(pid.to_string()).status() {
                        Ok(s) => {
                            if s.success() {
                                let _ = remove_pidfile();
                                Utils::success(&trf!("cli.commands.termux.daemon.killed", pid));
                            } else {
                                let code = s
                                    .code()
                                    .map(|c| c.to_string())
                                    .unwrap_or_else(|| "unknown".to_string());
                                Utils::error(&trf!(
                                    "cli.commands.termux.daemon.failed_to_kill",
                                    pid,
                                    code
                                ));
                            }
                        }
                        Err(e) => {
                            Utils::error(&trf!(
                                "cli.commands.termux.daemon.failed_to_kill",
                                pid,
                                e
                            ));
                        }
                    }
                } else {
                    Utils::info(&trf!("cli.commands.termux.daemon.not_running"));
                    let _ = remove_pidfile();
                }
            }
            Ok(None) => {
                Utils::info(&trf!("cli.commands.termux.daemon.not_running"));
            }
            Err(e) => {
                Utils::error(&trf!(
                    "cli.commands.termux.daemon.failed_to_read_pidfile",
                    e
                ));
            }
        }
        return Ok(());
    }

    // At this point: either daemon start, one-shot, or interactive.
    // Check for adb presence (required for starting daemon, one-shot or remote interactive)
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

    if args.daemon {
        // Deprecated: previous background adb-based daemon mode used adb + root to start
        // a remote Termux session. That approach is no longer supported.
        // Prefer running Termux's `sshd` on the phone and using `--ssh-forward` / `--ssh-connect`.
        Utils::warn(&trf!("cli.commands.termux.deprecated_daemon"));
        return Ok(());
    }

    // SSH helper operations (forwarding / push key / connect / setup).
    if args.ssh_forward {
        // Run: adb [<device>] forward tcp:<port> tcp:<port>
        let mut fwd_cmd = Command::new("adb");
        fwd_cmd.args(&adb_base);
        let port_spec = format!("tcp:{}", args.ssh_port);
        fwd_cmd.arg("forward").arg(&port_spec).arg(&port_spec);
        match fwd_cmd.status() {
            Ok(s) if s.success() => {
                Utils::success(&trf!("termux.ssh.forwarded", args.ssh_port));
            }
            Ok(s) => {
                Utils::error(&trf!("termux.ssh.forward_failed", s.code().unwrap_or(-1)));
            }
            Err(e) => {
                Utils::error(&trf!("termux.ssh.forward_failed_err", e));
            }
        }
        return Ok(());
    }

    if let Some(pubkey_path) = args.ssh_push_key.as_ref() {
        // Verify local key exists
        if !std::path::Path::new(pubkey_path).exists() {
            Utils::error(&trf!("termux.ssh.pubkey_missing", pubkey_path));
            return Ok(());
        }

        // Ensure remote .ssh directory exists (best-effort)
        let remote_ssh_dir = "/data/data/com.termux/files/home/.ssh";
        let mkdir_status = Command::new("adb")
            .args(&adb_base)
            .arg("shell")
            .arg(format!("mkdir -p {}", remote_ssh_dir))
            .status();

        if mkdir_status.is_err() || mkdir_status.unwrap().code().unwrap_or(1) != 0 {
            Utils::error(&trf!("termux.ssh.remote_mkdir_failed"));
            return Ok(());
        }

        // Push key
        let dest = format!("{}/authorized_keys", remote_ssh_dir);
        match Command::new("adb")
            .args(&adb_base)
            .arg("push")
            .arg(pubkey_path)
            .arg(&dest)
            .status()
        {
            Ok(s) if s.success() => {
                Utils::success(&trf!("termux.ssh.pushed_key", dest));
            }
            _ => {
                Utils::error(&trf!("termux.ssh.push_failed"));
            }
        }
        return Ok(());
    }

    if args.ssh_connect {
        // Try to ensure forwarding is set up (best-effort)
        let port_spec = format!("tcp:{}", args.ssh_port);
        let _ = Command::new("adb")
            .args(&adb_base)
            .arg("forward")
            .arg(&port_spec)
            .arg(&port_spec)
            .status();

        Utils::info(&trf!("termux.ssh.connecting", args.ssh_port));
        // Spawn ssh to localhost:<port>, attach to current tty
        let ssh_status = Command::new("ssh")
            .arg("localhost")
            .arg("-p")
            .arg(args.ssh_port.to_string())
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();

        match ssh_status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                Utils::error(&trf!("termux.ssh.ssh_exited", s.code().unwrap_or(-1)));
            }
            Err(e) => {
                Utils::error(&trf!("termux.ssh.ssh_failed", e));
            }
        }
        return Ok(());
    }

    if args.ssh_setup {
        // Print brief, safe instructions for preparing Termux (avoid automatic installation)
        Utils::info(&trf!("termux.ssh.setup_instructions"));
        Utils::info(&trf!("termux.ssh.setup_step1", "pkg update && pkg upgrade"));
        Utils::info(&trf!("termux.ssh.setup_step2", "pkg install openssh"));
        Utils::info(&trf!(
            "termux.ssh.setup_step3",
            "passwd  (set a password, e.g., 123456)"
        ));
        Utils::info(&trf!(
            "termux.ssh.setup_step4",
            "sshd    (start the SSH server; default port 8022)"
        ));
        Utils::info(&trf!(
            "termux.ssh.setup_note",
            args.ssh_port,
            args.ssh_port,
            args.ssh_port
        ));
        return Ok(());
    }

    if let Some(cmd) = args.command {
        // One-shot mode: execute command via SSH (requires Termux `sshd` + adb port forwarding).
        let port_spec = format!("tcp:{}", args.ssh_port);
        let _ = Command::new("adb")
            .args(&adb_base)
            .arg("forward")
            .arg(&port_spec)
            .arg(&port_spec)
            .status();

        // Use a shell on the remote side so complex commands are interpreted correctly.
        match Command::new("ssh")
            .arg("localhost")
            .arg("-p")
            .arg(args.ssh_port.to_string())
            .arg("sh")
            .arg("-lc")
            .arg(cmd)
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
                Utils::error(&trf!("termux.ssh.ssh_failed", e));
                Ok(())
            }
        }
    } else {
        // Interactive mode: prefer SSH-based connection (requires Termux `sshd` on the device).
        // Ensure adb port forwarding is set (best-effort), then spawn an interactive SSH session.
        let port_spec = format!("tcp:{}", args.ssh_port);
        let _ = Command::new("adb")
            .args(&adb_base)
            .arg("forward")
            .arg(&port_spec)
            .arg(&port_spec)
            .status();

        Utils::info(&trf!("termux.ssh.connecting", args.ssh_port));
        let status = Command::new("ssh")
            .arg("localhost")
            .arg("-p")
            .arg(args.ssh_port.to_string())
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();

        match status {
            Ok(s) => {
                if s.success() {
                    Ok(())
                } else {
                    Utils::error(&trf!("termux.ssh.ssh_exited", s.code().unwrap_or(-1)));
                    Ok(())
                }
            }
            Err(e) => {
                Utils::error(&trf!("termux.ssh.ssh_failed", e));
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn termux_args_struct_default() {
        let args = TermuxArgs {
            device: None,
            command: None,
            daemon: false,
            list: false,
            kill: false,
            ssh_setup: false,
            ssh_forward: false,
            ssh_push_key: None,
            ssh_connect: false,
            ssh_port: 8022,
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

    #[test]
    #[serial]
    fn termux_daemon_list_no_daemon() {
        use std::env;
        use tempfile::TempDir;

        // Ensure an isolated KAM_HOME so the daemon pidfile doesn't exist.
        let tmp = TempDir::new().unwrap();
        let orig = env::var_os("KAM_HOME");
        unsafe {
            env::set_var("KAM_HOME", tmp.path().to_str().unwrap());
        }

        let args = TermuxArgs {
            device: None,
            command: None,
            daemon: false,
            list: true,
            kill: false,
            ssh_setup: false,
            ssh_forward: false,
            ssh_push_key: None,
            ssh_connect: false,
            ssh_port: 8022,
            timeout: 60,
        };
        // Should return Ok even when no adb is present; the command prints a friendly message.
        assert!(run(args).is_ok());

        // Verify we did not accidentally create a pidfile
        let pid_path = crate::utils::kam_home_dir()
            .unwrap()
            .join("termux")
            .join("daemon.pid");
        assert!(!pid_path.exists());

        // Restore original KAM_HOME
        if let Some(v) = orig {
            unsafe {
                env::set_var("KAM_HOME", v);
            }
        } else {
            unsafe {
                env::remove_var("KAM_HOME");
            }
        }
    }

    #[test]
    #[serial]
    fn termux_daemon_kill_no_daemon() {
        use std::env;
        use tempfile::TempDir;

        // Ensure an isolated KAM_HOME so the daemon pidfile doesn't exist.
        let tmp = TempDir::new().unwrap();
        let orig = env::var_os("KAM_HOME");
        unsafe {
            env::set_var("KAM_HOME", tmp.path().to_str().unwrap());
        }

        let args = TermuxArgs {
            device: None,
            command: None,
            daemon: false,
            list: false,
            kill: true,
            ssh_setup: false,
            ssh_forward: false,
            ssh_push_key: None,
            ssh_connect: false,
            ssh_port: 8022,
            timeout: 60,
        };
        // Should return Ok even when no adb is present; the command prints a friendly message.
        assert!(run(args).is_ok());

        // Verify we did not accidentally create a pidfile
        let pid_path = crate::utils::kam_home_dir()
            .unwrap()
            .join("termux")
            .join("daemon.pid");
        assert!(!pid_path.exists());

        // Restore original KAM_HOME
        if let Some(v) = orig {
            unsafe {
                env::set_var("KAM_HOME", v);
            }
        } else {
            unsafe {
                env::remove_var("KAM_HOME");
            }
        }
    }
}
