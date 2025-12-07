use super::args::BuildArgs;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
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
    ];

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

            // On Unix, skip .ps1 files
            #[cfg(unix)]
            if path.extension().and_then(|s| s.to_str()) == Some("ps1") {
                continue;
            }

            println!("  {} Executing {}", "→".blue(), filename);

            #[cfg(unix)]
            let status = Command::new(&path)
                .current_dir(project_root)
                .envs(env_vars.iter().cloned())
                .status();

            #[cfg(windows)]
            let status = {
                // Simple extension check for Windows execution
                let ext = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                match ext.as_str() {
                    "ps1" => Command::new("powershell")
                        .arg("-ExecutionPolicy")
                        .arg("Bypass")
                        .arg("-File")
                        .arg(&path)
                        .current_dir(project_root)
                        .envs(env_vars.iter().cloned())
                        .status(),
                    "bat" | "cmd" => Command::new("cmd")
                        .arg("/C")
                        .arg(&path)
                        .current_dir(project_root)
                        .envs(env_vars.iter().cloned())
                        .status(),
                    // Try direct execution for .exe or if file association works
                    _ => Command::new(&path)
                        .current_dir(project_root)
                        .envs(env_vars.iter().cloned())
                        .status(),
                }
            };

            match status {
                Ok(s) => {
                    if !s.success() {
                        return Err(KamError::CommandFailed(format!(
                            "Hook script {} failed with status: {}",
                            filename, s
                        )));
                    }
                }
                Err(e) => {
                    // If permission denied on Unix, hint about chmod +x
                    #[cfg(unix)]
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        eprintln!(
                            "  {} Permission denied. Make sure the script is executable (chmod +x).",
                            "!".yellow()
                        );
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
