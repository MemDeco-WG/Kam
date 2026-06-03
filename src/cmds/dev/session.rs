use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::errors::KamError;

use super::args::DevArgs;
use super::context::DevContext;

pub(super) fn dev_mode_label(args: &DevArgs) -> &'static str {
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

pub(super) fn should_show_logs(args: &DevArgs) -> bool {
    args.logs
        || (!args.install
            && !args.watch
            && !args.webui
            && !args.hot
            && !args.sync_only
            && !args.mcp
            && args.forward.is_empty())
}

pub(super) fn reset_session_log(ctx: &DevContext) -> Result<(), KamError> {
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

pub(super) fn log_session(ctx: &DevContext, line: impl AsRef<str>) -> Result<(), KamError> {
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
