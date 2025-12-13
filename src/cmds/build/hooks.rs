use super::args::BuildArgs;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;
use crate::utils::Utils;

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::IsTerminal;
use std::path::Path;
use std::process::{Command, Stdio};

// 运行pre-build hooks
// 在构建之前执行，比如生成一些文件、检查环境等
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
pub fn run_post_build_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    args: &BuildArgs,
) -> Result<(), KamError> {
    run_hooks(project_root, kam_toml, output_dir, "post-build", args)
}

// 运行hooks的核心函数
// 这个函数有点长，但逻辑还算清晰
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

    // Read .env file into a local map instead of mutating process environment.
    // This keeps builds thread-safe and ensures parallel builds don't interfere with each other.
    let env_path = project_root.join(".env");
    let mut parsed_env: HashMap<String, String> = HashMap::new();
    if env_path.exists() {
        // 手动解析 .env 文件并放入 parsed_env（不会修改进程级环境）
        if let Ok(content) = fs::read_to_string(&env_path) {
            for (line_num, line) in content.lines().enumerate() {
                let line = line.trim();
                // Skip empty lines and comments
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                // Handle 'export KEY=VALUE' format (common in shell scripts)
                let line = if line.starts_with("export ") {
                    line.strip_prefix("export ").unwrap().trim()
                } else {
                    line
                };

                // Parse KEY=VALUE
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();

                    // Validate key (must be valid identifier)
                    if key.is_empty() || !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        Utils::warn(&trf!(
                            "hooks.invalid_env_variable_name",
                            key,
                            line_num + 1,
                            env_path.display()
                        ));
                        continue;
                    }

                    let value = value.trim();
                    // Remove quotes if present (both single and double)
                    let value = if (value.starts_with('"') && value.ends_with('"'))
                        || (value.starts_with('\'') && value.ends_with('\''))
                    {
                        if value.len() >= 2 {
                            &value[1..value.len() - 1]
                        } else {
                            value
                        }
                    } else {
                        value
                    };

                    // Put into local parsed_env map — do not modify global env in-process
                    parsed_env.insert(key.to_string(), value.to_string());
                } else if !line.is_empty() {
                    Utils::warn(&trf!(
                        "hooks.malformed_env_line",
                        line_num + 1,
                        env_path.display(),
                        line
                    ));
                }
            }
        }
    }

    // 模板提供的env自动加载已移除：
    //
    // 我们不再自动加载`.kam/template-vars.env`或`template-vars.env`到构建hook环境
    // 这样可以避免意外行为（比如隐式模板变量生效）并给调用者明确的控制
    //
    // 如果需要为hooks预加载env变量，写到项目根的`.env`文件
    // 或者在CI/自定义工作流中在调用`kam`之前显式export

    let hooks_dir_name = kam_toml
        .kam
        .build
        .as_ref()
        .and_then(|b| b.hooks_dir.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("hooks");

    let hooks_root = project_root.join(hooks_dir_name);
    let hooks_dir = hooks_root.join(stage);

    if !hooks_dir.exists() {
        return Ok(());
    }

    // Prepare environment variables
    let module_root = if let Some(build) = &kam_toml.kam.build {
        if let Some(custom_src) = &build.source_dir {
            project_root.join(custom_src)
        } else {
            project_root.join("src").join(&kam_toml.prop.id)
        }
    } else {
        project_root.join("src").join(&kam_toml.prop.id)
    };
    let web_root = module_root.join("webroot");

    // 确定仓库和ref（用于KAM_REPO / KAM_REPO_REF）
    // 优先级：.env（项目根） > 环境变量 > kam.toml配置
    let mut detected_repo = String::new();
    if let Some(repo) = parsed_env.get("GITHUB_REPOSITORY") {
        // .env 中的值优先
        detected_repo = repo.clone();
    } else if let Ok(repo) = std::env::var("GITHUB_REPOSITORY") {
        // GitHub Actions环境
        detected_repo = repo;
    } else if !kam_toml
        .mmrl
        .as_ref()
        .and_then(|m| m.repo.as_ref())
        .and_then(|r| r.repository.as_ref())
        .unwrap_or(&String::new())
        .is_empty()
    {
        // 从kam.toml读取
        detected_repo = kam_toml
            .mmrl
            .as_ref()
            .and_then(|m| m.repo.as_ref())
            .and_then(|r| r.repository.as_ref())
            .unwrap_or(&String::new())
            .clone();
    }

    // 确定仓库ref（分支名）
    // 优先级：.env（项目根） > 环境变量 > git命令
    let mut detected_ref = String::new();
    if let Some(github_ref) = parsed_env.get("GITHUB_REF") {
        // 从 .env 读取（如果存在）
        detected_ref = github_ref
            .strip_prefix("refs/heads/")
            .unwrap_or(github_ref)
            .to_string();
    } else if let Ok(github_ref) = std::env::var("GITHUB_REF") {
        // GitHub Actions环境，去掉refs/heads/前缀
        detected_ref = github_ref
            .strip_prefix("refs/heads/")
            .unwrap_or(&github_ref)
            .to_string();
    } else {
        // 尝试运行git命令获取当前分支
        // 虽然可能失败（比如不在git仓库里），但不影响构建
        if let Ok(out) = Command::new("git")
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .current_dir(project_root)
            .output()
        {
            if out.status.success() {
                detected_ref = String::from_utf8_lossy(&out.stdout).trim().to_string();
            }
        }
    }

    // 构建环境变量列表，跟踪key避免重复
    // 用HashSet快速检查是否已存在
    let mut env_vars: Vec<(String, String)> = Vec::new();
    let mut env_keys: HashSet<String> = HashSet::new();

    // 辅助闭包：插入环境变量，保留现有key（优先级）
    // 如果key已存在就不覆盖（先设置的优先级更高）
    let mut add_env = |k: &str, value: String| {
        if !env_keys.contains(k) {
            env_keys.insert(k.to_string());
            env_vars.push((k.to_string(), value));
        }
    };

    // 将 .env（parsed_env）中的变量先加入 env_vars（优先级最高）
    for (k, v) in parsed_env.iter() {
        add_env(k, v.clone());
    }

    // Template init variables are *not* merged automatically here anymore.
    // The environment for hooks is now constructed from the canonical values below
    // (KAM_PROJECT_ROOT, KAM_DIST_DIR, KAM_MODULE_ID, etc.). If you want to preload
    // additional values for testing or CI, put them in a `.env` that will be loaded
    // earlier above or export them before running `kam`.
    //
    // (This intentional no-op keeps build reproducibility and avoids implicit overrides.)

    // Basic environment variables
    add_env(
        "KAM_PROJECT_ROOT",
        project_root.to_string_lossy().to_string(),
    );
    add_env("KAM_HOOKS_ROOT", hooks_root.to_string_lossy().to_string());
    add_env("KAM_MODULE_ROOT", module_root.to_string_lossy().to_string());
    add_env("KAM_WEB_ROOT", web_root.to_string_lossy().to_string());
    add_env("KAM_DIST_DIR", output_dir.to_string_lossy().to_string());
    add_env("KAM_MODULE_ID", kam_toml.prop.id.clone());
    add_env("KAM_MODULE_VERSION", kam_toml.prop.version.clone());
    add_env(
        "KAM_MODULE_VERSION_CODE",
        kam_toml.prop.versionCode.to_string(),
    );
    add_env("KAM_MODULE_NAME", kam_toml.prop.get_name().to_string());
    add_env(
        "KAM_MODULE_AUTHOR",
        kam_toml
            .prop
            .author
            .as_ref()
            .unwrap_or(&String::new())
            .clone(),
    );
    add_env(
        "KAM_MODULE_DESCRIPTION",
        kam_toml.prop.get_description().to_string(),
    );
    add_env(
        "KAM_MODULE_UPDATE_JSON",
        kam_toml
            .prop
            .updateJson
            .as_ref()
            .unwrap_or(&String::new())
            .clone(),
    );

    // Build flags & state
    add_env("KAM_STAGE", stage.to_string());
    add_env(
        "KAM_BUMP_ENABLED",
        if args.bump {
            "1".to_string()
        } else {
            "0".to_string()
        },
    );
    add_env(
        "KAM_RELEASE_ENABLED",
        if args.release {
            "1".to_string()
        } else {
            "0".to_string()
        },
    );
    add_env(
        "KAM_SIGN_ENABLED",
        if args.sign {
            "1".to_string()
        } else {
            "0".to_string()
        },
    );
    add_env(
        "KAM_PRE_RELEASE",
        if args.pre_release {
            "1".to_string()
        } else {
            "0".to_string()
        },
    );
    add_env(
        "KAM_INTERACTIVE",
        if args.interactive {
            "1".to_string()
        } else {
            "0".to_string()
        },
    );

    // Repo detection
    add_env(
        "KAM_GIT_REPO",
        kam_toml
            .mmrl
            .as_ref()
            .and_then(|m| m.repo.as_ref())
            .and_then(|r| r.repository.as_ref())
            .unwrap_or(&String::new())
            .clone(),
    );
    add_env("KAM_GITHUB_REPO", detected_repo.clone());
    add_env("KAM_REPO", detected_repo.clone());
    add_env("KAM_REPO_REF", detected_ref.clone());
    add_env("KAM_RELEASE_TAG", kam_toml.prop.version.clone());

    // Add prop.* as environment variables for hooks (KAM_PROP_*)
    add_env("KAM_PROP_ID", kam_toml.prop.id.clone());
    add_env("KAM_PROP_NAME", kam_toml.prop.get_name().to_string());
    add_env("KAM_PROP_VERSION", kam_toml.prop.version.clone());
    add_env(
        "KAM_PROP_VERSION_CODE",
        kam_toml.prop.versionCode.to_string(),
    );
    add_env(
        "KAM_PROP_AUTHOR",
        kam_toml
            .prop
            .author
            .as_ref()
            .unwrap_or(&String::new())
            .clone(),
    );
    add_env(
        "KAM_PROP_DESCRIPTION",
        kam_toml.prop.get_description().to_string(),
    );

    // Add templated variables from kam.tmpl.variables as environment variables KAM_TMPL_<NAME>
    if let Some(tmpl_section) = &kam_toml.kam.tmpl {
        for (var_name, var_def) in tmpl_section.variables.iter() {
            // Upper-case and normalize var name into env var (dots and hyphens will be normalized to underscores)
            let env_key = format!(
                "KAM_TMPL_{}",
                var_name
                    .to_ascii_uppercase()
                    .replace('.', "_")
                    .replace('-', "_")
            );
            // Default value may exist in variable definition, or fallback to empty string
            let env_val = var_def.default.clone().unwrap_or_else(|| String::new());
            add_env(&env_key, env_val);
        }
    }

    // Auto-generate environment variables from flattened kam.toml values:
    // For each flattened key (e.g. "prop.id") create KAM_PROP_ID to make input consistent.
    let kt_vars = crate::template::TemplateVariableProcessor::flatten_kam_toml(kam_toml);
    for (k, v) in kt_vars {
        let env_key_base = k.to_ascii_uppercase().replace('.', "_").replace('-', "_");
        let env_key = format!("KAM_{}", env_key_base);
        add_env(&env_key, v);
    }

    // 直接执行hook文件，让OS决定执行行为
    // 这个runner故意避免OS特定的包装器或基于扩展名的分发
    // 如果脚本在当前平台无法执行，会失败并返回错误
    // 在确定hook总数后再显示header

    let mut entries: Vec<_> = fs::read_dir(&hooks_dir)
        .map_err(KamError::Io)?
        .filter_map(|e| e.ok()) // 忽略读取失败的条目
        .collect();

    // 按文件名排序，确保执行顺序确定（01-init.sh, 02-build.sh等）
    // 这样用户可以控制执行顺序
    entries.sort_by_key(|e| e.file_name());

    // Determine if we should show a progress bar
    let show_progress = !args.quiet && std::io::stdout().is_terminal();
    let total_hooks = entries.iter().filter(|e| e.path().is_file()).count();
    if !args.quiet {
        Utils::section(&format!(
            "✿ Running {} hooks from {} ({} script(s)) ✿",
            stage,
            hooks_dir.display(),
            total_hooks
        ));
    }
    let pb = if show_progress && total_hooks > 0 {
        let pb = ProgressBar::new(total_hooks as u64);
        let style = ProgressStyle::with_template(
            "{spinner:.green.bold} {msg:.bold} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {elapsed_precise}",
        )
        .unwrap()
        .progress_chars("█▉▊▋▌▍▎▏  ")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");
        pb.set_style(style);
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    // Iterate hooks in deterministic order and execute each non-hidden file directly.
    // The hook runner doesn't attempt to interpret file extensions or choose a runtime;
    // it simply invokes the file and defers to the platform to handle the execution.
    let mut idx = 0usize;
    for entry in entries {
        let path = entry.path();
        if path.is_file() {
            // Skip hidden files
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            let filename = path.file_name().unwrap().to_string_lossy();
            idx += 1;
            // Set progress bar message if present; otherwise print executing line with index info
            if let Some(pb) = &pb {
                pb.set_message(format!("[{} {}/{}] {}", stage, idx, total_hooks, filename));
            } else {
                Utils::executing(&format!("[{} {}/{}] {}", stage, idx, total_hooks, filename));
            }

            // 流式输出stdout/stderr，实时显示命令输出
            // 使用suspend()暂停进度条，确保输出不会被进度条干扰

            let mut cmd = Command::new(&path);
            cmd.current_dir(project_root)
                .envs(env_vars.iter().cloned()) // 设置所有环境变量
                .stdout(Stdio::inherit()) // 直接继承，实时输出
                .stderr(Stdio::inherit())
                .stdin(Stdio::inherit());

            // Execute the command while suspending the hook progress bar so stdout/stderr
            // and any interactive prompts are visible without being overwritten.
            let status_res = Utils::suspend_progressbar(pb.as_ref(), || cmd.status());

            // 处理命令执行结果
            match status_res {
                Ok(status) => {
                    if !status.success() {
                        // 命令执行失败
                        if let Some(pb) = &pb {
                            pb.finish_and_clear();
                        }

                        let status_code = status
                            .code()
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| status.to_string());
                        return Err(KamError::CommandFailed(format!(
                            "Hook script {} failed with status: {}. (Output above)",
                            filename, status_code
                        )));
                    }
                    // 命令执行成功
                    if let Some(pb) = &pb {
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
                Err(e) => {
                    // 执行命令时出错（权限、找不到文件等）
                    match e.kind() {
                        std::io::ErrorKind::PermissionDenied => {
                            // 权限被拒绝，提示用户
                            Utils::warn(
                                "Permission denied. Make sure the script is executable and accessible. On Unix, you may need to run: chmod +x <file>. On Windows, ensure the script association or runtime is available (or run via WSL/Git Bash).",
                            );
                        }
                        std::io::ErrorKind::NotFound => {
                            // 找不到解释器，提示用户
                            Utils::warn(&format!(
                                "Not found. Could not execute {}. Ensure the script has an interpreter or runtime available on the system (e.g., `sh`, `bash`, or `pwsh`), or invoke the script via a shell that is available on your platform.",
                                filename
                            ));
                        }
                        _ => {}
                    }
                    if let Some(pb) = &pb {
                        pb.finish_and_clear();
                    }
                    return Err(KamError::CommandFailed(format!(
                        "Failed to execute hook {}: {}",
                        filename, e
                    )));
                }
            }
        }
    }

    // Finish the progress bar if shown
    if let Some(pb) = &pb {
        pb.finish_with_message(format!("✓ Completed {} hooks", total_hooks));
    }

    Ok(())
}
