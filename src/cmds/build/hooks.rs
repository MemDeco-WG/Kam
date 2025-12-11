use super::args::BuildArgs;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;
use crate::utils::Utils;

use indicatif::{ProgressBar, ProgressStyle};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, IsTerminal};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn run_pre_build_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    args: &BuildArgs,
) -> Result<(), KamError> {
    run_hooks(project_root, kam_toml, output_dir, "pre-build", args)
}

pub fn run_post_build_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    args: &BuildArgs,
) -> Result<(), KamError> {
    run_hooks(project_root, kam_toml, output_dir, "post-build", args)
}

fn run_hooks(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    stage: &str,
    args: &BuildArgs,
) -> Result<(), KamError> {
    // Do not run hooks when packaging a template archive
    if kam_toml.kam.module_type == ModuleType::Template {
        Utils::info(&format!("Skipping {} hooks for template packaging", stage));
        return Ok(());
    }

    // Load .env from project root if it exists and override existing env vars
    let env_path = project_root.join(".env");
    if env_path.exists() {
        // Read and parse .env file manually to allow overriding existing variables
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
                        Utils::warn(&format!(
                            "Warning: Invalid environment variable name '{}' at line {} in {}",
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

                    // Override existing environment variable
                    // SAFETY: We're setting environment variables in a controlled manner
                    // during the build process. This is safe as long as we're not running
                    // concurrent code that accesses these variables during modification.
                    unsafe {
                        std::env::set_var(key, value);
                    }
                } else if !line.is_empty() {
                    Utils::warn(&format!(
                        "Warning: Malformed line {} in {}: {}",
                        line_num + 1,
                        env_path.display(),
                        line
                    ));
                }
            }
        }
    }

    // Load template-provided env files (written by `kam init`) to seed initial env variables.
    // Candidate paths (order): .kam/template-vars.env (preferred) and template-vars.env (legacy).
    let mut template_envs: Vec<(String, String)> = Vec::new();
    let candidate_paths = [
        project_root.join(".kam").join("template-vars.env"),
        project_root.join("template-vars.env"),
    ];
    for p in candidate_paths.iter() {
        if p.exists() {
            if let Ok(content) = fs::read_to_string(p) {
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
                            Utils::warn(&format!(
                                "Warning: Invalid environment variable name '{}' at line {} in {}",
                                key,
                                line_num + 1,
                                p.display()
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

                        // Set the environment variable for the current process and remember it
                        unsafe {
                            std::env::set_var(key, value);
                        }
                        template_envs.push((key.to_string(), value.to_string()));
                    } else if !line.is_empty() {
                        Utils::warn(&format!(
                            "Warning: Malformed line {} in {}: {}",
                            line_num + 1,
                            p.display(),
                            line
                        ));
                    }
                }
            }
        }
    }

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

    // Determine repo and ref for KAM_REPO / KAM_REPO_REF
    let mut detected_repo = String::new();
    if let Ok(repo) = std::env::var("GITHUB_REPOSITORY") {
        detected_repo = repo;
    } else if !kam_toml
        .mmrl
        .as_ref()
        .and_then(|m| m.repo.as_ref())
        .and_then(|r| r.repository.as_ref())
        .unwrap_or(&String::new())
        .is_empty()
    {
        detected_repo = kam_toml
            .mmrl
            .as_ref()
            .and_then(|m| m.repo.as_ref())
            .and_then(|r| r.repository.as_ref())
            .unwrap_or(&String::new())
            .clone();
    }

    // Determine repo ref (branch) from environment or local git
    let mut detected_ref = String::new();
    if let Ok(github_ref) = std::env::var("GITHUB_REF") {
        // Trim refs/heads/ prefix when present
        detected_ref = github_ref
            .strip_prefix("refs/heads/")
            .unwrap_or(&github_ref)
            .to_string();
    } else {
        // Attempt to run git rev-parse --abbrev-ref HEAD
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

    // Build env var list and keep track of keys to avoid accidental duplicates.
    let mut env_vars: Vec<(String, String)> = Vec::new();
    let mut env_keys: HashSet<String> = HashSet::new();

    // Helper closure to insert an env var while preserving existing keys (precedence)
    let mut add_env = |k: &str, value: String| {
        if !env_keys.contains(k) {
            env_keys.insert(k.to_string());
            env_vars.push((k.to_string(), value));
        }
    };

    // Merge in any values read from template env files (these have precedence as initial values).
    // These come from `.kam/template-vars.env` or `template-vars.env` generated by `kam init`.
    for (k, v) in template_envs.iter() {
        add_env(k.as_str(), v.clone());
    }

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
    add_env("KAM_MODULE_AUTHOR", kam_toml.prop.author.clone());
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
    add_env("KAM_PROP_AUTHOR", kam_toml.prop.author.clone());
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

    // Execute hook files directly and let the OS determine execution behavior.
    // This runner intentionally avoids OS-specific wrappers or extension-based dispatch.
    // If a script cannot be executed on the current platform, it will fail and return an error.
    // We'll display a header after we've determined the total number of hooks

    let mut entries: Vec<_> = fs::read_dir(&hooks_dir)
        .map_err(KamError::Io)?
        .filter_map(|e| e.ok())
        .collect();

    // Sort by filename to ensure deterministic order (01-init.sh, 02-build.sh, etc.)
    entries.sort_by_key(|e| e.file_name());

    // Determine if we should show a progress bar
    let show_progress = !args.quiet && std::io::stdout().is_terminal();
    let total_hooks = entries.iter().filter(|e| e.path().is_file()).count();
    if !args.quiet {
        Utils::section(&format!(
            "Running {} hooks from {} ({} script(s))",
            stage,
            hooks_dir.display(),
            total_hooks
        ));
    }
    let pb = if show_progress && total_hooks > 0 {
        let pb = ProgressBar::new(total_hooks as u64);
        let style = ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
        )
        .unwrap()
        .progress_chars("#>-");
        pb.set_style(style);
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

            // Stream stdout/stderr so the progress bar can continue animating and we get line-by-line logs.
            // This avoids blocking the main thread via `Command::output()` and prints output as it's produced.
            // For error reporting we keep a tail of recent lines to include in the error message if the script fails.
            const MAX_TAIL_LINES: usize = 200;
            const MAX_DISPLAY_LEN: usize = 2048;

            let spawn_res = Command::new(&path)
                .current_dir(project_root)
                .envs(env_vars.iter().cloned())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();

            match spawn_res {
                Ok(mut child) => {
                    let stdout = child.stdout.take();
                    let stderr = child.stderr.take();

                    // Prepare buffers for last N lines to display on error
                    let stdout_tail: Arc<Mutex<VecDeque<String>>> =
                        Arc::new(Mutex::new(VecDeque::new()));
                    let stderr_tail: Arc<Mutex<VecDeque<String>>> =
                        Arc::new(Mutex::new(VecDeque::new()));

                    // Clone some values needed in threads
                    let pb_for_threads = pb.clone();
                    let stage_for_threads = stage.to_string();
                    let filename_for_threads = filename.to_string();
                    let idx_for_threads = idx;
                    let total_for_threads = total_hooks;

                    // stdout reader thread
                    let stdout_tail_clone = Arc::clone(&stdout_tail);
                    let pb_clone_stdout = pb_for_threads.clone();
                    let stage_clone_stdout = stage_for_threads.clone();
                    let filename_clone_stdout = filename_for_threads.clone();
                    let stdout_handle = if let Some(out) = stdout {
                        Some(std::thread::spawn(move || {
                            let reader = BufReader::new(out);
                            for line in reader.lines() {
                                if let Ok(l) = line {
                                    let mut lock = stdout_tail_clone.lock().unwrap();
                                    if lock.len() >= MAX_TAIL_LINES {
                                        lock.pop_front();
                                    }
                                    lock.push_back(l.clone());

                                    let formatted = Utils::format_cmd_line(&l);
                                    if let Some(pb) = &pb_clone_stdout {
                                        pb.println(&formatted);
                                        // Update progress message with truncated line
                                        let message = if l.len() > 80 {
                                            format!("{}...", &l[..77])
                                        } else {
                                            l.clone()
                                        };
                                        pb.set_message(format!(
                                            "[{} {}/{}] {} - {}",
                                            stage_clone_stdout,
                                            idx_for_threads,
                                            total_for_threads,
                                            filename_clone_stdout,
                                            message
                                        ));
                                    } else {
                                        Utils::print_cmd_line(&l);
                                    }
                                }
                            }
                        }))
                    } else {
                        None
                    };

                    // stderr reader thread
                    let stderr_tail_clone = Arc::clone(&stderr_tail);
                    let pb_clone_stderr = pb_for_threads.clone();
                    let stage_clone_err = stage_for_threads.clone();
                    let filename_clone_err = filename_for_threads.clone();
                    let stderr_handle = if let Some(err) = stderr {
                        Some(std::thread::spawn(move || {
                            let reader = BufReader::new(err);
                            for line in reader.lines() {
                                if let Ok(l) = line {
                                    let mut lock = stderr_tail_clone.lock().unwrap();
                                    if lock.len() >= MAX_TAIL_LINES {
                                        lock.pop_front();
                                    }
                                    lock.push_back(l.clone());

                                    let formatted = Utils::format_cmd_line(&l);
                                    if let Some(pb) = &pb_clone_stderr {
                                        pb.println(&formatted);
                                        let message = if l.len() > 80 {
                                            format!("{}...", &l[..77])
                                        } else {
                                            l.clone()
                                        };
                                        pb.set_message(format!(
                                            "[{} {}/{}] {} - {}",
                                            stage_clone_err,
                                            idx_for_threads,
                                            total_for_threads,
                                            filename_clone_err,
                                            message
                                        ));
                                    } else {
                                        Utils::print_cmd_line(&l);
                                    }
                                }
                            }
                        }))
                    } else {
                        None
                    };

                    // Spinner tick thread: keep pb animating while the child process runs
                    let ticker_running = Arc::new(AtomicBool::new(true));
                    let ticker_running_clone = ticker_running.clone();
                    let pb_for_tick = pb_for_threads.clone();
                    let ticker = std::thread::spawn(move || {
                        while ticker_running_clone.load(Ordering::Relaxed) {
                            if let Some(pb) = &pb_for_tick {
                                pb.tick();
                            }
                            std::thread::sleep(Duration::from_millis(80));
                        }
                    });

                    // Wait for child process to exit
                    let status_res = child.wait();

                    // Stop ticker and join threads
                    ticker_running.store(false, Ordering::Relaxed);
                    let _ = ticker.join();

                    if let Some(h) = stdout_handle {
                        let _ = h.join();
                    }
                    if let Some(h) = stderr_handle {
                        let _ = h.join();
                    }

                    match status_res {
                        Ok(status) => {
                            if !status.success() {
                                // Gather a limited tail to show in error
                                let stdout_gathered = {
                                    let lock = stdout_tail.lock().unwrap();
                                    lock.iter().cloned().collect::<Vec<_>>().join("\n")
                                };
                                let stderr_gathered = {
                                    let lock = stderr_tail.lock().unwrap();
                                    lock.iter().cloned().collect::<Vec<_>>().join("\n")
                                };

                                // Truncate if too long
                                let mut stdout_str = stdout_gathered;
                                let mut stderr_str = stderr_gathered;
                                if stdout_str.len() > MAX_DISPLAY_LEN {
                                    stdout_str.truncate(MAX_DISPLAY_LEN);
                                    stdout_str.push_str("... [truncated]");
                                }
                                if stderr_str.len() > MAX_DISPLAY_LEN {
                                    stderr_str.truncate(MAX_DISPLAY_LEN);
                                    stderr_str.push_str("... [truncated]");
                                }

                                if let Some(pb) = &pb {
                                    pb.finish_and_clear();
                                }

                                let status_code = status
                                    .code()
                                    .map(|c| c.to_string())
                                    .unwrap_or_else(|| status.to_string());
                                return Err(KamError::CommandFailed(format!(
                                    "Hook script {} failed with status: {}\nStdout:\n{}\nStderr:\n{}",
                                    filename, status_code, stdout_str, stderr_str
                                )));
                            }
                            // successful run - increment progress or print success line
                            if pb.is_none() {
                                Utils::success(&format!(
                                    "[{} {}/{}] {}",
                                    stage, idx, total_hooks, filename
                                ));
                            } else {
                                if let Some(pb) = &pb {
                                    pb.inc(1);
                                }
                            }
                        }
                        Err(e) => {
                            // wait() error
                            if let Some(pb) = &pb {
                                pb.finish_and_clear();
                            }
                            return Err(KamError::CommandFailed(format!(
                                "Failed to wait for hook {}: {}",
                                filename, e
                            )));
                        }
                    }
                }
                Err(e) => {
                    // Same hints as before (permission / not found)
                    match e.kind() {
                        std::io::ErrorKind::PermissionDenied => {
                            Utils::warn(
                                "Permission denied. Make sure the script is executable and accessible. On Unix, you may need to run: chmod +x <file>. On Windows, ensure the script association or runtime is available (or run via WSL/Git Bash).",
                            );
                        }
                        std::io::ErrorKind::NotFound => {
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
            // Increment and update progress bar on successful run
            if let Some(pb) = &pb {
                pb.inc(1);
            }
        }
    }

    // Finish the progress bar if shown
    if let Some(pb) = &pb {
        pb.finish_with_message("Done");
    }

    Ok(())
}
