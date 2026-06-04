use clap::Args;
use ignore::WalkBuilder;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

#[derive(Args, Debug, Clone)]
pub struct DiffArgs {
    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,

    /// Override installed module path on device.
    #[arg(long)]
    pub module_path: Option<String>,

    /// Number of context lines to show.
    #[arg(short = 'U', long, default_value_t = 3)]
    pub context: usize,

    /// Print only a changed-path summary.
    #[arg(long)]
    pub stat: bool,

    /// Print planned adb/diff commands without executing.
    #[arg(long)]
    pub dry_run: bool,
}

struct DiffContext {
    module_id: String,
    local_module_root: PathBuf,
    device_module_root: String,
    device: Option<String>,
}

/// # Errors
/// Returns `KamError` when the project cannot be inspected or adb/diff commands fail.
pub fn run(args: &DiffArgs) -> Result<(), KamError> {
    let ctx = load_context(args)?;
    let temp = tempfile::tempdir().map_err(KamError::Io)?;
    let pulled = temp.path().join("device");
    let local_text = temp.path().join("local-text");
    let remote_text = temp.path().join("device-text");

    Utils::section("Kam module diff");
    Utils::info(format!("Module: {}", ctx.module_id));
    Utils::info(format!("Device: {}", ctx.device_module_root));
    Utils::info(format!("Local: {}", ctx.local_module_root.display()));

    if args.dry_run {
        print_dry_run(&ctx, args);
        return Ok(());
    }

    pull_installed_module(&ctx, &pulled)?;
    copy_text_tree(&ctx.local_module_root, &local_text)?;
    copy_text_tree(&pulled, &remote_text)?;
    run_diff(&remote_text, &local_text, args)
}

fn load_context(args: &DiffArgs) -> Result<DiffContext, KamError> {
    let project_root = std::env::current_dir()
        .map_err(KamError::Io)?
        .canonicalize()?;
    let kam_toml = KamToml::load_from_dir(&project_root)?;
    let module_id = kam_toml.prop.id.clone();
    let dev = kam_toml.dev.clone().unwrap_or_default();
    let local_module_root = kam_toml.kam.build.as_ref().map_or_else(
        || project_root.join("src").join(&module_id),
        |build| {
            build.source_dir.as_ref().map_or_else(
                || project_root.join("src").join(&module_id),
                |source| project_root.join(source.replace("{{id}}", &module_id)),
            )
        },
    );
    if !local_module_root.is_dir() {
        return Err(KamError::InvalidDirectory(format!(
            "Module source directory not found: {}",
            local_module_root.display()
        )));
    }
    let device_module_root = args
        .module_path
        .clone()
        .or(dev.module_path)
        .unwrap_or_else(|| format!("/data/adb/modules/{module_id}"));
    let device = args
        .device
        .clone()
        .or(dev.device)
        .filter(|value| !value.eq_ignore_ascii_case("auto"));

    Ok(DiffContext {
        module_id,
        local_module_root,
        device_module_root,
        device,
    })
}

fn pull_installed_module(ctx: &DiffContext, target: &Path) -> Result<(), KamError> {
    ensure_adb()?;
    match adb_pull(ctx, &ctx.device_module_root, target) {
        Ok(()) => Ok(()),
        Err(first_err) => {
            Utils::warn(format!(
                "Direct adb pull failed; retrying through root-readable staging: {first_err}"
            ));
            pull_installed_module_via_root(ctx, target)
        }
    }
}

fn adb_pull(ctx: &DiffContext, remote: &str, target: &Path) -> Result<(), KamError> {
    let target_str = target.to_string_lossy().to_string();
    let mut cmd = adb(ctx);
    cmd.args(["pull", remote, &target_str]);
    let status = Utils::run_and_stream_no_stderr_header(cmd).map_err(KamError::Io)?;
    if status.success() {
        Ok(())
    } else {
        Err(KamError::CommandFailed(format!(
            "adb pull failed with status {status}"
        )))
    }
}

