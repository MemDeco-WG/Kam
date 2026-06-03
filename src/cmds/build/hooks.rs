use super::args::BuildArgs;
use super::hook_command::hook_command;
use super::hook_env::build_hook_env;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;
use crate::utils::Utils;

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

// 运行pre-build hooks
// 在构建之前执行，比如生成一些文件、检查环境等
/// Run pre-build hooks (e.g., user-provided scripts).
///
/// # Errors
/// Returns `KamError` if hook discovery or execution fails.
pub fn run_pre_build_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    args: &BuildArgs,
) -> Result<(), KamError> {
    run_hooks(project_root, kam_toml, output_dir, "pre-build", args)
}

// 运行post-build hooks
// 在构建之后执行，比如签名、上传、清理等
/// Run post-build hooks (e.g., cleanup or packaging steps).
///
/// # Errors
/// Returns `KamError` if hook discovery or execution fails.
pub fn run_post_build_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    args: &BuildArgs,
) -> Result<(), KamError> {
    run_hooks(project_root, kam_toml, output_dir, "post-build", args)
}

/// Run dev-build hooks used by `kam dev`.
///
/// # Errors
/// Returns `KamError` if hook discovery or execution fails.
pub fn run_dev_build_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    args: &BuildArgs,
) -> Result<(), KamError> {
    run_hooks(project_root, kam_toml, output_dir, "dev-build", args)
}

/// Run dev-webui hooks used by incremental `kam dev --watch`.
///
/// # Errors
/// Returns `KamError` if hook discovery or execution fails.
pub fn run_dev_webui_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    args: &BuildArgs,
) -> Result<(), KamError> {
    run_hooks(project_root, kam_toml, output_dir, "dev-webui", args)
}

/// Run dev-binary hooks used by incremental `kam dev --watch`.
///
/// # Errors
/// Returns `KamError` if hook discovery or execution fails.
pub fn run_dev_binary_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    args: &BuildArgs,
) -> Result<(), KamError> {
    run_hooks(project_root, kam_toml, output_dir, "dev-binary", args)
}

/// Run dev-sync hooks used by `kam dev`.
///
/// # Errors
/// Returns `KamError` if hook discovery or execution fails.
pub fn run_dev_sync_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    args: &BuildArgs,
) -> Result<(), KamError> {
    run_hooks(project_root, kam_toml, output_dir, "dev-sync", args)
}

/// Run dev-install hooks used by `kam dev --install`.
///
/// # Errors
/// Returns `KamError` if hook discovery or execution fails.
pub fn run_dev_install_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    args: &BuildArgs,
) -> Result<(), KamError> {
    run_hooks(project_root, kam_toml, output_dir, "dev-install", args)
}

/// Run dev-start hooks used when a `kam dev` session starts.
///
/// # Errors
/// Returns `KamError` if hook discovery or execution fails.
pub fn run_dev_start_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    args: &BuildArgs,
) -> Result<(), KamError> {
    run_hooks(project_root, kam_toml, output_dir, "dev-start", args)
}

/// Run dev-stop hooks used when a `kam dev` session stops.
///
/// # Errors
/// Returns `KamError` if hook discovery or execution fails.
pub fn run_dev_stop_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    args: &BuildArgs,
) -> Result<(), KamError> {
    run_hooks(project_root, kam_toml, output_dir, "dev-stop", args)
}

// 运行hooks的核心函数
// 这个函数有点长，但逻辑还算清晰
/// Execute hooks for the given stage.
///
/// # Errors
/// Returns `KamError` if hook discovery, validation, or execution fails.
fn run_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    stage: &str,
    args: &BuildArgs,
) -> Result<(), KamError> {
    // 打包模板时不运行hooks（模板不需要构建）
    if kam_toml.kam.module_type == ModuleType::Template {
        Utils::info(&trf!("hooks.skipping_hooks_for_template_packaging", stage));
        return Ok(());
    }

    // 模板提供的env自动加载已移除：
    //
    // 我们不再自动加载`.kam/template-vars.env`或`template-vars.env`到构建hook环境
    // 这样可以避免意外行为（比如隐式模板变量生效）并给调用者明确的控制
    //
    // 如果需要为hooks预加载env变量，写到项目根的`.env`文件
    // 或者在CI/自定义工作流中在调用`kam`之前显式export

    let hook_env = build_hook_env(project_root, kam_toml, output_dir, stage, args);
    let hooks_dir = hook_env.hooks_dir;

    if !hooks_dir.exists() {
        return Ok(());
    }

    // 直接执行hook文件，让OS决定执行行为
    // 这个runner故意避免OS特定的包装器或基于扩展名的分发
    // 如果脚本在当前平台无法执行，会失败并返回错误
    // 在确定hook总数后再显示header

    let entries = hook_entries(&hooks_dir)?;
    let total_hooks = entries.iter().filter(|e| e.path().is_file()).count();
    if !args.quiet {
        let hd = hooks_dir.display();
        Utils::section(format!(
            "✿ Running {stage} hooks from {hd} ({total_hooks} script(s)) ✿"
        ));
    }
    let pb = hook_progress(args, total_hooks);

    let mut idx = 0usize;
    for entry in entries {
        let path = entry.path();
        if should_skip_hook_path(&path) {
            continue;
        }
        idx += 1;
        run_one_hook(
            &path,
            project_root,
            &hook_env.vars,
            stage,
            idx,
            total_hooks,
            pb.as_ref(),
        )?;
    }

    if let Some(pb) = &pb {
        pb.finish_and_clear();
    }

    Ok(())
}

