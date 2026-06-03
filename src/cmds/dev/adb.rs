use std::fs;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::errors::KamError;
use crate::utils::Utils;

use super::context::DevContext;
use super::sync::shell_quote;

static ROOT_SCRIPT_SEQ: AtomicU64 = AtomicU64::new(0);

pub(super) fn detect_device(ctx: &DevContext) -> Result<(), KamError> {
    if !crate::utils::command_exists("adb") {
        return Err(KamError::CommandFailed(
            "adb not found on PATH. Install Android platform-tools.".to_string(),
        ));
    }
    let mut cmd = Command::new("adb");
    if let Some(device) = &ctx.device {
        cmd.arg("-s").arg(device);
    }
    cmd.arg("get-state");
    let output = cmd.output().map_err(KamError::Io)?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if stderr.is_empty() { stdout } else { stderr };
        Err(KamError::CommandFailed(format!(
            "No usable adb device. Command: {}. {}",
            adb_command(ctx, &["get-state"]),
            if detail.is_empty() {
                "Connect one device or pass --device <serial>.".to_string()
            } else {
                detail
            }
        )))
    }
}

pub(super) fn adb_status(ctx: &DevContext, args: &[&str]) -> Result<(), KamError> {
    let mut cmd = Command::new("adb");
    if let Some(device) = &ctx.device {
        cmd.arg("-s").arg(device);
    }
    cmd.args(args).stdin(Stdio::inherit());
    let status = Utils::run_and_stream_no_stderr_header(cmd).map_err(KamError::Io)?;
    if status.success() {
        Ok(())
    } else {
        Err(KamError::CommandFailed(format!(
            "adb command failed with status {status}"
        )))
    }
}

pub(super) fn adb_command(ctx: &DevContext, args: &[&str]) -> String {
    let mut parts = vec!["adb".to_string()];
    if let Some(device) = &ctx.device {
        parts.push("-s".to_string());
        parts.push(shell_quote(device));
    }
    parts.extend(args.iter().map(|arg| shell_quote(arg)));
    parts.join(" ")
}

pub(super) fn adb_root(ctx: &DevContext, command: &str) -> Result<(), KamError> {
    let seq = ROOT_SCRIPT_SEQ.fetch_add(1, Ordering::Relaxed);
    let local_script =
        std::env::temp_dir().join(format!("kam-dev-root-{}-{seq}.sh", std::process::id()));
    let remote_script = format!("/sdcard/Download/kam-dev-root-{}-{seq}.sh", ctx.module_id);
    let script = format!("#!/system/bin/sh\n{command}\nrc=$?\necho __kam_dev_rc=$rc\nexit $rc\n");
    fs::write(&local_script, script).map_err(KamError::Io)?;
    let local_script_str = local_script.to_string_lossy().to_string();
    adb_status(ctx, &["push", &local_script_str, &remote_script])?;
    let _ = fs::remove_file(&local_script);

    let mut cmd = Command::new("adb");
    if let Some(device) = &ctx.device {
        cmd.arg("-s").arg(device);
    }
    cmd.arg("shell")
        .arg("su")
        .arg("-c")
        .arg(format!("sh {}", shell_quote(&remote_script)))
        .stdin(Stdio::inherit());
    let output = cmd.output().map_err(KamError::Io)?;
    cleanup_remote_script(ctx, &remote_script);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stdout
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim().starts_with("__kam_dev_rc="))
    {
        Utils::print_cmd_line(line);
    }
    for line in stderr.lines().filter(|line| !line.trim().is_empty()) {
        eprintln!("{line}");
    }
    let reported_rc = stdout
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix("__kam_dev_rc="))
        .and_then(|value| value.parse::<i32>().ok());
    if output.status.success() && reported_rc.unwrap_or(0) == 0 {
        Ok(())
    } else {
        Err(KamError::CommandFailed(format!(
            "adb root command failed with adb status {} and root rc {}: {command}",
            output.status,
            reported_rc
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        )))
    }
}

fn cleanup_remote_script(ctx: &DevContext, remote_script: &str) {
    let mut cmd = Command::new("adb");
    if let Some(device) = &ctx.device {
        cmd.arg("-s").arg(device);
    }
    let _ = cmd
        .arg("shell")
        .arg("rm")
        .arg("-f")
        .arg(remote_script)
        .status();
}

pub(super) fn adb_shell(ctx: &DevContext, command: &str) -> Result<(), KamError> {
    let mut cmd = Command::new("adb");
    if let Some(device) = &ctx.device {
        cmd.arg("-s").arg(device);
    }
    cmd.arg("shell").arg(command).stdin(Stdio::inherit());
    let status = Utils::run_and_stream_no_stderr_header(cmd).map_err(KamError::Io)?;
    if status.success() {
        Ok(())
    } else {
        Err(KamError::CommandFailed(format!(
            "adb shell command failed with status {status}: {command}"
        )))
    }
}
