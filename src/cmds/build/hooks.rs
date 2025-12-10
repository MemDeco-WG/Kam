use super::args::BuildArgs;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;
use crate::utils::Utils;

use std::fs;
use std::path::Path;
use std::process::Command;
use std::io::IsTerminal;
use indicatif::{ProgressBar, ProgressStyle};

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

    let env_vars = [
        (
            "KAM_PROJECT_ROOT",
            project_root.to_string_lossy().to_string(),
        ),
        ("KAM_HOOKS_ROOT", hooks_root.to_string_lossy().to_string()),
        ("KAM_MODULE_ROOT", module_root.to_string_lossy().to_string()),
        ("KAM_WEB_ROOT", web_root.to_string_lossy().to_string()),
        ("KAM_DIST_DIR", output_dir.to_string_lossy().to_string()),
        ("KAM_MODULE_ID", kam_toml.prop.id.clone()),
        ("KAM_MODULE_VERSION", kam_toml.prop.version.clone()),
        (
            "KAM_MODULE_VERSION_CODE",
            kam_toml.prop.versionCode.to_string(),
        ),
        ("KAM_MODULE_NAME", kam_toml.prop.get_name().to_string()),
        ("KAM_MODULE_AUTHOR", kam_toml.prop.author.clone()),
        (
            "KAM_MODULE_DESCRIPTION",
            kam_toml.prop.get_description().to_string(),
        ),
        (
            "KAM_MODULE_UPDATE_JSON",
            kam_toml
                .prop
                .updateJson
                .as_ref()
                .unwrap_or(&String::new())
                .clone(),
        ),
        ("KAM_STAGE", stage.to_string()),
        (
            "KAM_BUMP_ENABLED",
            if args.bump { "1" } else { "0" }.to_string(),
        ),
        (
            "KAM_RELEASE_ENABLED",
            if args.release { "1" } else { "0" }.to_string(),
        ),
        (
            "KAM_SIGN_ENABLE",
            if args.sign { "1" } else { "0" }.to_string(),
        ),
        (
            "KAM_SIGN_ENABLED",
            if args.sign { "1" } else { "0" }.to_string(),
        ),
        (
            "KAM_PRE_RELEASE",
            if args.pre_release { "1" } else { "0" }.to_string(),
        ),
        (
            "KAM_INTERACTIVE",
            if args.interactive { "1" } else { "0" }.to_string(),
        ),
        (
            "KAM_GIT_REPO",
            kam_toml
                .mmrl
                .as_ref()
                .and_then(|m| m.repo.as_ref())
                .and_then(|r| r.repository.as_ref())
                .unwrap_or(&String::new())
                .clone(),
        ),
            ("KAM_GITHUB_REPO", detected_repo.clone()),
            ("KAM_REPO", detected_repo.clone()),
            ("KAM_REPO_REF", detected_ref.clone()),
            ("KAM_RELEASE_TAG", kam_toml.prop.version.clone()),
    ];

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

            // Capture stdout/stderr to provide more detailed and actionable errors when
            // a hook fails. We intentionally avoid platform-specific interpreter selection
            // — the hook runner invokes the file and defers to the OS to decide how to run it.
            let output = Command::new(&path)
                .current_dir(project_root)
                .envs(env_vars.iter().cloned())
                .output();

            match output {
                Ok(out) => {
                    // Print structured stdout/stderr from the executed command to surface
                    // any non-fatal messages (e.g. `[WARN] gh release create ...`) even
                    // when the exit code is zero. This helps users quickly spot warnings.
                    Utils::print_cmd_output(&out.stdout, &out.stderr);

                    if !out.status.success() {
                        // Limit captured output to avoid extremely large messages
                        let mut stdout = String::from_utf8_lossy(&out.stdout).to_string();
                        let mut stderr = String::from_utf8_lossy(&out.stderr).to_string();
                        const MAX_LEN: usize = 2048;
                        if stdout.len() > MAX_LEN {
                            stdout.truncate(MAX_LEN);
                            stdout.push_str("... [truncated]");
                        }
                        if stderr.len() > MAX_LEN {
                            stderr.truncate(MAX_LEN);
                            stderr.push_str("... [truncated]");
                        }

                        // Build readable status string
                        let status_code = out
                            .status
                            .code()
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| out.status.to_string());

                        if let Some(pb) = &pb {
                            pb.finish_and_clear();
                        }
                        return Err(KamError::CommandFailed(format!(
                            "Hook script {} failed with status: {}\nStdout:\n{}\nStderr:\n{}",
                            filename, status_code, stdout, stderr
                        )));
                    }
                    // For non-interactive output (no progress bar), print a success line per hook for clarity
                    if pb.is_none() {
                        Utils::success(&format!("[{} {}/{}] {}", stage, idx, total_hooks, filename));
                    }
                }
                Err(e) => {
                    // Provide a cross-platform hint about permission, missing runtime, or execution issues.
                    // We intentionally don't decide the platform; just provide helpful hints so users can
                    // address common runtime/permission issues.
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

