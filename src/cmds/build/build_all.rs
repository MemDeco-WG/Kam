use colored::*;
use comfy_table::{Cell, Table};
use glob::glob;
use std::path::Path;
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

// 构建工作区成员
// 这个函数会切换目录，所以要注意恢复
fn build_workspace_member(project_path: &Path, member: &str, args: &BuildArgs) -> BuildResult {
    let member_path = project_path.join(member);
    if !member_path.exists() {
        Utils::warn(&format!("workspace member {} not found", member));
        return BuildResult {
            member: member.to_string(),
            success: false,
            error: Some("not found".to_string()),
        };
    }
    // 检查有没有kam.toml，没有就跳过
    if !member_path.join("kam.toml").exists() {
        if !args.quiet {
            Utils::info(&format!("Skipping {}: no kam.toml found", member));
        }
        return BuildResult {
            member: member.to_string(),
            success: false,
            error: Some("no kam.toml found".to_string()),
        };
    }
    if !args.quiet {
        Utils::banner(&format!("Building workspace member: {}", member));
    }
    // 保存当前目录，构建完要恢复
    // 虽然理论上应该用绝对路径，但有些代码可能依赖当前目录
    let original_cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            Utils::error(&format!("Failed to get current dir: {}", e));
            return BuildResult {
                member: member.to_string(),
                success: false,
                error: Some(format!("failed to get current dir: {}", e)),
            };
        }
    };
    // 切换到成员目录，这样相对路径就能正常工作了
    if let Err(e) = std::env::set_current_dir(&member_path) {
        Utils::error(&format!(
            "Failed to change to {}: {}",
            member_path.display(),
            e
        ));
        return BuildResult {
            member: member.to_string(),
            success: false,
            error: Some(format!("failed to change directory: {}", e)),
        };
    }

    let result = match KamToml::load_from_dir(".") {
        Ok(kt) => match build_project(std::path::Path::new("."), args, Some(kt)) {
            Ok(_) => BuildResult {
                member: member.to_string(),
                success: true,
                error: None,
            },
            Err(e) => {
                Utils::error(&format!("Failed to build {}: {}", member, e));
                BuildResult {
                    member: member.to_string(),
                    success: false,
                    error: Some(e.to_string()),
                }
            }
        },
        Err(e) => {
            Utils::warn(&format!(
                "Skipping {}: failed to load kam.toml: {}",
                member, e
            ));
            BuildResult {
                member: member.to_string(),
                success: false,
                error: Some(format!("failed to load kam.toml: {}", e)),
            }
        }
    };

    if let Err(e) = std::env::set_current_dir(original_cwd) {
        Utils::warn(&format!("Failed to restore cwd: {}", e));
    }

    result
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
        // 展开工作区成员的glob模式
        // 支持像 "modules/*" 这样的模式，会自动匹配所有子目录
        let mut expanded_members = Vec::new();

        for member_pattern in members {
            // 检查是否包含glob字符（*、?、[）
            if member_pattern.contains('*')
                || member_pattern.contains('?')
                || member_pattern.contains('[')
            {
                // 是glob模式，展开它
                let pattern_path = project_path.join(member_pattern);
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

                            // 只包含有kam.toml的目录（不然构建会失败）
                            if abs_entry.is_dir() && abs_entry.join("kam.toml").exists() {
                                if let Ok(rel_path) = abs_entry.strip_prefix(project_path) {
                                    let rel_str = rel_path.to_string_lossy().to_string();
                                    expanded_members.push(rel_str);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // glob模式无效，警告一下但继续
                        Utils::warn(&format!("Invalid glob pattern '{}': {}", member_pattern, e));
                    }
                }
            } else {
                // 不是glob模式，直接使用
                expanded_members.push(member_pattern.clone());
            }
        }

        if expanded_members.is_empty() {
            return Err(KamError::InvalidConfig(
                "No workspace members found after expanding patterns".to_string(),
            ));
        }

        for member in expanded_members {
            let result = build_workspace_member(project_path, &member, args);
            results.push(result);
        }
    } else {
        build_project(project_path, args, None)?;
        return Ok(());
    }

    let total_duration = start_time.elapsed();

    // 打印总结，让用户知道哪些成功了哪些失败了
    if !args.quiet {
        println!();
        Utils::section("✿ Workspace Build Summary ✿");
    }

    // 统计成功和失败的数量
    let success_count = results.iter().filter(|r| r.success).count();
    let failed_count = results.len() - success_count;

    if !args.quiet {
        let mut summary_table = Table::new();
        summary_table.set_header(vec!["模块", "状态"]);

        for result in &results {
            if result.success {
                summary_table.add_row(vec![
                    Cell::new(&result.member).fg(comfy_table::Color::White),
                    Cell::new("✓ 成功").fg(comfy_table::Color::Green),
                ]);
            } else {
                let error_msg = result
                    .error
                    .as_ref()
                    .unwrap_or(&"unknown error".to_string());
                summary_table.add_row(vec![
                    Cell::new(&result.member).fg(comfy_table::Color::White),
                    Cell::new(&format!("✗ 失败: {}", error_msg)).fg(comfy_table::Color::Red),
                ]);
            }
        }

        println!("{}", summary_table);
    }

    if !args.quiet {
        println!();
        let mut stats_table = Table::new();
        stats_table
            .set_header(vec!["统计项", "值"])
            .add_row(vec![
                Cell::new("总计").fg(comfy_table::Color::Cyan),
                Cell::new(results.len().to_string()).fg(comfy_table::Color::White),
            ])
            .add_row(vec![
                Cell::new("成功").fg(comfy_table::Color::Cyan),
                Cell::new(success_count.to_string()).fg(comfy_table::Color::Green),
            ])
            .add_row(vec![
                Cell::new("失败").fg(comfy_table::Color::Cyan),
                Cell::new(failed_count.to_string()).fg(comfy_table::Color::Red),
            ])
            .add_row(vec![
                Cell::new("总耗时").fg(comfy_table::Color::Cyan),
                Cell::new(&format!("{:.2}s", total_duration.as_secs_f64())).fg(comfy_table::Color::White),
            ]);

        println!("{}", stats_table);
    }

    // 如果有失败的，就返回错误
    // 虽然可以继续构建其他的，但通常用户希望所有都成功
    if failed_count > 0 {
        return Err(KamError::CommandFailed(format!(
            "{} workspace member(s) failed to build",
            failed_count
        )));
    }

    Ok(())
    // 全部成功！虽然可能花了点时间，但至少都构建完了
}