fn pull_installed_module_via_root(ctx: &DiffContext, target: &Path) -> Result<(), KamError> {
    let stage_dir = "/sdcard/Download/kam-diff";
    let stage_archive = format!("{stage_dir}/{}.tar", ctx.module_id);
    let local_archive = target.with_extension("tar");
    let script = format!(
        "set -eu\nrm -f {archive}\nmkdir -p {stage_dir}\ncd {src}\ntar -cf {archive} .\nchmod a+r {archive}\n",
        archive = shell_quote(&stage_archive),
        stage_dir = shell_quote(stage_dir),
        src = shell_quote(&ctx.device_module_root),
    );
    adb_root(ctx, &script)?;
    let pull_result = adb_pull(ctx, &stage_archive, &local_archive);
    let _ = adb_shell(ctx, &format!("rm -f {}", shell_quote(&stage_archive)));
    pull_result?;
    extract_tar(&local_archive, target)
}

fn extract_tar(archive: &Path, target: &Path) -> Result<(), KamError> {
    fs::create_dir_all(target).map_err(KamError::Io)?;
    let file = fs::File::open(archive).map_err(KamError::Io)?;
    let mut archive = tar::Archive::new(file);
    archive.unpack(target).map_err(KamError::Io)
}

fn copy_text_tree(src: &Path, dst: &Path) -> Result<(), KamError> {
    for entry in WalkBuilder::new(src).git_ignore(false).build() {
        let entry = entry.map_err(|err| KamError::CommandFailed(format!("Walk error: {err}")))?;
        let path = entry.path();
        if !path.is_file() || is_binary(path) {
            continue;
        }
        let rel = path.strip_prefix(src).map_err(|_| {
            KamError::InvalidDirectory(format!("{} is outside {}", path.display(), src.display()))
        })?;
        let out = dst.join(rel);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(KamError::Io)?;
        }
        fs::copy(path, out).map_err(KamError::Io)?;
    }
    Ok(())
}

fn run_diff(device: &Path, local: &Path, args: &DiffArgs) -> Result<(), KamError> {
    if !crate::utils::command_exists("diff") {
        return Err(KamError::CommandFailed(
            "diff not found on PATH. Install diffutils.".to_string(),
        ));
    }
    let mut cmd = Command::new("diff");
    if args.stat {
        cmd.arg("-qr");
    } else {
        cmd.arg("-ruN").arg(format!("-U{}", args.context));
    }
    cmd.arg(device).arg(local);
    let status = Utils::run_and_stream_no_stderr_header(cmd).map_err(KamError::Io)?;
    match status.code() {
        Some(0 | 1) => Ok(()),
        _ => Err(KamError::CommandFailed(format!(
            "diff failed with status {status}"
        ))),
    }
}

fn ensure_adb() -> Result<(), KamError> {
    if crate::utils::command_exists("adb") {
        Ok(())
    } else {
        Err(KamError::CommandFailed(
            "adb not found on PATH. Install Android platform-tools.".to_string(),
        ))
    }
}

fn adb(ctx: &DiffContext) -> Command {
    let mut cmd = Command::new("adb");
    if let Some(device) = &ctx.device {
        cmd.arg("-s").arg(device);
    }
    cmd
}

fn adb_shell(ctx: &DiffContext, command: &str) -> Result<(), KamError> {
    let mut cmd = adb(ctx);
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

fn adb_root(ctx: &DiffContext, command: &str) -> Result<(), KamError> {
    let remote_script = format!("/sdcard/Download/kam-diff-root-{}.sh", ctx.module_id);
    let local_script = std::env::temp_dir().join(format!(
        "kam-diff-root-{}-{}.sh",
        ctx.module_id,
        std::process::id()
    ));
    let script =
        format!("#!/system/bin/sh\n(\n{command}\n)\nrc=$?\necho __kam_diff_rc=$rc\nexit $rc\n");
    fs::write(&local_script, script).map_err(KamError::Io)?;
    let local_script_str = local_script.to_string_lossy().to_string();
    adb_status(ctx, &["push", &local_script_str, &remote_script])?;
    let _ = fs::remove_file(&local_script);

    let output = root_script_output(ctx, &remote_script).map_err(KamError::Io);
    let _ = adb_shell(ctx, &format!("rm -f {}", shell_quote(&remote_script)));
    let output = output?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stdout
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim().starts_with("__kam_diff_rc="))
    {
        Utils::print_cmd_line(line);
    }
    for line in stderr.lines().filter(|line| !line.trim().is_empty()) {
        eprintln!("{line}");
    }
    let reported_rc = stdout
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix("__kam_diff_rc="))
        .and_then(|value| value.parse::<i32>().ok());
    if output.status.success() && reported_rc.unwrap_or(0) == 0 {
        Ok(())
    } else {
        Err(KamError::CommandFailed(format!(
            "adb root command failed with adb status {} and root rc {}",
            output.status,
            reported_rc.map_or_else(|| "unknown".to_string(), |value| value.to_string())
        )))
    }
}

