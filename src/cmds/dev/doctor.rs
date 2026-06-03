use std::path::PathBuf;

use crate::errors::KamError;
use crate::utils::Utils;

use super::adb::{adb_root, detect_device};
use super::args::DevArgs;
use super::context::DevContext;
use super::sync::shell_quote;

pub(super) fn doctor(ctx: &DevContext, args: &DevArgs) -> Result<(), KamError> {
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

fn check(label: &str, ok: bool) {
    if ok {
        Utils::success(format!("{label}: ok"));
    } else {
        Utils::warn(format!("{label}: check failed"));
    }
}
