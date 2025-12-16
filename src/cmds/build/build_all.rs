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

// 展开成员模式（支持 glob 和 [] 包裹）
// 返回 (是否用 [] 包裹, 展开后的成员列表)
fn expand_member_pattern(
    project_path: &Path,
    member_pattern: &str,
) -> Result<(bool, Vec<String>), KamError> {
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
                    if abs_entry.is_dir() && abs_entry.join("kam.toml").exists() {
                        if let Ok(rel_path) = abs_entry.strip_prefix(project_path) {
                            let rel_str = rel_path.to_string_lossy().to_string();
                            expanded.push(rel_str);
                        }
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

    Ok((is_bracketed, expanded))
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
            Ok(_) => BuildResult {
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
                error: Some(format!("failed to load kam.toml: {}", e)),
            }
        }
    }
}

pub fn run_build_all(project_path: &Path, args: &BuildArgs) -> Result<(), KamError> {
    let start_time = Instant::now();
    let root_kam_toml = KamToml::load_from_dir(project_path)?;
    let workspace = root_kam_toml
        .kam
        .workspace
        .as_ref()
        .ok_or_else(|| KamError::InvalidConfig("No workspace section found".to_string()))?;

    let mut results = Vec::new();

    if let Some(members) = &workspace.members {
        // 解析成员列表，识别用 [] 包裹的项目
        // 重要：只有用 [] 包裹的项目组才会并发执行，没有包裹的必须顺序执行
        // 连续的 [] 包裹的项目会被合并成一个并发组
        // 这样可以避免重复构建和竞争条件
        let mut i = 0;
        while i < members.len() {
            let member_pattern = &members[i];
            match expand_member_pattern(project_path, member_pattern) {
                Ok((is_bracketed, expanded)) => {
                    if expanded.is_empty() {
                        i += 1;
                        continue;
                    }

                    if is_bracketed {
                        // 收集所有连续的 [] 包裹的项目，合并成一个并发组
                        let mut all_concurrent_members = expanded;
                        i += 1;

                        // 继续收集后续的 [] 包裹的项目
                        while i < members.len() {
                            match expand_member_pattern(project_path, &members[i]) {
                                Ok((next_bracketed, next_expanded)) => {
                                    if next_bracketed && !next_expanded.is_empty() {
                                        // 也是 [] 包裹的，合并到当前并发组
                                        all_concurrent_members.extend(next_expanded);
                                        i += 1;
                                    } else {
                                        // 不是 [] 包裹的，停止收集
                                        break;
                                    }
                                }
                                Err(_) => {
                                    // 展开失败，停止收集
                                    break;
                                }
                            }
                        }

                        // 现在处理合并后的并发组
                        // 用 [] 包裹的项目组，使用线程池并发执行
                        // 注意：只有这里才会使用并发，-j 参数也只在这里生效
                        let member_count = all_concurrent_members.len();

                        // 确定线程池大小（仅在并发组中使用）
                        let num_jobs = args.jobs.unwrap_or_else(|| {
                            thread::available_parallelism()
                                .map(|n| n.get())
                                .unwrap_or(1)
                        });

                        // 确保至少有一个线程（如果 member_count > 0）
                        let actual_jobs = if member_count > 0 {
                            num_jobs.min(member_count).max(1)
                        } else {
                            0
                        };

                        if !args.quiet {
                            Utils::info(&trf!(
                                "build.concurrent_building_members",
                                member_count,
                                actual_jobs
                            ));
                        }

                        // 如果没有任务，跳过
                        if member_count == 0 {
                            continue;
                        }

                        // 使用线程池并发执行
                        let project_path = Arc::new(project_path.to_path_buf());
                        let args = Arc::new(args.clone());

                        // 创建任务队列和结果 channel
                        let task_queue = Arc::new(Mutex::new(all_concurrent_members));
                        let (result_tx, result_rx) = mpsc::channel();

                        // 创建工作线程
                        let mut handles = Vec::new();
                        for _ in 0..actual_jobs {
                            let task_queue = Arc::clone(&task_queue);
                            let result_tx = result_tx.clone();
                            let project_path = Arc::clone(&project_path);
                            let args = Arc::clone(&args);

                            let handle = thread::spawn(move || {
                                loop {
                                    // 从任务队列中取任务
                                    let member = {
                                        let mut queue = task_queue.lock().unwrap();
                                        queue.pop()
                                    };

                                    if let Some(member) = member {
                                        let result = build_workspace_member(
                                            project_path.as_path(),
                                            &member,
                                            args.as_ref(),
                                        );
                                        result_tx.send(result).unwrap();
                                    } else {
                                        // 没有更多任务，退出
                                        break;
                                    }
                                }
                            });
                            handles.push(handle);
                        }

                        // 等待所有工作线程完成
                        // 注意：工作线程中的 result_tx clone 会在线程结束时自动销毁
                        for handle in handles {
                            handle.join().unwrap();
                        }

                        // 收集结果
                        // 所有工作线程已完成，它们的 result_tx clone 已经销毁
                        // 现在关闭主发送端，使 channel 关闭，然后收集所有结果
                        drop(result_tx);
                        // 从 channel 中收集所有结果（iter 会在 channel 关闭时结束）
                        let mut concurrent_results: Vec<_> = result_rx.iter().collect();
                        results.append(&mut concurrent_results);
                    } else {
                        // 没有用 [] 包裹的项目，必须顺序执行
                        // 重要：这里不使用并发，即使设置了 -j 参数也不会生效
                        // 这样可以避免重复构建和竞争条件
                        for member in expanded {
                            let result = build_workspace_member(project_path, &member, args);
                            results.push(result);
                        }
                        i += 1;
                    }
                }
                Err(e) => {
                    Utils::warn(&trf!(
                        "Failed to expand member pattern '{}': {}",
                        member_pattern,
                        e
                    ));
                    i += 1;
                }
            }
        }

        if results.is_empty() {
            return Err(KamError::InvalidConfig(
                "No workspace members found after expanding patterns".to_string(),
            ));
        }
    } else {
        build_project(project_path, args, None)?;
        return Ok(());
    }

    let total_duration = start_time.elapsed();

    // 打印总结，让用户知道哪些成功了哪些失败了
    if !args.quiet {
        println!();
        Utils::section(crate::i18n::tr_key("workspace.summary.title"));
    }

    // 统计成功和失败的数量
    let success_count = results.iter().filter(|r| r.success).count();
    let failed_count = results.len() - success_count;

    if !args.quiet {
        let mut summary_table = Table::new();
        summary_table.set_header(vec![
            crate::i18n::tr_key("table.header.module"),
            crate::i18n::tr_key("table.header.status"),
        ]);

        for result in &results {
            if result.success {
                summary_table.add_row(vec![
                    Cell::new(&result.member).fg(comfy_table::Color::White),
                    Cell::new(crate::i18n::tr_key("status.success")).fg(comfy_table::Color::Green),
                ]);
            } else {
                let default_error = "unknown error".to_string();
                let error_msg = result.error.as_ref().unwrap_or(&default_error);
                summary_table.add_row(vec![
                    Cell::new(&result.member).fg(comfy_table::Color::White),
                    Cell::new(trf!("status.failed", error_msg)).fg(comfy_table::Color::Red),
                ]);
            }
        }

        println!("{}", summary_table);
    }

    if !args.quiet {
        println!();
        let mut stats_table = Table::new();
        stats_table
            .set_header(vec![
                crate::i18n::tr_key("table.header.stat"),
                crate::i18n::tr_key("table.header.value"),
            ])
            .add_row(vec![
                Cell::new(crate::i18n::tr_key("table.stat.total")).fg(comfy_table::Color::Cyan),
                Cell::new(results.len().to_string()).fg(comfy_table::Color::White),
            ])
            .add_row(vec![
                Cell::new(crate::i18n::tr_key("table.stat.succeeded")).fg(comfy_table::Color::Cyan),
                Cell::new(success_count.to_string()).fg(comfy_table::Color::Green),
            ])
            .add_row(vec![
                Cell::new(crate::i18n::tr_key("table.stat.failed")).fg(comfy_table::Color::Cyan),
                Cell::new(failed_count.to_string()).fg(comfy_table::Color::Red),
            ])
            .add_row(vec![
                Cell::new(crate::i18n::tr_key("table.stat.total_duration"))
                    .fg(comfy_table::Color::Cyan),
                Cell::new(format!("{:.2}s", total_duration.as_secs_f64()))
                    .fg(comfy_table::Color::White),
            ]);

        println!("{}", stats_table);
    }

    // 如果有失败的，就返回错误
    // 虽然可以继续构建其他的，但通常用户希望所有都成功
    if failed_count > 0 {
        return Err(KamError::CommandFailed(trf!(
            "build.failed_workspace_members",
            failed_count
        )));
    }

    Ok(())
    // 全部成功！虽然可能花了点时间，但至少都构建完了
}
