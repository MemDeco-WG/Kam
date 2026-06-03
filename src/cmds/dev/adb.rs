use std::process::{Command, Stdio};

use crate::errors::KamError;
use crate::utils::Utils;

use super::context::DevContext;
use super::sync::shell_quote;

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
    let mut cmd = Command::new("adb");
    if let Some(device) = &ctx.device {
        cmd.arg("-s").arg(device);
    }
    cmd.arg("shell")
        .arg("su")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::inherit());
    let status = Utils::run_and_stream_no_stderr_header(cmd).map_err(KamError::Io)?;
    if status.success() {
        Ok(())
    } else {
        Err(KamError::CommandFailed(format!(
            "adb root command failed with status {status}: {command}"
        )))
    }
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
