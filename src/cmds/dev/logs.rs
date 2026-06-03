use std::fs;

use crate::errors::KamError;
use crate::utils::Utils;

use super::adb::{adb_root, adb_shell};
use super::context::DevContext;
use super::sync::shell_quote;

pub(super) fn show_logs(ctx: &DevContext, dry_run: bool) -> Result<(), KamError> {
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
