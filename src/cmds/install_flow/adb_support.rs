fn clone_repo_to_tempdir(spec: &str) -> Result<(TempDir, PathBuf), KamError> {
    let tmp = create_tempdir_with_fallback().map_err(KamError::Io)?;
    // We explicitly create a subdirectory to avoid ambiguity with how some CLIs behave.
    let dest = tmp.path().join("repo");
    // Try gh first
    if crate::utils::command_exists("gh") {
        // Allow +git / +gh prefixes to be used when invoking the command by stripping them.
        let gh_spec = spec
            .trim()
            .trim_start_matches("+git")
            .trim_start_matches("+gh")
            .trim_start_matches('+')
            .trim_start_matches(':');
        Utils::info(format!(
            "Cloning '{gh_spec}' using 'gh' into: {}",
            dest.display()
        ));
        let dest_str = dest
            .to_str()
            .map_or_else(|| dest.to_string_lossy().into_owned(), ToString::to_string);
        let mut cmd = Command::new("gh");
        cmd.arg("repo").arg("clone").arg(gh_spec).arg(&dest_str);
        cmd.stdin(Stdio::inherit());
        match Utils::run_and_stream_no_stderr_header(cmd) {
            Ok(status) if status.success() => return Ok((tmp, dest)),
            Ok(status) => {
                Utils::warn(format!("'gh' clone failed with status: {status:?}"));
                // fallthrough to git
            }
            Err(e) => {
                Utils::warn(format!("'gh' clone failed: {e}"));
                // fallthrough to git
            }
        }
    }

    // Fallback to git
    if crate::utils::command_exists("git") {
        let url = expand_git_shorthand(spec);
        Utils::info(format!(
            "Cloning '{url}' using 'git' into: {}",
            dest.display()
        ));
        let dest_str = dest
            .to_str()
            .map_or_else(|| dest.to_string_lossy().into_owned(), ToString::to_string);
        let mut cmd = Command::new("git");
        cmd.arg("clone").arg(&url).arg(&dest_str);
        cmd.stdin(Stdio::inherit());
        match Utils::run_and_stream_no_stderr_header(cmd) {
            Ok(status) if status.success() => return Ok((tmp, dest)),
            Ok(status) => {
                return Err(KamError::CommandFailed(format!(
                    "git clone failed with status: {status:?}"
                )));
            }
            Err(e) => return Err(KamError::Io(e)),
        }
    }

    Err(KamError::CommandFailed(
        "Neither 'gh' (GitHub CLI) nor 'git' found on PATH. Please install one to clone repositories."
            .to_string(),
    ))
}

