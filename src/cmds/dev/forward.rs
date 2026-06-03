use crate::cmds::build::hooks::run_dev_stop_hooks;
use crate::cmds::mcp::{self, McpCommand};
use crate::errors::KamError;
use crate::utils::Utils;

use std::collections::BTreeSet;

use super::adb::{adb_command, adb_status};
use super::args::DevArgs;
use super::context::DevContext;
use super::session::log_session;

pub(super) fn run_forwards(
    ctx: &DevContext,
    args: &DevArgs,
    dry_run: bool,
) -> Result<(), KamError> {
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

pub(super) fn enable_mcp(ctx: &DevContext, dry_run: bool) -> Result<(), KamError> {
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
