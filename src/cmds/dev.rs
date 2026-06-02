use clap::{Args, Subcommand};
use glob::Pattern;
use ignore::WalkBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::cmds::build::args::BuildArgs;
use crate::cmds::build::build_project::determine_output_dir;
use crate::cmds::build::hooks::{
    run_dev_binary_hooks, run_dev_build_hooks, run_dev_install_hooks, run_dev_start_hooks,
    run_dev_stop_hooks, run_dev_sync_hooks, run_dev_webui_hooks,
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
    #[arg(long, global = true)]
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
    #[arg(long, global = true)]
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
    session_log: PathBuf,
    output_dir: PathBuf,
    build_args: BuildArgs,
    mcp: McpRuntime,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct WatchPlan {
    changed: Vec<PathBuf>,
    webui: bool,
    binary: bool,
    hot_files: Vec<PathBuf>,
    structure: bool,
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
    let session_log = project_root
        .join(".kam")
        .join("dev")
        .join("last-session.log");
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
        session_log,
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

    reset_session_log(ctx)?;
    log_session(ctx, format!("module={}", ctx.module_id))?;
    log_session(ctx, format!("module_path={}", ctx.module_path))?;
    log_session(ctx, format!("mode={}", dev_mode_label(args)))?;
    detect_device(ctx)?;
    if args.install {
        log_session(ctx, "stage=dev-build")?;
        run_dev_build_hooks(
            &ctx.project_root,
            &ctx.kam_toml,
            &ctx.output_dir,
            &ctx.build_args,
        )?;
        log_session(ctx, "command=kam build")?;
        crate::cmds::build::run(&ctx.build_args)?;
        log_session(ctx, "command=kam install --adb --manager Auto --yes")?;
        crate::cmds::install::run(&InstallArgs {
            path: None,
            manager: Some("Auto".to_string()),
            dry_run: false,
            adb: true,
            verbose: true,
            quiet: false,
            assume_yes: true,
        })?;
        log_session(ctx, "stage=dev-install")?;
        run_dev_install_hooks(
            &ctx.project_root,
            &ctx.kam_toml,
            &ctx.output_dir,
            &ctx.build_args,
        )?;
    } else {
        if args.webui {
            if args.sync_only {
                Utils::info("Sync-only mode: skipping dev-webui hooks.");
                log_session(ctx, "skip=dev-webui sync-only")?;
            } else {
                log_session(ctx, "stage=dev-webui")?;
                run_dev_webui_hooks(
                    &ctx.project_root,
                    &ctx.kam_toml,
                    &ctx.output_dir,
                    &ctx.build_args,
                )?;
            }
            sync_matching_hot_files(ctx, &["webroot/**"])?;
        } else if !args.sync_only && !args.hot {
            log_session(ctx, "stage=dev-build")?;
            run_dev_build_hooks(
                &ctx.project_root,
                &ctx.kam_toml,
                &ctx.output_dir,
                &ctx.build_args,
            )?;
            sync_hot_files(ctx)?;
        } else {
            sync_hot_files(ctx)?;
        }
        log_session(ctx, "stage=dev-sync")?;
        run_dev_sync_hooks(
            &ctx.project_root,
            &ctx.kam_toml,
            &ctx.output_dir,
            &ctx.build_args,
        )?;
    }

    log_session(ctx, "stage=dev-start")?;
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
    for file in planned_hot_files(ctx, args)? {
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

fn planned_hot_files(ctx: &DevContext, args: &DevArgs) -> Result<Vec<PathBuf>, KamError> {
    if args.webui {
        collect_matching_hot_files(ctx, &["webroot/**"])
    } else {
        collect_hot_files(ctx)
    }
}

fn dev_mode_label(args: &DevArgs) -> &'static str {
    if args.install {
        "install"
    } else if args.webui {
        "webui"
    } else if args.hot {
        "hot"
    } else if args.sync_only {
        "sync-only"
    } else {
        "dev-build-hot-sync"
    }
}

fn reset_session_log(ctx: &DevContext) -> Result<(), KamError> {
    if let Some(parent) = ctx.session_log.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }
    fs::write(
        &ctx.session_log,
        format!(
            "# kam dev session\nstarted_at_unix={}\n",
            now_unix_seconds()
        ),
    )
    .map_err(KamError::Io)
}

fn log_session(ctx: &DevContext, line: impl AsRef<str>) -> Result<(), KamError> {
    if let Some(parent) = ctx.session_log.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ctx.session_log)
        .map_err(KamError::Io)?;
    writeln!(file, "{}", line.as_ref()).map_err(KamError::Io)
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn sync_hot_files(ctx: &DevContext) -> Result<(), KamError> {
    sync_selected_hot_files(ctx, &collect_hot_files(ctx)?)?;
    run_restart_command(ctx)?;
    Ok(())
}

fn sync_selected_hot_files(ctx: &DevContext, files: &[PathBuf]) -> Result<(), KamError> {
    for file in files {
        let remote = remote_path(ctx, file)?;
        Utils::info(format!("Writing device file: {}", remote.display()));
        log_session(ctx, format!("write_device_file={}", remote.display()))?;
        push_file_with_backup(ctx, file)?;
    }
    Ok(())
}

fn sync_matching_hot_files(ctx: &DevContext, patterns: &[&str]) -> Result<(), KamError> {
    let files = collect_matching_hot_files(ctx, patterns)?;
    sync_selected_hot_files(ctx, &files)?;
    Ok(())
}

fn collect_matching_hot_files(
    ctx: &DevContext,
    patterns: &[&str],
) -> Result<Vec<PathBuf>, KamError> {
    let all = collect_hot_files(ctx)?;
    let patterns = compile_patterns(
        &patterns
            .iter()
            .map(|pattern| (*pattern).to_string())
            .collect::<Vec<_>>(),
    )?;
    let mut files = Vec::new();
    for file in all {
        let rel = file.strip_prefix(&ctx.module_root).map_err(|_| {
            KamError::InvalidDirectory(format!(
                "{} is outside {}",
                file.display(),
                ctx.module_root.display()
            ))
        })?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if patterns.iter().any(|pattern| pattern.matches(&rel_str)) {
            files.push(file);
        }
    }
    files.sort();
    Ok(files)
}

fn sync_incremental_hot_files(ctx: &DevContext, files: &[PathBuf]) -> Result<(), KamError> {
    let mut selected = Vec::new();
    for file in files {
        if file.exists() && is_allowed_hot_file(ctx, file)? {
            selected.push(file.clone());
        }
    }
    selected.sort();
    selected.dedup();
    if selected.is_empty() {
        Utils::info("No changed files matched the hot sync allowlist.");
    } else {
        sync_selected_hot_files(ctx, &selected)?;
    }
    Ok(())
}

fn run_restart_command(ctx: &DevContext) -> Result<(), KamError> {
    if let Some(command) = &ctx.restart_command {
        Utils::info(format!("Running restart command: {command}"));
        log_session(ctx, format!("restart_command={command}"))?;
        adb_root(ctx, command)?;
    }
    Ok(())
}

fn is_allowed_hot_file(ctx: &DevContext, file: &Path) -> Result<bool, KamError> {
    if !file.is_file() {
        return Ok(false);
    }
    matches_hot_path(ctx, file)
}

fn matches_hot_path(ctx: &DevContext, file: &Path) -> Result<bool, KamError> {
    let Ok(rel) = file.strip_prefix(&ctx.module_root) else {
        return Ok(false);
    };
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    if rel_str.starts_with(".config/") {
        return Ok(false);
    }
    Ok(compile_patterns(&ctx.hot_patterns)?
        .iter()
        .any(|pattern| pattern.matches(&rel_str)))
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
    log_session(
        ctx,
        format!("adb_forward_webui=tcp:{local_port}->tcp:{device_port}"),
    )?;
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
    if !dry_run {
        log_session(ctx, format!("mcp_endpoint={}", ctx.mcp.url()))?;
        log_session(ctx, format!("mcp_cli={}", ctx.mcp.cli_path))?;
    }
    mcp::run_command(&ctx.mcp, &McpCommand::Forward, dry_run)?;
    mcp::run_command(&ctx.mcp, &McpCommand::Enable, dry_run)?;
    mcp::run_command(&ctx.mcp, &McpCommand::Status { json: true }, dry_run)?;
    Utils::success(format!("MCP Streamable HTTP endpoint: {}", ctx.mcp.url()));
    Ok(())
}

fn show_logs(ctx: &DevContext, dry_run: bool) -> Result<(), KamError> {
    let install_logs = install_log_paths(ctx);
    if dry_run {
        Utils::info(format!(
            "Would show dev session log: {}",
            ctx.session_log.display()
        ));
        for log in &install_logs {
            Utils::info(format!("Would show install log if present: {log}"));
        }
        for log in &ctx.logs {
            Utils::info(format!("Would tail device log: {log}"));
        }
        Utils::info(format!(
            "Would run logcat filter for module id: {}",
            ctx.module_id
        ));
        return Ok(());
    }
    show_local_session_log(ctx)?;
    for log in &install_logs {
        let command = format!(
            "[ ! -f {log} ] || tail -n 120 {log}",
            log = shell_quote(log)
        );
        adb_root(ctx, &command)?;
    }
    for log in &ctx.logs {
        let command = format!("for f in {log}; do [ ! -f \"$f\" ] || tail -n 80 \"$f\"; done");
        adb_root(ctx, &command)?;
    }
    adb_shell(
        ctx,
        &format!(
            "logcat -d -t 300 2>/dev/null | grep -i {} || true",
            shell_quote(&ctx.module_id)
        ),
    )?;
    Ok(())
}

fn install_log_paths(ctx: &DevContext) -> Vec<String> {
    vec![
        format!("{}/install.log", ctx.module_path),
        format!("{}/.log/install.log", ctx.module_path),
        "/cache/magisk.log".to_string(),
        "/data/adb/ksu/logs/module_install.log".to_string(),
        "/data/adb/ap/logs/module_install.log".to_string(),
    ]
}

fn show_local_session_log(ctx: &DevContext) -> Result<(), KamError> {
    Utils::section("Last kam dev session");
    if ctx.session_log.exists() {
        let content = fs::read_to_string(&ctx.session_log).map_err(KamError::Io)?;
        for line in content
            .lines()
            .rev()
            .take(80)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            Utils::info(line);
        }
    } else {
        Utils::info(format!("No session log yet: {}", ctx.session_log.display()));
    }
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
        "dev-webui",
        "dev-binary",
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
            let plan = plan_watch_changes(ctx, &previous, &current)?;
            report_watch_plan(ctx, &plan);
            run_incremental(ctx, args, &plan)?;
            previous = current;
        }
    }
}

