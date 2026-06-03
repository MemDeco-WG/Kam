use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) fn hook_command(
    path: &Path,
    project_root: &Path,
    env_vars: &[(String, String)],
    stage: &str,
) -> Command {
    if stage.starts_with("dev-") {
        dev_hook_command(path, project_root, env_vars)
    } else {
        release_hook_command(path, project_root, env_vars)
    }
}

fn dev_hook_command(path: &Path, project_root: &Path, env_vars: &[(String, String)]) -> Command {
    let log_path = project_root
        .join(".kam")
        .join("dev")
        .join("last-session.log");
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut cmd = Command::new("sh");
    cmd.current_dir(project_root)
        .envs(env_vars.iter().cloned())
        .env("KAM_HOOK_PATH", path)
        .env("KAM_DEV_SESSION_LOG", &log_path)
        .arg("-c")
        .arg(
            r#"
status_file="${TMPDIR:-/tmp}/kam-dev-hook-status.$$"
{
  printf '\n## hook %s\n' "$KAM_HOOK_PATH"
  "$KAM_HOOK_PATH" "$@"
  printf '%s' "$?" > "$status_file"
} 2>&1 | tee -a "$KAM_DEV_SESSION_LOG"
status="$(cat "$status_file" 2>/dev/null || printf 1)"
rm -f "$status_file"
exit "$status"
"#,
        )
        .arg("kam-dev-hook")
        .stdin(Stdio::inherit());
    cmd
}

fn release_hook_command(
    path: &Path,
    project_root: &Path,
    env_vars: &[(String, String)],
) -> Command {
    let mut cmd = Command::new(path);
    cmd.current_dir(project_root)
        .envs(env_vars.iter().cloned())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit());
    cmd
}
