use glob::Pattern;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

use crate::errors::KamError;
use crate::types::kam_toml::sections::DevSection;
use crate::utils::Utils;

use super::adb::{adb_root, adb_status};
use super::context::DevContext;
use super::session::log_session;
use super::sync_plan::{
    SyncMode, all_mirror_roots, is_under_mirror_root, mirror_roots_for_patterns, sync_mode,
};

pub(super) fn planned_hot_files(
    ctx: &DevContext,
    args: &super::args::DevArgs,
) -> Result<Vec<PathBuf>, KamError> {
    if args.webui {
        Ok(Vec::new())
    } else {
        overlay_files(ctx, &collect_hot_files(ctx)?)
    }
}

pub(super) fn planned_mirror_roots(
    ctx: &DevContext,
    args: &super::args::DevArgs,
) -> Result<Vec<PathBuf>, KamError> {
    let roots = if args.webui {
        mirror_roots_for_patterns(ctx, &["webroot/**"])
    } else {
        all_mirror_roots(ctx)
    };
    roots
        .iter()
        .map(|root| remote_path(ctx, root))
        .collect::<Result<Vec<_>, _>>()
}

pub(super) fn sync_hot_files(ctx: &DevContext) -> Result<(), KamError> {
    let files = collect_hot_files(ctx)?;
    for root in all_mirror_roots(ctx) {
        sync_mirror_root(ctx, &root)?;
    }
    let overlay_files = overlay_files(ctx, &files)?;
    sync_selected_hot_files(ctx, &overlay_files)?;
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

pub(super) fn sync_matching_hot_files(ctx: &DevContext, patterns: &[&str]) -> Result<(), KamError> {
    for root in mirror_roots_for_patterns(ctx, patterns) {
        sync_mirror_root(ctx, &root)?;
    }
    let files = collect_matching_hot_files(ctx, patterns)?;
    let overlay_files = overlay_files(ctx, &files)?;
    sync_selected_hot_files(ctx, &overlay_files)?;
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
        if patterns.iter().any(|pattern| pattern.matches(&rel_str))
            && matches!(sync_mode(ctx, &file)?, Some(SyncMode::Overlay))
        {
            files.push(file);
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn sync_incremental_hot_files(
    ctx: &DevContext,
    files: &[PathBuf],
) -> Result<(), KamError> {
    let mut selected = Vec::new();
    for file in files {
        if file.exists()
            && is_allowed_hot_file(ctx, file)?
            && matches!(sync_mode(ctx, file)?, Some(SyncMode::Overlay))
        {
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

pub(super) fn run_restart_command(ctx: &DevContext) -> Result<(), KamError> {
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

pub(super) fn matches_hot_path(ctx: &DevContext, file: &Path) -> Result<bool, KamError> {
    let Ok(rel) = file.strip_prefix(&ctx.module_root) else {
        return Ok(false);
    };
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    if sync_mode(ctx, file)?.is_none() {
        return Ok(false);
    }
    Ok(compile_patterns(&ctx.hot_patterns)?
        .iter()
        .any(|pattern| pattern.matches(&rel_str)))
}

pub(super) fn collect_hot_files(ctx: &DevContext) -> Result<Vec<PathBuf>, KamError> {
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
        if patterns.iter().any(|pattern| pattern.matches(&rel_str)) {
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}

fn overlay_files(ctx: &DevContext, files: &[PathBuf]) -> Result<Vec<PathBuf>, KamError> {
    let mut out = Vec::new();
    for file in files {
        if matches!(sync_mode(ctx, file)?, Some(SyncMode::Overlay))
            && !is_under_mirror_root(ctx, file)?
        {
            out.push(file.clone());
        }
    }
    Ok(out)
}

fn sync_mirror_root(ctx: &DevContext, local_root: &Path) -> Result<(), KamError> {
    if !local_root.exists() {
        return Ok(());
    }
    let remote_root = remote_path(ctx, local_root)?;
    let remote_parent = remote_root.parent().ok_or_else(|| {
        KamError::InvalidDirectory(format!("Invalid remote path: {}", remote_root.display()))
    })?;
    let stage_root = PathBuf::from(ctx.sync_policy.rendered_stage_dir(&ctx.module_id)).join(
        local_root.file_name().ok_or_else(|| {
            KamError::InvalidDirectory(format!("Invalid mirror root: {}", local_root.display()))
        })?,
    );
    Utils::info(format!(
        "Mirroring device directory: {}",
        remote_root.display()
    ));
    log_session(ctx, format!("mirror_device_dir={}", remote_root.display()))?;
    adb_root(ctx, &prepare_stage_command(&stage_root))?;
    adb_status(
        ctx,
        &[
            "push",
            &local_root.to_string_lossy(),
            &stage_root.to_string_lossy(),
        ],
    )?;
    adb_root(
        ctx,
        &format!(
            "mkdir -p {parent} || exit 1; rm -rf {remote}.bak || exit 1; [ ! -e {remote} ] || cp -a {remote} {remote}.bak || exit 1; rm -rf {remote} || exit 1; cp -a {stage} {remote} || exit 1; rm -rf {stage} || exit 1",
            parent = shell_quote(&remote_parent.to_string_lossy()),
            remote = shell_quote(&remote_root.to_string_lossy()),
            stage = shell_quote(&stage_root.to_string_lossy()),
        ),
    )
}

fn prepare_stage_command(stage_root: &Path) -> String {
    let stage_path = stage_root.to_string_lossy();
    let parent_path = stage_root
        .parent()
        .unwrap_or_else(|| Path::new("/sdcard/Download/kam-dev"))
        .to_string_lossy();
    let stage = shell_quote(&stage_path);
    let parent = shell_quote(&parent_path);
    format!(
        "set -e; rm -rf {stage}; mkdir -p {parent}; chown shell:shell {parent} 2>/dev/null || chown 2000:2000 {parent} 2>/dev/null || true; chmod 0775 {parent} 2>/dev/null || true"
    )
}

fn prepare_stage_file_command(stage_file: &Path) -> String {
    let file_path = stage_file.to_string_lossy();
    let parent_path = stage_file
        .parent()
        .unwrap_or_else(|| Path::new("/sdcard/Download/kam-dev"))
        .to_string_lossy();
    let file = shell_quote(&file_path);
    let parent = shell_quote(&parent_path);
    format!("set -e; rm -f {file}; mkdir -p {parent}; chmod 0775 {parent} 2>/dev/null || true")
}

fn push_file_with_backup(ctx: &DevContext, local: &Path) -> Result<(), KamError> {
    let remote = remote_path(ctx, local)?;
    let remote_str = remote.to_string_lossy();
    let rel = local.strip_prefix(&ctx.module_root).map_err(|_| {
        KamError::InvalidDirectory(format!(
            "{} is outside {}",
            local.display(),
            ctx.module_root.display()
        ))
    })?;
    let tmp_remote = PathBuf::from(ctx.sync_policy.rendered_stage_dir(&ctx.module_id)).join(rel);
    let parent = remote.parent().ok_or_else(|| {
        KamError::InvalidDirectory(format!("Invalid remote path: {}", remote.display()))
    })?;
    let tmp_parent = tmp_remote.parent().ok_or_else(|| {
        KamError::InvalidDirectory(format!("Invalid stage path: {}", tmp_remote.display()))
    })?;
    adb_root(ctx, &prepare_stage_file_command(&tmp_remote))?;
    adb_status(
        ctx,
        &[
            "push",
            &local.to_string_lossy(),
            &tmp_remote.to_string_lossy(),
        ],
    )?;
    adb_root(
        ctx,
        &format!("mkdir -p {}", shell_quote(&parent.to_string_lossy())),
    )?;
    adb_root(
        ctx,
        &format!(
            "had_old=0; [ ! -e {remote} ] || {{ cp -a {remote} {remote}.bak || exit 1; had_old=1; }}; rollback() {{ if [ \"$had_old\" = 1 ] && [ -e {remote}.bak ]; then cp -a {remote}.bak {remote}; fi; rm -f {tmp}; }}; trap rollback EXIT HUP INT TERM; mv {tmp} {remote} || exit 1; chmod 0644 {remote} || exit 1; case {remote} in *.sh) chmod 0755 {remote} || exit 1;; esac; trap - EXIT HUP INT TERM",
            remote = shell_quote(&remote_str),
            tmp = shell_quote(&tmp_remote.to_string_lossy()),
        ),
    )?;
    adb_root(
        ctx,
        &format!(
            "rmdir {} 2>/dev/null || true",
            shell_quote(&tmp_parent.to_string_lossy())
        ),
    )
}

pub(super) fn remote_path(ctx: &DevContext, local: &Path) -> Result<PathBuf, KamError> {
    let rel = local.strip_prefix(&ctx.module_root).map_err(|_| {
        KamError::InvalidDirectory(format!(
            "{} is outside {}",
            local.display(),
            ctx.module_root.display()
        ))
    })?;
    Ok(PathBuf::from(&ctx.module_path).join(rel))
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

pub(super) fn default_hot_patterns() -> Vec<String> {
    DevSection::default().hot.unwrap_or_default()
}

pub(super) fn default_watch_paths() -> Vec<String> {
    DevSection::default().watch.unwrap_or_default()
}

pub(super) fn default_sync_policy() -> crate::types::kam_toml::sections::DevSyncSection {
    crate::types::kam_toml::sections::DevSyncSection::default()
}

pub(super) fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{prepare_stage_command, prepare_stage_file_command};

    #[test]
    fn prepare_stage_command_makes_parent_writable_by_adb_shell() {
        let command = prepare_stage_command(Path::new("/sdcard/Download/kam-dev/MagicNet/webroot"));

        assert!(command.contains("rm -rf /sdcard/Download/kam-dev/MagicNet/webroot"));
        assert!(command.contains("mkdir -p /sdcard/Download/kam-dev/MagicNet"));
        assert!(command.contains("chown shell:shell /sdcard/Download/kam-dev/MagicNet"));
        assert!(command.contains("chown 2000:2000 /sdcard/Download/kam-dev/MagicNet"));
        assert!(command.contains("chmod 0775 /sdcard/Download/kam-dev/MagicNet"));
    }

    #[test]
    fn prepare_stage_file_command_makes_parent_writable_by_adb_shell() {
        let command =
            prepare_stage_file_command(Path::new("/sdcard/Download/kam-dev/MagicNet/service.sh"));

        assert!(command.contains("rm -f /sdcard/Download/kam-dev/MagicNet/service.sh"));
        assert!(command.contains("mkdir -p /sdcard/Download/kam-dev/MagicNet"));
        assert!(command.contains("chmod 0775 /sdcard/Download/kam-dev/MagicNet"));
    }
}