/// Clone, optionally run the repository's `kam.sh` (after review), build (if needed),
/// and return the path to the produced artifact in the temporary checkout.
/// The returned `TempDir` must be kept alive by the caller until install completes.
#[allow(clippy::too_many_lines)] // TODO: split into smaller helper functions
fn handle_git_install(spec: &str, args: &InstallArgs) -> Result<(PathBuf, TempDir), KamError> {
    // Clone repo
    let (tmpdir, workdir) = clone_repo_to_tempdir(spec)?;

    // Switch to repository directory while performing build/script steps.
    let orig_cwd = env::current_dir().map_err(KamError::Io)?;
    env::set_current_dir(&workdir).map_err(KamError::Io)?;

    // Inner work happening inside the cloned repo. We'll restore cwd afterwards.
    let inner_res: Result<PathBuf, KamError> = (|| -> Result<PathBuf, KamError> {
        let kam_sh = workdir.join("kam.sh");
        if kam_sh.exists() && kam_sh.is_file() {
            // Present the file to the user for review
            if let Ok(content) = fs::read_to_string(&kam_sh) {
                Utils::section("Preview: kam.sh");
                println!("{content}");
                let assume_yes = std::env::args().any(|a| a == "-y" || a == "--yes");
                let run_script = if assume_yes {
                    true
                } else {
                    Confirm::new()
                        .with_prompt("Execute 'kam.sh' from cloned repository?")
                        .default(false)
                        .interact()
                        .map_err(|e| KamError::Io(e.into()))?
                };

                if run_script {
                    // Determine interpreter (prefer bash if shebang mentions it)
                    let interpreter = content
                        .lines()
                        .next()
                        .and_then(|l| l.strip_prefix("#!").map(ToString::to_string))
                        .unwrap_or_else(|| "sh".to_string());
                    // Use a simple heuristic: if shebang contains 'bash' prefer 'bash', otherwise fall back to 'sh'
                    let exec = if interpreter.contains("bash") {
                        "bash"
                    } else {
                        "sh"
                    };
                    Utils::info(format!("Executing '{exec}' {}", kam_sh.display()));
                    let kam_sh_str = kam_sh.to_str().map_or_else(
                        || kam_sh.to_string_lossy().into_owned(),
                        ToString::to_string,
                    );
                    let mut cmd = Command::new(exec);
                    cmd.arg(&kam_sh_str).stdin(Stdio::inherit());
                    let status =
                        Utils::run_and_stream_no_stderr_header(cmd).map_err(KamError::Io)?;
                    if !status.success() {
                        return Err(KamError::CommandFailed(format!(
                            "'kam.sh' execution failed with status: {status:?}"
                        )));
                    }
                    // Assume the script produced whatever it needed (dist/artifacts). We'll still check.
                } else {
                    Utils::info("Not executing 'kam.sh'. Offering to run 'kam build' instead.");
                    let assume_yes = std::env::args().any(|a| a == "-y" || a == "--yes");
                    let run_build = if assume_yes {
                        true
                    } else {
                        Confirm::new()
                            .with_prompt("Run 'kam build' in the repository to produce artifacts?")
                            .default(true)
                            .interact()
                            .map_err(|e| KamError::Io(e.into()))?
                    };
                    if !run_build {
                        return Err(KamError::CommandFailed(
                            "Aborted by user: did not run 'kam.sh' and declined to build."
                                .to_string(),
                        ));
                    }
                    // else fallthrough to build step below
                }
            } else {
                Utils::warn("Failed to read 'kam.sh' - proceeding to build step if possible");
            }
        }

        // Load kam.toml (failure is a meaningful error)
        let kt = KamToml::load_from_dir(&workdir)?;

        // Helper to find artifact after any build/script step
        let find_artifact = |kt_ref: &KamToml| -> Result<PathBuf, KamError> {
            // target_dir may be relative (default 'dist')
            let target_dir = kt_ref
                .kam
                .build
                .as_ref()
                .and_then(|b| b.target_dir.as_deref())
                .unwrap_or("dist");
            let basename = crate::cmds::build::build_project::determine_basename(kt_ref)?;
            let candidate = workdir.join(target_dir).join(format!("{basename}.zip"));
            if candidate.exists() && candidate.is_file() {
                return Ok(candidate.canonicalize().unwrap_or(candidate));
            }

            // Fallback: scan workdir and workdir/target_dir for latest zip
            let mut candidates: Vec<PathBuf> = Vec::new();
            for dir in &[workdir.join("."), workdir.join(target_dir)] {
                if dir.exists() && dir.is_dir() {
                    for entry in fs::read_dir(dir).map_err(KamError::Io)? {
                        let e = entry.map_err(KamError::Io)?;
                        let p = e.path();
                        if p.is_file()
                            && let Some(ext) = p.extension().and_then(|x| x.to_str())
                            && ext.eq_ignore_ascii_case("zip")
                        {
                            candidates.push(p);
                        }
                    }
                }
            }
            if candidates.is_empty() {
                return Err(KamError::PackageNotFound(
                    "No built artifact (.zip) found after building the repository".to_string(),
                ));
            }
            let latest = candidates
                .into_iter()
                .max_by_key(|p| {
                    fs::metadata(p)
                        .and_then(|m| m.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH)
                })
                .ok_or_else(|| {
                    KamError::PackageNotFound(
                        "No built artifact (.zip) found after building the repository".to_string(),
                    )
                })?;
            Ok(latest.canonicalize().unwrap_or(latest))
        };

        // If the artifact is already present (e.g., produced by kam.sh), return it
        if let Ok(artifact) = find_artifact(&kt) {
            return Ok(artifact);
        }

        // Otherwise run a build
        let build_args = BuildArgs {
            path: ".".to_string(),
            all: false,
            output: None,
            bump: false,
            release: false,
            sign: false,
            interactive: false,
            pre_release: false,
            quiet: args.quiet,
            jobs: None,
        };
        crate::cmds::build::build_project::build_project(&workdir, &build_args, Some(kt.clone()))?;

        // After build try to find the artifact again
        let artifact = find_artifact(&kt)?;
        Ok(artifact)
    })();

    // Restore original cwd regardless of inner result
    env::set_current_dir(&orig_cwd).map_err(KamError::Io)?;

    // Return either the found artifact (keeping tmpdir alive) or propagate the error
    match inner_res {
        Ok(a) => Ok((a, tmpdir)),
        Err(e) => Err(e),
    }
}