fn plan_watch_changes(
    ctx: &DevContext,
    previous: &BTreeMap<PathBuf, SystemTime>,
    current: &BTreeMap<PathBuf, SystemTime>,
) -> Result<WatchPlan, KamError> {
    let changed = changed_files(previous, current);
    if changed.is_empty() {
        return Ok(WatchPlan::default());
    }

    let mut plan = WatchPlan {
        changed,
        ..WatchPlan::default()
    };

    for file in &plan.changed {
        let rel_project = rel_to_project(ctx, file);
        if rel_project.starts_with("webui/") {
            plan.webui = true;
        } else if rel_project.starts_with("crates/") {
            plan.binary = true;
        } else if is_module_rel(ctx, file, "webroot/") {
            plan.webui = true;
        } else if is_module_rel(ctx, file, ".local/bin/") {
            plan.binary = true;
        } else if matches_hot_path(ctx, file)? {
            plan.hot_files.push(file.clone());
        } else {
            plan.structure = true;
        }
    }

    plan.hot_files.sort();
    plan.hot_files.dedup();
    Ok(plan)
}

fn report_watch_plan(ctx: &DevContext, plan: &WatchPlan) {
    if plan.changed.is_empty() {
        Utils::info("Detected file changes; no actionable path changes found.");
        return;
    }
    for file in &plan.changed {
        Utils::info(format!("Changed: {}", rel_to_project(ctx, file)));
    }
    if plan.webui {
        Utils::info("Detected WebUI changes; running dev-webui hooks, then hot syncing webroot.");
    }
    if plan.binary {
        Utils::info(
            "Detected CLI/binary changes; running dev-binary hooks, then hot syncing .local/bin.",
        );
    }
    if !plan.hot_files.is_empty() {
        Utils::info("Detected hot-file changes; pushing matching allowlisted files.");
    }
    if plan.structure {
        Utils::warn(
            "Detected module structure changes; a full `kam dev --install` may be required.",
        );
    }
}

