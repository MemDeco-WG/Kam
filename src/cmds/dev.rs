use clap::{Args, Subcommand};
use glob::Pattern;
use ignore::WalkBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::cmds::build::args::BuildArgs;
use crate::cmds::build::build_project::determine_output_dir;
use crate::cmds::build::hooks::{
    run_dev_build_hooks, run_dev_install_hooks, run_dev_start_hooks, run_dev_stop_hooks,
    run_dev_sync_hooks,
};
use crate::cmds::install::InstallArgs;
use crate::cmds::mcp::{self, McpCommand, McpRuntime};
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::sections::DevSection;
use crate::utils::Utils;

#[derive(Args, Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct DevArgs {
    #[command(subcommand)]
    pub command: Option<DevCommand>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,

    /// Watch source files and repeat dev build/sync when they change.
    #[arg(long)]
    pub watch: bool,

    /// Hot-update allowlisted files without a full module install.
    #[arg(long)]
    pub hot: bool,

    /// Build/sync WebUI assets and forward the declared WebUI port when configured.
    #[arg(long)]
    pub webui: bool,

    /// Skip dev-build hooks and only synchronize allowed files to the device.
    #[arg(long)]
    pub sync_only: bool,

    /// Build and install a full ZIP for first install or structural changes.
    #[arg(long)]
    pub install: bool,

    /// Tail declared module logs and module-related logcat output.
    #[arg(long)]
    pub logs: bool,

    /// Enable the standard MCP runtime contract during the dev session.
    #[arg(long)]
    pub mcp: bool,

    /// Forward named endpoints. Accepts mcp, webui, or mcp:webui.
    #[arg(long, value_delimiter = ':')]
    pub forward: Vec<String>,

    /// Print planned local and device writes without executing them.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DevCommand {
    /// Diagnose adb, root, module path, hooks, logs, and MCP contract.
    Doctor,
}

#[derive(Debug)]
struct DevContext {
    project_root: PathBuf,
    kam_toml: KamToml,
    module_id: String,
    module_root: PathBuf,
    module_path: String,
    device: Option<String>,
    hot_patterns: Vec<String>,
    watch_paths: Vec<PathBuf>,
    logs: Vec<String>,
    forwards: Vec<String>,
    webui_port: Option<u16>,
    webui_local_port: Option<u16>,
    restart_command: Option<String>,
    output_dir: PathBuf,
    build_args: BuildArgs,
    mcp: McpRuntime,
}

/// Run the dev command.
///
/// # Errors
/// Returns `KamError` when project discovery, hook execution, adb operations, or
/// development diagnostics fail.
pub fn run(args: &DevArgs) -> Result<(), KamError> {
    let ctx = load_context(args)?;
    if matches!(args.command, Some(DevCommand::Doctor)) {
        return doctor(&ctx, args);
    }

    run_once(&ctx, args)?;
    if args.watch && !args.dry_run {
        watch(&ctx, args)?;
    }
    Ok(())
}