/// 判断是否为权限相关错误
fn is_permission_error(output: &str) -> bool {
    let output_lower = output.to_lowercase();
    output_lower.contains("permission denied")
        || output_lower.contains("access denied")
        || output_lower.contains("operation not permitted")
        || output_lower.contains("sudo") && output_lower.contains("required")
}

/// 判断是否为命令未找到错误
fn is_command_not_found_error(output: &str) -> bool {
    let output_lower = output.to_lowercase();
    output_lower.contains("not found")
        || output_lower.contains("command not found")
        || output_lower.contains("no such file or directory")
}

fn quote_shell_arg(arg: &str) -> String {
    if arg.contains('\'') {
        let escaped = arg.replace('\'', "'\"'\"'");
        format!("'{escaped}'")
    } else {
        format!("'{arg}'")
    }
}

fn shell_command(cli_bin: &str, cli_args: &[String]) -> String {
    std::iter::once(cli_bin.to_string())
        .chain(cli_args.iter().cloned())
        .map(|s| quote_shell_arg(&s))
        .collect::<Vec<_>>()
        .join(" ")
}

const ADB_REMOTE_PACKAGE_DIR: &str = "/sdcard/kam/tmp";

fn adb_remote_package_dir() -> &'static str {
    ADB_REMOTE_PACKAGE_DIR
}

fn adb_remote_path(artifact: &Path) -> Result<String, KamError> {
    let file_name = artifact
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            KamError::PackageNotFound(format!(
                "Unable to derive remote package name from {}",
                artifact.display()
            ))
        })?;
    Ok(format!("{ADB_REMOTE_PACKAGE_DIR}/{file_name}"))
}

fn run_status(mut cmd: Command, verbose: bool) -> Result<std::process::ExitStatus, KamError> {
    if verbose {
        cmd.stdin(Stdio::inherit());
        Utils::run_and_stream_no_stderr_header(cmd).map_err(KamError::Io)
    } else {
        cmd.status().map_err(KamError::Io)
    }
}

fn run_output(mut cmd: Command) -> Result<Output, KamError> {
    cmd.output().map_err(KamError::Io)
}

fn adb_shell_su_output(shell_cmd: &str) -> Result<Output, KamError> {
    let mut cmd = Command::new("adb");
    cmd.arg("shell").arg("su").arg("-c").arg(shell_cmd);
    run_output(cmd)
}

fn detect_adb_root_manager() -> Result<String, KamError> {
    for (manager, cli_bin) in [
        ("KernelSU", "ksud"),
        ("Magisk", "magisk"),
        ("APatchSU", "apd"),
    ] {
        let probe = format!("command -v {cli_bin} >/dev/null 2>&1");
        let out = adb_shell_su_output(&probe)?;
        if out.status.success() {
            return Ok(manager.to_string());
        }
    }
    Err(KamError::CommandFailed(
        "Unable to auto-detect root manager on device via adb. Pass --manager Magisk, KernelSU, or APatchSU.".to_string(),
    ))
}

fn resolve_adb_root_manager(manager_override: Option<&str>) -> Result<String, KamError> {
    let requested = manager_override.map_or_else(get_root_manager, normalize_root_manager_or_auto);
    match requested.as_str() {
        "Auto" => detect_adb_root_manager(),
        "Unknown" => Err(KamError::CommandFailed(crate::i18n::tr(
            "install.unable_to_determine",
        ))),
        manager => Ok(manager.to_string()),
    }
}

fn install_cli_for_manager_name(
    manager: &str,
    package_path: &str,
) -> Result<(String, Vec<String>), KamError> {
    match manager {
        "Magisk" => Ok((
            "magisk".to_string(),
            vec!["--install-module".to_string(), package_path.to_string()],
        )),
        "KernelSU" => Ok((
            "ksud".to_string(),
            vec![
                "module".to_string(),
                "install".to_string(),
                package_path.to_string(),
            ],
        )),
        "APatchSU" => Ok((
            "apd".to_string(),
            vec![
                "module".to_string(),
                "install".to_string(),
                package_path.to_string(),
            ],
        )),
        _ => Err(KamError::CommandFailed(crate::i18n::tr(
            "install.unable_to_determine",
        ))),
    }
}
