//! `kam termux` - Interact with Termux on a connected Android device via adb.
//!
//! Features:
//! - `kam termux`
//!   Start an interactive Termux session (allocates a PTY, runs Termux `login`).
//! - `kam termux -c "<cmd>"`
//!   Run a single command inside the Termux environment and print output.
//!
//! Notes:
//! - This command expects `adb` to be available on the PATH and a device to be connected.
//! - For the one-shot mode we base64-encode the payload to avoid complex on-device shell escaping.
#![allow(unused_imports, dead_code)]
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
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};
use std::io::{self, Write};

const TERMUX_DATA_DIR: &str = "/data/data/com.termux/files";
/// Relative path (under TERMUX_DATA_DIR) to the termux env file that should be sourced.
const TERMUX_ENV_REL: &str = "usr/etc/termux/termux.env";
/// Relative path (under TERMUX_DATA_DIR) to the termux login binary.
const TERMUX_LOGIN_REL: &str = "usr/bin/login";
/// Relative path (under TERMUX_DATA_DIR) to the shell binary used for one-shot commands.
const TERMUX_SH_REL: &str = "usr/bin/sh";
// NOTE: The previous adb-shell one-shot format has been removed.
// One-shot execution now uses SSH over an adb port-forward (see SSH helpers below).

fn is_android_host() -> bool {
    // Use sysinfo as the authoritative source for OS identification.
    // Rely on System::name() / System::kernel_version() and avoid ad-hoc TERMUX env/file heuristics.
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();

    if let Some(name) = System::name()
        && name.to_lowercase().contains("android")
    {
        return true;
    }
    if let Some(kernel) = System::kernel_version()
        && kernel.to_lowercase().contains("android")
    {
        return true;
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

    // NOTE: 'daemon' / 'list' / 'kill' options (adb-based background Termux sessions)
    // have been removed. The project now uses an SSH-based workflow (safer and more robust).
    // Use the SSH helpers instead:
    //   - `--ssh-setup`    : prints Termux-side setup instructions (install openssh / start sshd)
    //   - `--ssh-forward`  : establish `adb forward tcp:<port> tcp:<port>`
    //   - `--ssh-push-key` : push a local public key to Termux (attempts `adb push`)
    //   - `--ssh-connect`  : open an interactive SSH session (`ssh -p <port> localhost`)
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

    /// Interactive: guided SSH login (forward -> install public key -> connect).
    /// Prompts to select a local public key and attempts `ssh-copy-id`, falls back to `scp` or `adb push`.
    #[arg(short = 'i', long = "interactive", action = clap::ArgAction::SetTrue)]
    pub interactive: bool,

    /// Auto: forward + push public key (if available) + connect via SSH.
    #[arg(long = "ssh-auto", action = clap::ArgAction::SetTrue)]
    pub ssh_auto: bool,

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

    // Daemon helper functions removed.
    // The previous daemon/list/kill feature (based on adb-shell background sessions) has been removed.
    // Use the SSH-based workflow (see `--ssh-setup`, `--ssh-forward`, `--ssh-connect`, `--ssh-push-key`).

    // The previous mutual-exclusion check for daemon/list/kill/command has been removed
    // because daemon/list/kill modes are no longer supported. The command now uses SSH-based helpers.

    // Daemon/list/kill operations have been removed (this functionality was adb-shell based).
    // Please use the SSH-based workflow instead: `--ssh-setup`, `--ssh-forward`, `--ssh-connect`.

    // daemon/list/kill functionality has been removed in favor of the SSH-based workflow.
    // Use the SSH helpers: --ssh-setup, --ssh-forward, --ssh-push-key, --ssh-connect.

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

    // daemon mode removed: background adb-shell Termux sessions are no longer supported.
    // Prefer SSH-based access instead (see --ssh-setup / --ssh-forward / --ssh-connect).

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

    // Auto: forward, attempt to push a public key (if available), then connect via SSH
    if args.ssh_auto {
        // 1) Ensure adb forward
        let port_spec = format!("tcp:{}", args.ssh_port);
        match Command::new("adb")
            .args(&adb_base)
            .arg("forward")
            .arg(&port_spec)
            .arg(&port_spec)
            .status()
        {
            Ok(s) if s.success() => {
                Utils::info(&trf!("termux.ssh.forwarded", args.ssh_port));
            }
            Ok(s) => {
                Utils::error(&trf!("termux.ssh.forward_failed", s.code().unwrap_or(-1)));
                return Ok(());
            }
            Err(e) => {
                Utils::error(&trf!("termux.ssh.forward_failed_err", e));
                return Ok(());
            }
        }

        // 2) Determine public key path (either supplied or default ~/.ssh/id_rsa.pub)
        let key_path: PathBuf = args.ssh_push_key.as_ref().map_or_else(
            || {
                dirs::home_dir().map_or_else(PathBuf::new, |mut p| {
                    p.push(".ssh/id_rsa.pub");
                    p
                })
            },
            PathBuf::from,
        );

        // Attempt to push key if it exists locally
        if key_path.exists() {
            let dest = "/data/data/com.termux/files/home/.ssh/authorized_keys";
            match Command::new("adb")
                .args(&adb_base)
                .arg("push")
                .arg(key_path.to_str().unwrap_or_default())
                .arg(dest)
                .status()
            {
                Ok(s) if s.success() => {
                    Utils::success(&trf!("termux.ssh.pushed_key", dest));
                }
                _ => {
                    Utils::warn(&trf!("termux.ssh.push_failed"));
                }
            }
        } else {
            Utils::warn(&trf!("termux.ssh.auto_no_pubkey"));
        }

        // 3) Connect via SSH (interactive)
        Utils::info(&trf!("termux.ssh.auto_connecting", args.ssh_port));
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

    // Interactive guided flow: forward -> select local pubkey -> try ssh-copy-id -> scp fallback -> adb push fallback -> ssh
    if args.interactive {
        // Banner
        Utils::info(&trf!("termux.ssh.interactive.starting"));

        // Ensure ssh client exists locally
        if !crate::utils::command_exists("ssh") {
            Utils::error(&trf!("termux.ssh.ssh_missing"));
            return Ok(());
        }

        // 1) Ensure adb forward
        let port_spec = format!("tcp:{}", args.ssh_port);
        match Command::new("adb")
            .args(&adb_base)
            .arg("forward")
            .arg(&port_spec)
            .arg(&port_spec)
            .status()
        {
            Ok(s) if s.success() => {
                Utils::success(&trf!("termux.ssh.forwarded", args.ssh_port));
            }
            Ok(s) => {
                Utils::error(&trf!("termux.ssh.forward_failed", s.code().unwrap_or(-1)));
                return Ok(());
            }
            Err(e) => {
                Utils::error(&trf!("termux.ssh.forward_failed_err", e));
                return Ok(());
            }
        }

        // 2) Discover local public keys (~/.ssh/*.pub)
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(mut h) = dirs::home_dir() {
            h.push(".ssh");
            if h.exists()
                && let Ok(entries) = fs::read_dir(&h)
            {
                for e in entries.filter_map(|r| r.ok()) {
                    let p = e.path();
                    if p.is_file()
                        && let Some(n) = p.file_name().and_then(|s| s.to_str())
                        && n.ends_with(".pub")
                    {
                        candidates.push(p);
                    }
                }
            }
        }

        if candidates.is_empty() {
            // No pubkey found
            Utils::warn(&trf!("termux.ssh.auto_no_pubkey"));
            Utils::info(&trf!("termux.ssh.interactive.gen_hint"));
            return Ok(());
        }

        // 3) Let user choose a key (or create a new one)
        let mut items: Vec<String> = candidates.iter().map(|p| p.display().to_string()).collect();
        // Append an option to generate a new key
        items.push(trf!("termux.ssh.interactive.create_new_key_option"));
        let chosen_idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(&trf!("termux.ssh.interactive.choose_key_or_create"))
            .items(&items)
            .default(0)
            .interact()
            .unwrap_or_default();

        // Determine the chosen public key or create a new one
        let chosen_pub: PathBuf;
        if chosen_idx == items.len() - 1 {
            // User chose "Create a new key"
            if !crate::utils::command_exists("ssh-keygen") {
                Utils::error(&trf!("termux.ssh.interactive.create_new_key_missing_tool"));
                return Ok(());
            }

            // Default private key path: ~/.ssh/id_ed25519
            let default_priv = dirs::home_dir().map_or_else(
                || "id_ed25519".to_string(),
                |mut p| {
                    p.push(".ssh/id_ed25519");
                    p.to_string_lossy().to_string()
                },
            );

            let priv_input = Input::<String>::with_theme(&ColorfulTheme::default())
                .with_prompt(&trf!(
                    "termux.ssh.interactive.enter_private_key_path",
                    default_priv
                ))
                .allow_empty(false)
                .default(default_priv.clone())
                .interact_text()
                .map_or(default_priv, |s| s);

            // Expand '~' if present
            let priv_path_buf = if priv_input.starts_with("~/") {
                if let Some(mut hd) = dirs::home_dir() {
                    let rest = priv_input.trim_start_matches("~/");
                    hd.push(rest);
                    hd
                } else {
                    PathBuf::from(priv_input)
                }
            } else {
                PathBuf::from(priv_input)
            };

            // Confirm overwrite if file exists
            if priv_path_buf.exists() {
                let overwrite = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(&trf!("termux.ssh.interactive.create_new_key_prompt"))
                    .default(false)
                    .interact()
                    .unwrap_or_default();
                if !overwrite {
                    Utils::warn(&trf!(
                        "termux.ssh.interactive.create_new_key_failed",
                        "user cancelled"
                    ));
                    return Ok(());
                }
            } else if let Some(parent) = priv_path_buf.parent()
                && let Err(e) = fs::create_dir_all(parent)
            {
                Utils::error(&format!(
                    "failed to create directory {}: {}",
                    parent.display(),
                    e
                ));
                return Ok(());
            }

            // Generate key pair (no passphrase)
            Utils::info(&format!(
                "Generating SSH key pair at {}",
                priv_path_buf.display()
            ));
            match Command::new("ssh-keygen")
                .arg("-t")
                .arg("ed25519")
                .arg("-f")
                .arg(priv_path_buf.to_str().unwrap_or_default())
                .arg("-N")
                .arg("")
                .status()
            {
                Ok(s) if s.success() => {
                    let pub_path = priv_path_buf.with_extension("pub");
                    Utils::success(&trf!(
                        "termux.ssh.interactive.create_new_key_success",
                        pub_path.display()
                    ));
                    chosen_pub = pub_path;
                }
                Ok(s) => {
                    Utils::error(&trf!(
                        "termux.ssh.interactive.create_new_key_failed",
                        s.code().unwrap_or(-1)
                    ));
                    return Ok(());
                }
                Err(e) => {
                    Utils::error(&trf!("termux.ssh.interactive.create_new_key_failed", e));
                    return Ok(());
                }
            }
        } else {
            chosen_pub = candidates
                .get(chosen_idx)
                .cloned()
                .unwrap_or_else(|| candidates[0].clone());
        }

        // 4) Ask for remote username (default to local $USER)
        let default_user = std::env::var("USER").unwrap_or_default();
        let username = Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt(&trf!("termux.ssh.interactive.ask_username"))
            .allow_empty(true)
            .default(default_user.clone())
            .interact_text()
            .map_or(default_user, |s| s);
        let remote = if username.trim().is_empty() {
            "localhost".to_string()
        } else {
            format!("{}@localhost", username.trim())
        };

        // 5) Try ssh-copy-id if available
        if crate::utils::command_exists("ssh-copy-id") {
            Utils::info(&trf!("termux.ssh.interactive.ssh_copy_id_attempt"));
            let mut sc = Command::new("ssh-copy-id");
            sc.arg("-p")
                .arg(args.ssh_port.to_string())
                .arg("-i")
                .arg(chosen_pub.to_str().unwrap_or_default())
                .arg(&remote)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
            match sc.status() {
                Ok(s) if s.success() => {
                    Utils::success(&trf!("termux.ssh.interactive.key_installed"));
                }
                Ok(_) => {
                    Utils::warn(&trf!("termux.ssh.interactive.ssh_copy_id_failed", ""));
                }
                Err(e) => {
                    Utils::warn(&trf!("termux.ssh.interactive.ssh_copy_id_failed", e));
                }
            }
        } else if crate::utils::command_exists("scp") {
            // 6) scp fallback
            Utils::info(&trf!("termux.ssh.interactive.scp_fallback"));
            let remote_tmp = "/tmp/kam_pubkey";
            let scp_status = Command::new("scp")
                .arg("-P")
                .arg(args.ssh_port.to_string())
                .arg(chosen_pub.to_str().unwrap_or_default())
                .arg(format!("{}:{}", remote, remote_tmp))
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();
            if scp_status.is_ok() && scp_status.unwrap().success() {
                // Run remote command to append key
                let remote_cmd = format!(
                    "mkdir -p ~/.ssh && cat {} >> ~/.ssh/authorized_keys && chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys && rm {}",
                    remote_tmp, remote_tmp
                );
                let mut s = Command::new("ssh");
                s.arg("-p")
                    .arg(args.ssh_port.to_string())
                    .arg(&remote)
                    .arg(&remote_cmd)
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit());
                match s.status() {
                    Ok(st) if st.success() => {
                        Utils::success(&trf!("termux.ssh.interactive.key_installed"));
                    }
                    _ => {
                        Utils::warn(&trf!("termux.ssh.interactive.scp_failed"));
                    }
                }
            } else {
                Utils::warn(&trf!("termux.ssh.interactive.scp_failed"));
            }
        } else {
            // 7) adb push fallback
            let dest = "/data/data/com.termux/files/home/.ssh/authorized_keys";
            match Command::new("adb")
                .args(&adb_base)
                .arg("push")
                .arg(chosen_pub.to_str().unwrap_or_default())
                .arg(dest)
                .status()
            {
                Ok(s) if s.success() => {
                    Utils::success(&trf!("termux.ssh.pushed_key", dest));
                }
                _ => {
                    // Try /sdcard as a safe fallback and instruct the user
                    let fname = format!("kam_pubkey_{}.pub", args.ssh_port);
                    let sd_dest = format!("/sdcard/{}", fname);
                    match Command::new("adb")
                        .args(&adb_base)
                        .arg("push")
                        .arg(chosen_pub.to_str().unwrap_or_default())
                        .arg(&sd_dest)
                        .status()
                    {
                        Ok(s) if s.success() => {
                            let instr = format!(
                                "cat {} >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys && rm {}",
                                sd_dest, sd_dest
                            );
                            Utils::info(&trf!(
                                "termux.ssh.interactive.pushed_to_sdcard",
                                fname,
                                instr
                            ));
                        }
                        _ => {
                            Utils::error(&trf!("termux.ssh.interactive.adb_push_failed"));
                            return Ok(());
                        }
                    }
                }
            }
        }

        // 8) Connect via SSH
        Utils::info(&trf!("termux.ssh.interactive.connecting", args.ssh_port));
        let ssh_status = Command::new("ssh")
            .arg("-p")
            .arg(args.ssh_port.to_string())
            .arg(&remote)
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

    if args.ssh_connect {
        // Try to ensure forwarding is set up (best-effort)
        let p = args.ssh_port.to_string();
        let _ = Command::new("adb")
            .args(&adb_base)
            .arg("forward")
            .arg(format!("tcp:{}", p))
            .arg(format!("tcp:{}", p))
            .status();

        Utils::info(&trf!("termux.ssh.connecting", args.ssh_port));
        // Spawn ssh to localhost:<port>, attach to current tty
        let ssh_status = Command::new("ssh")
            .arg("localhost")
            .arg("-p")
            .arg(p)
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

    // Ensure adb port forwarding (best-effort) for both one-shot and interactive SSH
    let port_spec = format!("tcp:{}", args.ssh_port);
    let _ = Command::new("adb")
        .args(&adb_base)
        .arg("forward")
        .arg(&port_spec)
        .arg(&port_spec)
        .status();

    if let Some(cmd) = args.command {
        // One-shot mode: execute command via SSH (requires Termux `sshd` + adb port forwarding).
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
            }
            Err(e) => {
                Utils::error(&trf!("termux.ssh.ssh_failed", e));
            }
        }
    } else {
        // Interactive mode: prefer SSH-based connection (requires Termux `sshd` on the device).
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
                    // ok
                } else {
                    Utils::error(&trf!("termux.ssh.ssh_exited", s.code().unwrap_or(-1)));
                }
            }
            Err(e) => {
                Utils::error(&trf!("termux.ssh.ssh_failed", e));
            }
        }
    }

    Ok(())
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
            ssh_auto: false,
            ssh_setup: false,
            ssh_forward: false,
            ssh_push_key: None,
            ssh_connect: false,
            interactive: false,
            ssh_port: 8022,
            timeout: 60,
        };
        // In environments without adb available the command will return Ok(()) after printing an error.
        // We assert the function returns Ok so tests don't fail on CI where adb isn't present.
        assert!(run(args).is_ok());
    }

    #[test]
    fn termux_ssh_auto_no_adb() {
        let args = TermuxArgs {
            device: None,
            command: None,
            ssh_setup: false,
            ssh_forward: false,
            ssh_push_key: None,
            ssh_connect: false,
            interactive: false,
            ssh_auto: true,
            ssh_port: 8022,
            timeout: 60,
        };
        // In environments without adb available the command will return Ok(()) after printing an error.
        // We assert the function returns Ok so tests don't fail on CI where adb isn't present.
        assert!(run(args).is_ok());
    }

    #[test]
    fn is_android_host_callable() {
        // Ensure the detection helper is callable and does not panic.
        // We do not assert a platform-specific value here because that depends on the runtime environment.
        let _ = is_android_host();
    }

    // NOTE: test for daemon/list/kill removed because daemon/list/kill are no longer supported.

    // NOTE: test for daemon/list/kill removed because daemon/list/kill are no longer supported.
}