fn load_context(args: &DevArgs) -> Result<DevContext, KamError> {
    let project_root = std::env::current_dir()
        .map_err(KamError::Io)?
        .canonicalize()?;
    let kam_toml = KamToml::load_from_dir(&project_root)?;
    let module_id = kam_toml.prop.id.clone();
    let dev = kam_toml.dev.clone().unwrap_or_default();
    let module_root = kam_toml.kam.build.as_ref().map_or_else(
        || project_root.join("src").join(&module_id),
        |build| {
            build.source_dir.as_ref().map_or_else(
                || project_root.join("src").join(&module_id),
                |source| project_root.join(source.replace("{{id}}", &module_id)),
            )
        },
    );
    let module_path = dev
        .module_path
        .clone()
        .unwrap_or_else(|| format!("/data/adb/modules/{module_id}"));
    let device = args
        .device
        .clone()
        .or(dev.device.clone())
        .filter(|value| !value.eq_ignore_ascii_case("auto"));
    let hot_patterns = dev.hot.clone().unwrap_or_else(default_hot_patterns);
    let watch_paths = dev
        .watch
        .clone()
        .unwrap_or_else(default_watch_paths)
        .into_iter()
        .map(|path| project_root.join(path.replace("{{id}}", &module_id)))
        .collect();
    let logs = dev.logs.clone().unwrap_or_else(|| {
        vec![
            format!("{module_path}/logs/*.log"),
            format!("{module_path}/.log/*.log"),
        ]
    });
    let forwards = dev.forward.clone().unwrap_or_default();
    let webui_port = dev.webui_port;
    let webui_local_port = dev.webui_local_port.or(webui_port);
    let restart_command = dev.restart_command.clone();
    let build_args = BuildArgs {
        path: ".".to_string(),
        all: false,
        output: None,
        bump: false,
        release: false,
        sign: false,
        interactive: false,
        pre_release: false,
        quiet: false,
        jobs: None,
    };
    let output_dir = determine_output_dir(&project_root, &build_args, &kam_toml)?;
    let mcp = mcp::runtime_from_toml(
        project_root.clone(),
        &kam_toml,
        device.as_deref(),
        None,
        None,
    )?;

    Ok(DevContext {
        project_root,
        kam_toml,
        module_id,
        module_root,
        module_path,
        device,
        hot_patterns,
        watch_paths,
        logs,
        forwards,
        webui_port,
        webui_local_port,
        restart_command,
        output_dir,
        build_args,
        mcp,
    })
}

fn run_once(ctx: &DevContext, args: &DevArgs) -> Result<(), KamError> {
    print_plan(ctx, args)?;
    if args.dry_run {
        return Ok(());
    }

    detect_device(ctx)?;
    if args.install {
        run_dev_build_hooks(
            &ctx.project_root,
            &ctx.kam_toml,
            &ctx.output_dir,
            &ctx.build_args,
        )?;
        crate::cmds::build::run(&ctx.build_args)?;
        crate::cmds::install::run(&InstallArgs {
            path: None,
            manager: Some("Auto".to_string()),
            dry_run: false,
            adb: true,
            verbose: true,
            quiet: false,
            assume_yes: true,
        })?;
        run_dev_install_hooks(
            &ctx.project_root,
            &ctx.kam_toml,
            &ctx.output_dir,
            &ctx.build_args,
        )?;
    } else {
        if !args.sync_only && !args.hot {
            run_dev_build_hooks(
                &ctx.project_root,
                &ctx.kam_toml,
                &ctx.output_dir,
                &ctx.build_args,
            )?;
        }
        sync_hot_files(ctx)?;
        run_dev_sync_hooks(
            &ctx.project_root,
            &ctx.kam_toml,
            &ctx.output_dir,
            &ctx.build_args,
        )?;
    }

    run_dev_start_hooks(
        &ctx.project_root,
        &ctx.kam_toml,
        &ctx.output_dir,
        &ctx.build_args,
    )?;
    run_forwards(ctx, args, false)?;
    if args.mcp {
        enable_mcp(ctx, false)?;
    }
    if args.logs {
        show_logs(ctx, false)?;
    }
    Ok(())
}

fn print_plan(ctx: &DevContext, args: &DevArgs) -> Result<(), KamError> {
    Utils::section("Kam dev session plan");
    Utils::info(format!("Module: {}", ctx.module_id));
    Utils::info(format!("Local module root: {}", ctx.module_root.display()));
    Utils::info(format!("Device module root: {}", ctx.module_path));
    Utils::info(format!(
        "Device: {}",
        ctx.device.as_deref().unwrap_or("auto")
    ));
    if args.install {
        Utils::info("Mode: full dev install");
    } else if args.hot {
        Utils::info("Mode: hot update only");
    } else if args.sync_only {
        Utils::info("Mode: sync only");
    } else {
        Utils::info("Mode: dev build + hot sync");
    }
    for file in collect_hot_files(ctx)? {
        Utils::info(format!(
            "Will write device file: {}",
            remote_path(ctx, &file)?.display()
        ));
    }
    run_forwards(ctx, args, true)?;
    if args.mcp {
        enable_mcp(ctx, true)?;
    }
    if args.logs {
        show_logs(ctx, true)?;
    }
    if let Some(command) = &ctx.restart_command {
        Utils::info(format!("Would run restart command: {command}"));
    }
    Ok(())
}

