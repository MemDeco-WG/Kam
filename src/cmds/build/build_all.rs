use comfy_table::{Cell, Table};
use glob::glob;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use super::args::BuildArgs;
use super::build_project::build_project;
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

struct BuildResult {
    member: String,
    success: bool,
    error: Option<String>,
}

struct WorkspaceBuildSummary {
    total: usize,
    succeeded: usize,
    failed: usize,
}

// 展开成员模式（支持 glob 和 [] 包裹）
// 返回 (是否用 [] 包裹, 展开后的成员列表)
fn expand_member_pattern(project_path: &Path, member_pattern: &str) -> (bool, Vec<String>) {
    // 检查是否用 [] 包裹
    let (is_bracketed, pattern) =
        if member_pattern.starts_with('[') && member_pattern.ends_with(']') {
            // 去掉首尾的 []
            let inner = &member_pattern[1..member_pattern.len() - 1];
            (true, inner.to_string())
        } else {
            (false, member_pattern.to_string())
        };

    let mut expanded = Vec::new();

    // 检查是否包含 glob 字符
    if pattern.contains('*') || pattern.contains('?') || pattern.contains('[') {
        // 是 glob 模式，展开它
        let pattern_path = project_path.join(&pattern);
        let pattern_str = pattern_path.to_string_lossy();

        match glob(&pattern_str) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    // 规范化路径为绝对路径
                    let abs_entry = if entry.is_absolute() {
                        entry.clone()
                    } else {
                        project_path.join(&entry)
                    };

                    // 只包含有 kam.toml 的目录
                    if abs_entry.is_dir()
                        && abs_entry.join("kam.toml").exists()
                        && let Ok(rel_path) = abs_entry.strip_prefix(project_path)
                    {
                        let rel_str = rel_path.to_string_lossy().to_string();
                        expanded.push(rel_str);
                    }
                }
            }
            Err(e) => {
                // glob 模式无效，警告一下但继续
                Utils::warn(&trf!("build.invalid_glob_pattern", pattern, e));
            }
        }
    } else {
        // 不是 glob 模式，直接使用
        expanded.push(pattern);
    }

    (is_bracketed, expanded)
}

// 构建工作区成员
// 这个函数会切换目录，所以要注意恢复
fn build_workspace_member(project_path: &Path, member: &str, args: &BuildArgs) -> BuildResult {
    let member_path = project_path.join(member);
    if !member_path.exists() {
        Utils::warn(&trf!("build.workspace_member_not_found", member));
        return BuildResult {
            member: member.to_string(),
            success: false,
            error: Some("not found".to_string()),
        };
    }
    // 检查有没有kam.toml，没有就跳过
    if !member_path.join("kam.toml").exists() {
        if !args.quiet {
            Utils::info(&trf!("build.skipping_no_kam_toml", member));
        }
        return BuildResult {
            member: member.to_string(),
            success: false,
            error: Some("no kam.toml found".to_string()),
        };
    }
    if !args.quiet {
        Utils::banner(&trf!("build.building_workspace_member", member));
    }

    // 注：不要切换全局 CWD（在并发环境下会导致竞态）。
    // 直接使用成员路径进行构建，避免修改进程级别的状态。
    match KamToml::load_from_dir(member_path.as_path()) {
        Ok(kt) => match build_project(member_path.as_path(), args, Some(kt)) {
            Ok(()) => BuildResult {
                member: member.to_string(),
                success: true,
                error: None,
            },
            Err(e) => {
                Utils::error(&trf!("build.failed_to_build", member, e));
                BuildResult {
                    member: member.to_string(),
                    success: false,
                    error: Some(e.to_string()),
                }
            }
        },
        Err(e) => {
            Utils::warn(&trf!("build.skipping_failed_load_kam_toml", member, e));
            BuildResult {
                member: member.to_string(),
                success: false,
                error: Some(format!("failed to load kam.toml: {e}")),
            }
        }
    }
}

fn build_workspace_members(
    project_path: &Path,
    members: &[String],
    args: &BuildArgs,
) -> Result<Vec<BuildResult>, KamError> {
    let mut results = Vec::new();

    // 解析成员列表，识别用 [] 包裹的项目。只有 [] 包裹的项目组并发执行；
    // 连续的并发组会合并，避免重复构建和不必要的竞争条件。
    let mut i = 0;
    while i < members.len() {
        let member_pattern = &members[i];
        let (is_bracketed, expanded) = expand_member_pattern(project_path, member_pattern);
        if expanded.is_empty() {
            i += 1;
            continue;
        }

        if is_bracketed {
            let mut all_concurrent_members = expanded;
            i += 1;

            while i < members.len() {
                let (next_bracketed, next_expanded) =
                    expand_member_pattern(project_path, &members[i]);
                if next_bracketed && !next_expanded.is_empty() {
                    all_concurrent_members.extend(next_expanded);
                    i += 1;
                } else {
                    break;
                }
            }

            let mut concurrent_results =
                build_concurrent_members(project_path, all_concurrent_members, args)?;
            results.append(&mut concurrent_results);
        } else {
            for member in expanded {
                results.push(build_workspace_member(project_path, &member, args));
            }
            i += 1;
        }
    }

    if results.is_empty() {
        return Err(KamError::InvalidConfig(crate::i18n::tr(
            "build.no_workspace_members",
        )));
    }

    Ok(results)
}

