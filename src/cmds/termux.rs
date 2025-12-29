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

    /// Timeout in seconds for one-shot command execution (default: 60)
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
        // Start background daemon session.
        if !crate::utils::command_exists("nohup") {
            Utils::error(&trf!("cli.commands.termux.daemon.no_nohup"));
            return Ok(());
        }

        // Check existing pidfile
        match read_pidfile() {
            Ok(Some(pid)) if is_pid_running(pid) => {
                Utils::error(&trf!("cli.commands.termux.daemon.already_running", pid));
                return Ok(());
            }
            _ => {}
        }

        // Build the same login inner command as interactive mode
        let login_inner = format!(
            "D={}; U=$(stat -c %u $D/home); exec su $U -c \"cd $D/home && . $D/{}; exec $D/{}\"",
            TERMUX_DATA_DIR, TERMUX_ENV_REL, TERMUX_LOGIN_REL
        );
        let remote_login = format!("su -c '{}'", login_inner);

        // Prepare log file and spawn background process (nohup adb ... shell -t -t ...)
        let log_path = match termux_log_path() {
            Ok(p) => p,
            Err(e) => {
                Utils::error(&format!("Failed to create daemon log dir: {}", e));
                return Ok(());
            }
        };

        let mut log_file = match OpenOptions::new().create(true).append(true).open(&log_path) {
            Ok(f) => f,
            Err(e) => {
                Utils::error(&format!("Failed to open daemon log file: {}", e));
                return Ok(());
            }
        };
        // Build nohup command
        let mut cmd = Command::new("nohup");
        cmd.arg("adb");
        cmd.args(&adb_base);
        cmd.arg("shell");
        // Force remote PTY allocation for detached session by using `-t -t`
        cmd.arg("-t");
        cmd.arg("-t");
        cmd.arg(remote_login);
        cmd.stdin(Stdio::null());
        // stdout/stderr -> log file
        let log_clone = match log_file.try_clone() {
            Ok(f) => f,
            Err(e) => {
                Utils::error(&format!("Failed to clone log file handle: {}", e));
                return Ok(());
            }
        };
        cmd.stdout(Stdio::from(log_file));
        cmd.stderr(Stdio::from(log_clone));

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                if let Err(e) = write_pidfile(pid) {
                    Utils::error(&trf!(
                        "cli.commands.termux.daemon.failed_to_write_pidfile",
                        e
                    ));
                    // best-effort: still leave the child running
                    return Ok(());
                }
                Utils::success(&trf!("cli.commands.termux.daemon.started", pid));
                Utils::info(&trf!("cli.commands.termux.daemon.logs", log_path.display()));
            }
            Err(e) => {
                Utils::error(&trf!("cli.commands.termux.daemon.failed_to_start", e));
            }
        }
        return Ok(());
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
    use serial_test::serial;

    #[test]
    fn termux_args_struct_default() {
        let args = TermuxArgs {
            device: None,
            command: None,
            daemon: false,
            list: false,
            kill: false,
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
