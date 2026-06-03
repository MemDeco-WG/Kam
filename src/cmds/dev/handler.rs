use crate::cmds::build::hooks::{
    run_dev_build_hooks, run_dev_install_hooks, run_dev_start_hooks, run_dev_sync_hooks,
    run_dev_webui_hooks,
};
use crate::cmds::install::InstallArgs;
use crate::errors::KamError;
use crate::utils::Utils;

use super::adb::detect_device;
use super::args::{DevArgs, DevCommand};
use super::context::{DevContext, load_context};
use super::forward::{enable_mcp, run_forwards};
use super::logs::show_logs;
use super::session::{dev_mode_label, log_session, reset_session_log, should_show_logs};
use super::sync::{
    planned_hot_files, planned_mirror_roots, remote_path, sync_hot_files, sync_matching_hot_files,
};
use super::watch::watch;

/// Run the dev command.
///
/// # Errors
/// Returns `KamError` when project discovery, hook execution, adb operations, or
/// development diagnostics fail.
pub fn run(args: &DevArgs) -> Result<(), KamError> {
    let ctx = load_context(args)?;
    if matches!(args.command, Some(DevCommand::Doctor)) {
        return super::doctor::doctor(&ctx, args);
    }

    run_once(&ctx, args)?;
    if args.watch && !args.dry_run {
        watch(&ctx, args)?;
    }
    Ok(())
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
    if should_show_logs(args) {
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
    for root in planned_mirror_roots(ctx, args)? {
        Utils::info(format!("Will mirror device directory: {}", root.display()));
    }
    run_forwards(ctx, args, true)?;
    if args.mcp {
        enable_mcp(ctx, true)?;
    }
    if should_show_logs(args) {
        show_logs(ctx, true)?;
    }
    if let Some(command) = &ctx.restart_command {
        Utils::info(format!("Would run restart command: {command}"));
    }
    Ok(())
}
