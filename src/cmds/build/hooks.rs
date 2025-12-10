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
            "KAM_IMMUTABLE_RELEASE",
            if args.immutable_release { "1" } else { "0" }.to_string(),
        ),
        (
            "KAM_PRE_RELEASE",
            if args.pre_release { "1" } else { "0" }.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmds::build::args::BuildArgs;
    use crate::types::kam_toml::KamToml;
    use crate::types::kam_toml::enums::ModuleType;
    use std::fs;
    use tempfile::tempdir;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn test_build_flags_set_envs_for_hooks() {
        // Prepare temp project dir
        let tmp = tempdir().expect("tempdir");
        let project_path = tmp.path();

        // Create a minimalkam toml
        let kt = KamToml::new_with_current_timestamp(
            "test.module".to_string(),
            "Test Module".to_string(),
            "0.1.0".to_string(),
            "author".to_string(),
            "desc".to_string(),
            None,
            Some(ModuleType::Kam),
        );
        kt.write_to_dir(project_path).expect("write kam.toml");

        // create hooks/pre-build folder and a script that writes envs to file
        let hooks_pre_dir = project_path.join("hooks").join("pre-build");
        fs::create_dir_all(&hooks_pre_dir).expect("create hooks dir");
        let script_path = hooks_pre_dir.join("01-env-test.sh");
        let script = r#"#!/bin/sh
echo KAM_SIGN_ENABLE=$KAM_SIGN_ENABLE > hook_out.txt
echo KAM_IMMUTABLE_RELEASE=$KAM_IMMUTABLE_RELEASE >> hook_out.txt
echo KAM_PRE_RELEASE=$KAM_PRE_RELEASE >> hook_out.txt
"#;
        fs::write(&script_path, script).expect("write script");
        // make executable
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("chmod");

        // prepare build args
        let args = BuildArgs {
            path: project_path.to_string_lossy().to_string(),
            all: false,
            output: None,
            bump: false,
            release: false,
            quiet: true,
            sign: true,
            immutable_release: true,
            pre_release: true,
        };

        // run pre build hooks
        let out_dir = project_path.join("dist");
        fs::create_dir_all(&out_dir).expect("create dist");

        let kt2 = KamToml::load_from_dir(project_path).expect("load kam toml");
        run_pre_build_hooks(project_path, &kt2, &out_dir, &args).expect("run hooks");

        // read output file and verify values
        let out_file = project_path.join("hook_out.txt");
        let contents = fs::read_to_string(out_file).expect("read out");
        assert!(contents.contains("KAM_SIGN_ENABLE=1"));
        assert!(contents.contains("KAM_IMMUTABLE_RELEASE=1"));
        assert!(contents.contains("KAM_PRE_RELEASE=1"));
    }

    #[test]
    fn test_post_build_sign_and_upload_hooks() {
        use super::*;
        use tempfile::tempdir;
        use std::io::Write;

        let tmp = tempdir().expect("tempdir");
        let project_path = tmp.path();
        let output_dir = project_path.join("dist");
        std::fs::create_dir_all(&output_dir).expect("create dist");

        // Create a fake artifact
        let artifact = output_dir.join("test.zip");
        let mut af = std::fs::File::create(&artifact).unwrap();
        af.write_all(b"dummy").unwrap();

        // Create post-build hooks folder
        let hooks_dir = project_path.join("hooks").join("post-build");
        std::fs::create_dir_all(&hooks_dir).unwrap();

        // Create a sign hook that writes env info
        let sign_script = hooks_dir.join("8000.SIGN_IF_ENABLE.sh");
        let sign_script_content = r#"#!/bin/sh
echo "SIG: $KAM_SIGN_ENABLE" > hook_out.txt
if [ "$KAM_SIGN_ENABLE" = "1" ]; then
    echo "SIGNED $KAM_DIST_DIR/test.zip" >> hook_out.txt
fi
"#;
        std::fs::write(&sign_script, sign_script_content).unwrap();
        let mut perms = std::fs::metadata(&sign_script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&sign_script, perms).unwrap();

        // Create upload hook that writes release-related env info
        let upload_script = hooks_dir.join("9000.UPLOAD_IF_ENABLED.sh");
        let upload_script_content = r#"#!/bin/sh
echo "REL: $KAM_RELEASE_ENABLED" > hook_out_upload.txt
echo "PRE: $KAM_PRE_RELEASE" >> hook_out_upload.txt
echo "IMM: $KAM_IMMUTABLE_RELEASE" >> hook_out_upload.txt
"#;
        std::fs::write(&upload_script, upload_script_content).unwrap();
        let mut perms2 = std::fs::metadata(&upload_script).unwrap().permissions();
        perms2.set_mode(0o755);
        std::fs::set_permissions(&upload_script, perms2).unwrap();

        // Prepare minimal KamToml
        let kt = crate::types::kam_toml::KamToml::new_with_current_timestamp(
            "test.module".into(),
            "Test Module".into(),
            "0.1.0".into(),
            "author".into(),
            "desc".into(),
            None,
            None,
        );

        let args = BuildArgs {
            path: project_path.to_string_lossy().to_string(),
            all: false,
            output: None,
            bump: false,
            release: true,
            quiet: true,
            sign: true,
            immutable_release: false,
            pre_release: true,
        };

        // Run hooks
        run_post_build_hooks(project_path, &kt, &output_dir, &args).expect("run hooks");

        // read hook outputs
        let contents = std::fs::read_to_string(project_path.join("hook_out.txt")).unwrap();
        assert!(contents.contains("SIG: 1"));
        assert!(contents.contains("SIGNED"));

        let contents2 = std::fs::read_to_string(project_path.join("hook_out_upload.txt")).unwrap();
        assert!(contents2.contains("REL: 1"));
        assert!(contents2.contains("PRE: 1"));
        assert!(contents2.contains("IMM: 0"));
    }
}