fn run_incremental(ctx: &DevContext, args: &DevArgs, plan: &WatchPlan) -> Result<(), KamError> {
    if plan.changed.is_empty() {
        return Ok(());
    }

    detect_device(ctx)?;
    if plan.webui {
        if args.sync_only {
            Utils::info("Sync-only mode: skipping dev-webui hooks.");
        } else {
            run_dev_webui_hooks(
                &ctx.project_root,
                &ctx.kam_toml,
                &ctx.output_dir,
                &ctx.build_args,
            )?;
        }
        sync_matching_hot_files(ctx, &["webroot/**"])?;
    }
    if plan.binary {
        if args.sync_only {
            Utils::info("Sync-only mode: skipping dev-binary hooks.");
        } else {
            run_dev_binary_hooks(
                &ctx.project_root,
                &ctx.kam_toml,
                &ctx.output_dir,
                &ctx.build_args,
            )?;
        }
        sync_matching_hot_files(ctx, &[".local/bin/**"])?;
    }
    if !plan.hot_files.is_empty() {
        sync_incremental_hot_files(ctx, &plan.hot_files)?;
    }
    if plan.webui || plan.binary || !plan.hot_files.is_empty() {
        run_dev_sync_hooks(
            &ctx.project_root,
            &ctx.kam_toml,
            &ctx.output_dir,
            &ctx.build_args,
        )?;
        run_restart_command(ctx)?;
    }
    if plan.structure {
        Utils::warn(
            "Skipped non-hot structural changes. Run `kam dev --install` for a full install.",
        );
    }
    run_forwards(ctx, args, false)?;
    if args.mcp {
        enable_mcp(ctx, false)?;
    }
    if args.logs {
        show_logs(ctx, false)?;
    }
    Ok(())
}