fn root_script_output(
    ctx: &DiffContext,
    remote_script: &str,
) -> std::io::Result<std::process::Output> {
    let mut cmd = adb(ctx);
    cmd.arg("shell")
        .arg("su")
        .arg("-c")
        .arg(format!("sh {}", shell_quote(remote_script)))
        .stdin(Stdio::inherit())
        .output()
}

fn adb_status(ctx: &DiffContext, args: &[&str]) -> Result<(), KamError> {
    let mut cmd = adb(ctx);
    cmd.args(args).stdin(Stdio::inherit());
    let status = Utils::run_and_stream_no_stderr_header(cmd).map_err(KamError::Io)?;
    if status.success() {
        Ok(())
    } else {
        Err(KamError::CommandFailed(format!(
            "adb command failed with status {status}: {}",
            adb_command(ctx, args)
        )))
    }
}

fn adb_command(ctx: &DiffContext, args: &[&str]) -> String {
    let mut parts = vec!["adb".to_string()];
    if let Some(device) = &ctx.device {
        parts.push("-s".to_string());
        parts.push(shell_quote(device));
    }
    parts.extend(args.iter().map(|arg| shell_quote(arg)));
    parts.join(" ")
}

fn print_dry_run(ctx: &DiffContext, args: &DiffArgs) {
    let stage_archive = format!("/sdcard/Download/kam-diff/{}.tar", ctx.module_id);
    println!(
        "{}",
        adb_command(ctx, &["pull", &ctx.device_module_root, "DEVICE_TMP"])
    );
    println!(
        "fallback: adb shell su -c 'cd {} && tar -cf {} .'",
        shell_quote(&ctx.device_module_root),
        shell_quote(&stage_archive)
    );
    println!(
        "{}",
        adb_command(ctx, &["pull", &stage_archive, "DEVICE_TMP.tar"])
    );
    if args.stat {
        println!("diff -qr DEVICE_TEXT LOCAL_TEXT");
    } else {
        println!("diff -ruN -U{} DEVICE_TEXT LOCAL_TEXT", args.context);
    }
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

fn is_binary(path: &Path) -> bool {
    if let Ok(mut file) = fs::File::open(path) {
        let mut buffer = [0; 1024];
        if let Ok(n) = file.read(&mut buffer)
            && buffer[..n].contains(&0)
        {
            return true;
        }
    }
    if let Some(ext) = path.extension().and_then(|value| value.to_str()) {
        let binary_exts = [
            "png", "jpg", "jpeg", "gif", "webp", "ico", "zip", "tar", "gz", "xz", "zst", "so", "a",
            "o", "bin", "dex", "apk", "exe",
        ];
        return binary_exts.contains(&ext.to_ascii_lowercase().as_str());
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{copy_text_tree, is_binary};

    #[test]
    fn copy_text_tree_skips_binary_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        std::fs::create_dir_all(&src).expect("src");
        std::fs::write(src.join("service.sh"), "echo ok\n").expect("text");
        std::fs::write(src.join("payload.bin"), b"\0binary").expect("binary");

        copy_text_tree(&src, &dst).expect("copy text");

        assert!(dst.join("service.sh").exists());
        assert!(!dst.join("payload.bin").exists());
        assert!(is_binary(&src.join("payload.bin")));
    }
}