fn build_concurrent_members(
    project_path: &Path,
    members: Vec<String>,
    args: &BuildArgs,
) -> Result<Vec<BuildResult>, KamError> {
    let member_count = members.len();
    if member_count == 0 {
        return Ok(Vec::new());
    }

    let requested_jobs = args
        .jobs
        .unwrap_or_else(|| thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get));
    let actual_jobs = requested_jobs.min(member_count).max(1);

    if !args.quiet {
        Utils::info(&trf!(
            "build.concurrent_building_members",
            member_count,
            actual_jobs
        ));
    }

    let project_path = Arc::new(project_path.to_path_buf());
    let args = Arc::new(args.clone());
    let task_queue = Arc::new(Mutex::new(members));
    let (result_tx, result_rx) = mpsc::channel();

    let mut handles = Vec::new();
    for _ in 0..actual_jobs {
        let task_queue = Arc::clone(&task_queue);
        let result_tx = result_tx.clone();
        let project_path = Arc::clone(&project_path);
        let args = Arc::clone(&args);

        handles.push(thread::spawn(move || {
            build_worker_loop(&task_queue, &result_tx, &project_path, &args);
        }));
    }

    for handle in handles {
        if let Err(e) = handle.join() {
            return Err(KamError::CommandFailed(format!(
                "Worker thread panicked: {e:?}"
            )));
        }
    }

    drop(result_tx);
    Ok(result_rx.iter().collect())
}

fn build_worker_loop(
    task_queue: &Arc<Mutex<Vec<String>>>,
    result_tx: &mpsc::Sender<BuildResult>,
    project_path: &Arc<std::path::PathBuf>,
    args: &Arc<BuildArgs>,
) {
    loop {
        let member = match task_queue.lock() {
            Ok(mut guard) => guard.pop(),
            Err(poisoned) => {
                let result = BuildResult {
                    member: "<internal>".to_string(),
                    success: false,
                    error: Some(format!("task queue mutex poisoned: {poisoned:?}")),
                };
                let _ = result_tx.send(result);
                break;
            }
        };

        let Some(member) = member else {
            break;
        };

        let result = build_workspace_member(project_path.as_path(), &member, args.as_ref());
        if let Err(e) = result_tx.send(result) {
            Utils::error(format!("Failed to send result to channel: {e}"));
            break;
        }
    }
}

fn summarize_results(results: &[BuildResult]) -> WorkspaceBuildSummary {
    let succeeded = results.iter().filter(|result| result.success).count();
    WorkspaceBuildSummary {
        total: results.len(),
        succeeded,
        failed: results.len() - succeeded,
    }
}

fn print_workspace_summary(results: &[BuildResult]) {
    println!();
    Utils::section(crate::i18n::tr("workspace.summary.title"));

    let mut summary_table = Table::new();
    summary_table.set_header(vec![
        crate::i18n::tr("table.header.module"),
        crate::i18n::tr("table.header.status"),
    ]);

    for result in results {
        if result.success {
            summary_table.add_row(vec![
                Cell::new(&result.member).fg(comfy_table::Color::White),
                Cell::new(crate::i18n::tr("status.success")).fg(comfy_table::Color::Green),
            ]);
        } else {
            let error_msg = result.error.as_deref().unwrap_or("unknown error");
            summary_table.add_row(vec![
                Cell::new(&result.member).fg(comfy_table::Color::White),
                Cell::new(trf!("status.failed", error_msg)).fg(comfy_table::Color::Red),
            ]);
        }
    }

    println!("{summary_table}");
}

fn print_workspace_stats(summary: &WorkspaceBuildSummary, elapsed_secs: f64) {
    println!();
    let mut stats_table = Table::new();
    stats_table
        .set_header(vec![
            crate::i18n::tr("table.header.stat"),
            crate::i18n::tr("table.header.value"),
        ])
        .add_row(vec![
            Cell::new(crate::i18n::tr("table.stat.total")).fg(comfy_table::Color::Cyan),
            Cell::new(summary.total.to_string()).fg(comfy_table::Color::White),
        ])
        .add_row(vec![
            Cell::new(crate::i18n::tr("table.stat.succeeded")).fg(comfy_table::Color::Cyan),
            Cell::new(summary.succeeded.to_string()).fg(comfy_table::Color::Green),
        ])
        .add_row(vec![
            Cell::new(crate::i18n::tr("table.stat.failed")).fg(comfy_table::Color::Cyan),
            Cell::new(summary.failed.to_string()).fg(comfy_table::Color::Red),
        ])
        .add_row(vec![
            Cell::new(crate::i18n::tr("table.stat.total_duration")).fg(comfy_table::Color::Cyan),
            Cell::new(format!("{elapsed_secs:.2}s")).fg(comfy_table::Color::White),
        ]);

    println!("{stats_table}");
}

/// Build every configured workspace member, respecting Kam's sequential and
/// bracketed-concurrent workspace member syntax.
///
/// # Errors
///
/// Returns `KamError` when workspace metadata is invalid, a worker thread
/// panics, or any selected member fails to build.
pub fn run_build_all(project_path: &Path, args: &BuildArgs) -> Result<(), KamError> {
    let start_time = Instant::now();
    let root_kam_toml = KamToml::load_from_dir(project_path)?;
    let Some(workspace) = root_kam_toml.kam.workspace.as_ref() else {
        build_project(project_path, args, None)?;
        return Ok(());
    };
    let members = workspace
        .members
        .as_ref()
        .ok_or_else(|| KamError::InvalidConfig(crate::i18n::tr("build.no_workspace_section")))?;

    let results = build_workspace_members(project_path, members, args)?;
    let summary = summarize_results(&results);

    if !args.quiet {
        print_workspace_summary(&results);
        print_workspace_stats(&summary, start_time.elapsed().as_secs_f64());
    }

    if summary.failed > 0 {
        return Err(KamError::CommandFailed(trf!(
            "build.failed_workspace_members",
            summary.failed
        )));
    }

    Ok(())
}