fn hook_entries(hooks_dir: &Path) -> Result<Vec<std::fs::DirEntry>, KamError> {
    let mut entries: Vec<_> = fs::read_dir(hooks_dir)
        .map_err(KamError::Io)?
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    Ok(entries)
}

fn hook_progress(args: &BuildArgs, total_hooks: usize) -> Option<ProgressBar> {
    if args.quiet || total_hooks == 0 || !std::io::stdout().is_terminal() {
        return None;
    }
    let pb = ProgressBar::new(total_hooks as u64);
    let style = ProgressStyle::with_template(
        "{spinner:.green.bold} {msg:.bold} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {elapsed_precise}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar())
    .progress_chars("█▉▊▋▌▍▎▏  ")
    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");
    pb.set_style(style);
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    Some(pb)
}

fn should_skip_hook_path(path: &Path) -> bool {
    !path.is_file()
        || path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|s| s.starts_with('.'))
}

fn run_one_hook(
    path: &Path,
    project_root: &Path,
    env_vars: &[(String, String)],
    stage: &str,
    idx: usize,
    total_hooks: usize,
    pb: Option<&ProgressBar>,
) -> Result<(), KamError> {
    let filename = path.file_name().map_or_else(
        || PathBuf::from("<unknown>").to_string_lossy().to_string(),
        |n| n.to_string_lossy().to_string(),
    );
    announce_hook(stage, idx, total_hooks, &filename, pb);

    let mut cmd = hook_command(path, project_root, env_vars, stage);
    let status_res = Utils::suspend_progressbar(pb, || cmd.status());
    match status_res {
        Ok(status) if status.success() => {
            finish_successful_hook(idx, total_hooks, &filename, pb);
            Ok(())
        }
        Ok(status) => {
            if let Some(pb) = pb {
                pb.finish_and_clear();
            }
            let status_code = status
                .code()
                .map_or_else(|| status.to_string(), |c| c.to_string());
            Err(KamError::CommandFailed(format!(
                "Hook script {filename} failed with status: {status_code}. (Output above)"
            )))
        }
        Err(e) => {
            warn_hook_exec_error(&filename, &e);
            if let Some(pb) = pb {
                pb.finish_and_clear();
            }
            Err(KamError::CommandFailed(format!(
                "Failed to execute hook {filename}: {e}"
            )))
        }
    }
}

fn announce_hook(
    stage: &str,
    idx: usize,
    total_hooks: usize,
    filename: &str,
    pb: Option<&ProgressBar>,
) {
    if let Some(pb) = pb {
        pb.set_message(format!("[{stage} {idx}/{total_hooks}] {filename}"));
    } else {
        Utils::executing(format!("[{stage} {idx}/{total_hooks}] {filename}"));
    }
}

fn finish_successful_hook(
    idx: usize,
    total_hooks: usize,
    filename: &str,
    pb: Option<&ProgressBar>,
) {
    if let Some(pb) = pb {
        pb.inc(1);
    } else {
        println!(
            "  {} [{}/{}] {}",
            "✓".green().bold(),
            idx,
            total_hooks,
            filename.green()
        );
    }
}

fn warn_hook_exec_error(filename: &str, err: &std::io::Error) {
    match err.kind() {
        std::io::ErrorKind::PermissionDenied => {
            Utils::warn(
                "Permission denied. Make sure the script is executable and accessible. On Unix, you may need to run: chmod +x <file>. On Windows, ensure the script association or runtime is available (or run via WSL/Git Bash).",
            );
        }
        std::io::ErrorKind::NotFound => {
            Utils::warn(format!(
                "Not found. Could not execute {filename}. Ensure the script has an interpreter or runtime available on the system (e.g., `sh`, `bash`, or `pwsh`), or invoke the script via a shell that is available on your platform."
            ));
        }
        _ => {}
    }
}
