use super::args::BuildArgs;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;
use colored::*;
use std::fs;
use std::path::Path;
use std::process::Command;

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
        println!(
            "  {} Skipping {} hooks for template packaging",
            "•".cyan(),
            stage
        );
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
                        eprintln!(
                            "Warning: Invalid environment variable name '{}' at line {} in {}",
                            key,
                            line_num + 1,
                            env_path.display()
                        );
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
                    eprintln!(
                        "Warning: Malformed line {} in {}: {}",
                        line_num + 1,
                        env_path.display(),
                        line
                    );
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
            "KAM_GIT_REPO",
            kam_toml
                .mmrl
                .as_ref()
                .and_then(|m| m.repo.as_ref())
                .and_then(|r| r.repository.as_ref())
                .unwrap_or(&String::new())
                .clone(),
        ),
    ];

    // Execute hook files directly and let the OS determine execution behavior.
    // This runner intentionally avoids OS-specific wrappers or extension-based dispatch.
    // If a script cannot be executed on the current platform, it will fail and return an error.
    println!(
        "{} Running {} hooks from {}",
        "•".cyan(),
        stage,
        hooks_dir.display().to_string().dimmed()
    );

    let mut entries: Vec<_> = fs::read_dir(&hooks_dir)
        .map_err(KamError::Io)?
        .filter_map(|e| e.ok())
        .collect();

    // Sort by filename to ensure deterministic order (01-init.sh, 02-build.sh, etc.)
    entries.sort_by_key(|e| e.file_name());

    // Iterate hooks in deterministic order and execute each non-hidden file directly.
    // The hook runner doesn't attempt to interpret file extensions or choose a runtime;
    // it simply invokes the file and defers to the platform to handle the execution.
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

            println!("  {} Executing {}", "→".blue(), filename);

            // Capture stdout/stderr to provide more detailed and actionable errors when
            // a hook fails. We intentionally avoid platform-specific interpreter selection
            // — the hook runner invokes the file and defers to the OS to decide how to run it.
            let output = Command::new(&path)
                .current_dir(project_root)
                .envs(env_vars.iter().cloned())
                .output();

            match output {
                Ok(out) => {
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

                        return Err(KamError::CommandFailed(format!(
                            "Hook script {} failed with status: {}\nStdout:\n{}\nStderr:\n{}",
                            filename, status_code, stdout, stderr
                        )));
                    }
                }
                Err(e) => {
                    // Provide a cross-platform hint about permission, missing runtime, or execution issues.
                    // We intentionally don't decide the platform; just provide helpful hints so users can
                    // address common runtime/permission issues.
                    match e.kind() {
                        std::io::ErrorKind::PermissionDenied => {
                            eprintln!(
                                "  {} Permission denied. Make sure the script is executable and accessible. On Unix, you may need to run: chmod +x <file>. On Windows, ensure the script association or runtime is available (or run via WSL/Git Bash).",
                                "!".yellow()
                            );
                        }
                        std::io::ErrorKind::NotFound => {
                            eprintln!(
                                "  {} Not found. Could not execute {}. Ensure the script has an interpreter or runtime available on the system (e.g., `sh`, `bash`, or `pwsh`), or invoke the script via a shell that is available on your platform.",
                                "!".yellow(),
                                filename
                            );
                        }
                        _ => {}
                    }
                    return Err(KamError::CommandFailed(format!(
                        "Failed to execute hook {}: {}",
                        filename, e
                    )));
                }
            }
        }
    }

    Ok(())
}