fn sync_hot_files(ctx: &DevContext) -> Result<(), KamError> {
    for file in collect_hot_files(ctx)? {
        push_file_with_backup(ctx, &file)?;
    }
    if let Some(command) = &ctx.restart_command {
        Utils::info(format!("Running restart command: {command}"));
        adb_root(ctx, command)?;
    }
    Ok(())
}

fn collect_hot_files(ctx: &DevContext) -> Result<Vec<PathBuf>, KamError> {
    if !ctx.module_root.exists() {
        return Err(KamError::InvalidDirectory(format!(
            "Module source directory not found: {}",
            ctx.module_root.display()
        )));
    }
    let patterns = compile_patterns(&ctx.hot_patterns)?;
    let mut files = Vec::new();
    for entry in WalkBuilder::new(&ctx.module_root).git_ignore(false).build() {
        let entry = entry.map_err(|err| KamError::CommandFailed(format!("Walk error: {err}")))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let rel = path.strip_prefix(&ctx.module_root).unwrap_or(path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if rel_str.starts_with(".config/") {
            continue;
        }
        if patterns.iter().any(|pattern| pattern.matches(&rel_str)) {
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}

fn push_file_with_backup(ctx: &DevContext, local: &Path) -> Result<(), KamError> {
    let remote = remote_path(ctx, local)?;
    let remote_str = remote.to_string_lossy();
    let tmp_remote = format!("{remote_str}.kam-tmp");
    let parent = remote.parent().ok_or_else(|| {
        KamError::InvalidDirectory(format!("Invalid remote path: {}", remote.display()))
    })?;
    adb_status(ctx, &["push", &local.to_string_lossy(), &tmp_remote])?;
    adb_root(
        ctx,
        &format!("mkdir -p {}", shell_quote(&parent.to_string_lossy())),
    )?;
    adb_root(
        ctx,
        &format!(
            "set -e; had_old=0; [ ! -e {remote} ] || {{ cp -a {remote} {remote}.bak; had_old=1; }}; rollback() {{ if [ \"$had_old\" = 1 ] && [ -e {remote}.bak ]; then cp -a {remote}.bak {remote}; fi; rm -f {tmp}; }}; trap rollback EXIT HUP INT TERM; mv {tmp} {remote}; chmod 0644 {remote}; case {remote} in *.sh) chmod 0755 {remote};; esac; trap - EXIT HUP INT TERM",
            remote = shell_quote(&remote_str),
            tmp = shell_quote(&tmp_remote),
        ),
    )
}

fn remote_path(ctx: &DevContext, local: &Path) -> Result<PathBuf, KamError> {
    let rel = local.strip_prefix(&ctx.module_root).map_err(|_| {
        KamError::InvalidDirectory(format!(
            "{} is outside {}",
            local.display(),
            ctx.module_root.display()
        ))
    })?;
    Ok(PathBuf::from(&ctx.module_path).join(rel))
}

fn run_forwards(ctx: &DevContext, args: &DevArgs, dry_run: bool) -> Result<(), KamError> {
    let mut forwards = BTreeSet::new();
    forwards.extend(ctx.forwards.iter().map(String::as_str));
    forwards.extend(args.forward.iter().map(String::as_str));
    if args.mcp {
        forwards.remove("mcp");
    }
    if args.webui {
        forwards.insert("webui");
    }
    for forward in forwards {
        match forward {
            "mcp" => mcp::run_command(&ctx.mcp, &McpCommand::Forward, dry_run)?,
            "webui" => forward_webui(ctx, dry_run)?,
            other => Utils::warn(format!("Unknown forward target: {other}")),
        }
    }
    Ok(())
}

fn forward_webui(ctx: &DevContext, dry_run: bool) -> Result<(), KamError> {
    let Some(device_port) = ctx.webui_port else {
        Utils::info("WebUI forward requested but [dev].webui_port is not configured.");
        return Ok(());
    };
    let local_port = ctx.webui_local_port.unwrap_or(device_port);
    let local = format!("tcp:{local_port}");
    let remote = format!("tcp:{device_port}");
    if dry_run {
        Utils::info(format!(
            "Would run: {}",
            adb_command(ctx, &["forward", &local, &remote])
        ));
        Utils::info(format!("WebUI URL: http://127.0.0.1:{local_port}/"));
        return Ok(());
    }
    adb_status(ctx, &["forward", &local, &remote])?;
    Utils::success(format!("Forwarded WebUI: http://127.0.0.1:{local_port}/"));
    Ok(())
}

#[allow(dead_code)]
fn _run_dev_stop_hooks(ctx: &DevContext) -> Result<(), KamError> {
    run_dev_stop_hooks(
        &ctx.project_root,
        &ctx.kam_toml,
        &ctx.output_dir,
        &ctx.build_args,
    )
}

fn enable_mcp(ctx: &DevContext, dry_run: bool) -> Result<(), KamError> {
    mcp::run_command(&ctx.mcp, &McpCommand::Forward, dry_run)?;
    mcp::run_command(&ctx.mcp, &McpCommand::Enable, dry_run)?;
    mcp::run_command(&ctx.mcp, &McpCommand::Status { json: true }, dry_run)?;
    Utils::success(format!("MCP Streamable HTTP endpoint: {}", ctx.mcp.url()));
    Ok(())
}

fn show_logs(ctx: &DevContext, dry_run: bool) -> Result<(), KamError> {
    if dry_run {
        for log in &ctx.logs {
            Utils::info(format!("Would tail device log: {log}"));
        }
        Utils::info(format!(
            "Would run logcat filter for module id: {}",
            ctx.module_id
        ));
        return Ok(());
    }
    for log in &ctx.logs {
        let command = format!("for f in {log}; do [ ! -f \"$f\" ] || tail -n 80 \"$f\"; done");
        adb_root(ctx, &command)?;
    }
    adb_status(ctx, &["logcat", "-d", "-t", "200"])?;
    Ok(())
}

fn doctor(ctx: &DevContext, args: &DevArgs) -> Result<(), KamError> {
    Utils::section("kam dev doctor");
    check("kam.toml", ctx.project_root.join("kam.toml").exists());
    check("module source", ctx.module_root.exists());
    check("adb", crate::utils::command_exists("adb"));
    if crate::utils::command_exists("adb") && !args.dry_run {
        detect_device(ctx)?;
        let root_ok = adb_root(ctx, "id >/dev/null 2>&1").is_ok();
        check("adb root shell", root_ok);
        let module_ok = adb_root(ctx, &format!("[ -d {} ]", shell_quote(&ctx.module_path))).is_ok();
        check("device module dir", module_ok);
        let cli_ok = adb_root(ctx, &format!("[ -x {} ]", shell_quote(&ctx.mcp.cli_path))).is_ok();
        check("standard cli", cli_ok);
    }
    for stage in [
        "dev-build",
        "dev-sync",
        "dev-install",
        "dev-start",
        "dev-stop",
    ] {
        let path = hooks_dir(ctx).join(stage);
        Utils::info(format!(
            "{stage}: {}",
            if path.exists() {
                path.display().to_string()
            } else {
                "not configured".to_string()
            }
        ));
    }
    Ok(())
}

fn watch(ctx: &DevContext, args: &DevArgs) -> Result<(), KamError> {
    Utils::section("Watching for Kam dev changes");
    let mut previous = snapshot(ctx)?;
    loop {
        thread::sleep(Duration::from_secs(2));
        let current = snapshot(ctx)?;
        if current != previous {
            report_watch_changes(ctx, &previous, &current);
            run_once(
                ctx,
                &DevArgs {
                    watch: false,
                    ..args.clone()
                },
            )?;
            previous = current;
        }
    }
}

fn report_watch_changes(
    ctx: &DevContext,
    previous: &BTreeMap<PathBuf, SystemTime>,
    current: &BTreeMap<PathBuf, SystemTime>,
) {
    let changed = changed_files(previous, current);
    if changed.is_empty() {
        Utils::info("Detected file changes; running dev build/sync.");
        return;
    }
    let mut webui = false;
    let mut binary = false;
    let mut script_or_config = false;
    let mut structure = false;
    for file in &changed {
        let rel = file
            .strip_prefix(&ctx.project_root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        if rel.starts_with("webui/") || rel.contains("/webroot/") {
            webui = true;
        } else if rel.starts_with("crates/") || rel.contains("/.local/bin/") {
            binary = true;
        } else if has_ext(&rel, "sh")
            || has_ext(&rel, "prop")
            || has_ext(&rel, "rule")
            || rel.contains("templates/")
        {
            script_or_config = true;
        } else {
            structure = true;
        }
    }
    if webui {
        Utils::info(
            "Detected WebUI changes; dev-build hooks may rebuild WebUI, then hot sync webroot.",
        );
    }
    if binary {
        Utils::info(
            "Detected CLI/binary changes; dev-build hooks may rebuild .local/bin, then hot sync binaries.",
        );
    }
    if script_or_config {
        Utils::info(
            "Detected script/config changes; hot sync will push matching allowlisted files.",
        );
    }
    if structure {
        Utils::warn(
            "Detected module structure changes; a full `kam dev --install` may be required.",
        );
    }
}

fn changed_files(
    previous: &BTreeMap<PathBuf, SystemTime>,
    current: &BTreeMap<PathBuf, SystemTime>,
) -> Vec<PathBuf> {
    let mut files = BTreeSet::new();
    files.extend(previous.keys().cloned());
    files.extend(current.keys().cloned());
    files
        .into_iter()
        .filter(|file| previous.get(file) != current.get(file))
        .collect()
}

fn has_ext(path: &str, ext: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|value| value.eq_ignore_ascii_case(ext))
}

fn snapshot(ctx: &DevContext) -> Result<BTreeMap<PathBuf, SystemTime>, KamError> {
    let mut out = BTreeMap::new();
    for file in collect_hot_files(ctx)? {
        let modified = fs::metadata(&file)
            .and_then(|meta| meta.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        out.insert(file, modified);
    }
    for root in &ctx.watch_paths {
        if !root.exists() {
            continue;
        }
        for entry in WalkBuilder::new(root).git_ignore(false).build() {
            let entry =
                entry.map_err(|err| KamError::CommandFailed(format!("Walk error: {err}")))?;
            let path = entry.path();
            if path.is_file() {
                let modified = fs::metadata(path)
                    .and_then(|meta| meta.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                out.insert(path.to_path_buf(), modified);
            }
        }
    }
    Ok(out)
}

fn detect_device(ctx: &DevContext) -> Result<(), KamError> {
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
        Err(KamError::CommandFailed(
            "No usable adb device. Connect one device or pass --device <serial>.".to_string(),
        ))
    }
}

fn adb_status(ctx: &DevContext, args: &[&str]) -> Result<(), KamError> {
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

fn adb_command(ctx: &DevContext, args: &[&str]) -> String {
    let mut parts = vec!["adb".to_string()];
    if let Some(device) = &ctx.device {
        parts.push("-s".to_string());
        parts.push(shell_quote(device));
    }
    parts.extend(args.iter().map(|arg| shell_quote(arg)));
    parts.join(" ")
}

fn adb_root(ctx: &DevContext, command: &str) -> Result<(), KamError> {
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

fn hooks_dir(ctx: &DevContext) -> PathBuf {
    ctx.project_root.join(
        ctx.kam_toml
            .kam
            .build
            .as_ref()
            .and_then(|build| build.hooks_dir.as_deref())
            .unwrap_or("hooks"),
    )
}

fn compile_patterns(patterns: &[String]) -> Result<Vec<Pattern>, KamError> {
    patterns
        .iter()
        .map(|pattern| {
            Pattern::new(pattern).map_err(|err| {
                KamError::CommandFailed(format!("Invalid dev hot pattern '{pattern}': {err}"))
            })
        })
        .collect()
}

fn default_hot_patterns() -> Vec<String> {
    DevSection::default().hot.unwrap_or_default()
}

fn default_watch_paths() -> Vec<String> {
    DevSection::default().watch.unwrap_or_default()
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

fn check(label: &str, ok: bool) {
    if ok {
        Utils::success(format!("{label}: ok"));
    } else {
        Utils::warn(format!("{label}: check failed"));
    }
}