fn rel_to_project(ctx: &DevContext, file: &Path) -> String {
    file.strip_prefix(&ctx.project_root)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_module_rel(ctx: &DevContext, file: &Path, prefix: &str) -> bool {
    file.strip_prefix(&ctx.module_root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .is_some_and(|rel| rel.starts_with(prefix))
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

fn adb_shell(ctx: &DevContext, command: &str) -> Result<(), KamError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> DevContext {
        let project_root = PathBuf::from("/tmp/kam-dev-test");
        let module_id = "MagicNet".to_string();
        let module_root = project_root.join("src").join(&module_id);
        let kam_toml = KamToml::default();
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
        let mcp = McpRuntime {
            project_root: project_root.clone(),
            module_id: module_id.clone(),
            module_path: format!("/data/adb/modules/{module_id}"),
            cli_path: format!("/data/adb/modules/{module_id}/cli"),
            device: None,
            device_port: 8765,
            local_port: 8765,
            endpoint: "/mcp".to_string(),
            transport: "streamable-http".to_string(),
        };
        DevContext {
            project_root,
            kam_toml,
            module_id,
            module_root,
            module_path: "/data/adb/modules/MagicNet".to_string(),
            device: None,
            hot_patterns: default_hot_patterns(),
            watch_paths: Vec::new(),
            logs: Vec::new(),
            forwards: Vec::new(),
            webui_port: None,
            webui_local_port: None,
            restart_command: None,
            session_log: PathBuf::from("/tmp/kam-dev-test/.kam/dev/last-session.log"),
            output_dir: PathBuf::from("/tmp/kam-dev-test/dist"),
            build_args,
            mcp,
        }
    }

    fn snapshot_from(files: &[PathBuf]) -> BTreeMap<PathBuf, SystemTime> {
        files
            .iter()
            .cloned()
            .map(|file| (file, SystemTime::UNIX_EPOCH))
            .collect()
    }

    fn changed_plan(file: PathBuf) -> WatchPlan {
        let ctx = test_context();
        let previous = BTreeMap::new();
        let current = snapshot_from(&[file]);
        plan_watch_changes(&ctx, &previous, &current).expect("watch plan")
    }

    #[test]
    fn watch_plan_routes_webui_source_to_webui_stage() {
        let ctx = test_context();
        let plan = changed_plan(ctx.project_root.join("webui/src/App.tsx"));
        assert!(plan.webui);
        assert!(!plan.binary);
        assert!(!plan.structure);
        assert!(plan.hot_files.is_empty());
    }

    #[test]
    fn watch_plan_routes_crate_source_to_binary_stage() {
        let ctx = test_context();
        let plan = changed_plan(ctx.project_root.join("crates/cli/src/main.rs"));
        assert!(plan.binary);
        assert!(!plan.webui);
        assert!(!plan.structure);
        assert!(plan.hot_files.is_empty());
    }

    #[test]
    fn watch_plan_routes_allowlisted_script_to_hot_sync() {
        let ctx = test_context();
        let script = ctx.module_root.join("service.sh");
        let plan = changed_plan(script.clone());
        assert!(!plan.webui);
        assert!(!plan.binary);
        assert!(!plan.structure);
        assert_eq!(plan.hot_files, vec![script]);
    }

    #[test]
    fn watch_plan_treats_runtime_config_as_structure_not_hot() {
        let ctx = test_context();
        let plan = changed_plan(ctx.module_root.join(".config/subscription.json"));
        assert!(!plan.webui);
        assert!(!plan.binary);
        assert!(plan.structure);
        assert!(plan.hot_files.is_empty());
    }
}
