#[allow(clippy::too_many_lines)]
fn execute_adb_install_from_artifact(artifact: &Path, args: &InstallArgs) -> Result<(), KamError> {
    if !crate::utils::command_exists("adb") {
        return Err(KamError::CommandFailed(
            "adb not found on PATH. Install Android platform-tools or disable --adb.".to_string(),
        ));
    }

    let remote_path = adb_remote_path(artifact)?;

    if args.dry_run {
        let manager = args
            .manager
            .as_deref()
            .map_or_else(get_root_manager, normalize_root_manager_or_auto);
        let remote_install_cmd = if manager == "Auto" {
            "<auto-detected root manager install command>".to_string()
        } else {
            let (cli_bin, cli_args) = install_cli_for_manager_name(&manager, &remote_path)?;
            shell_command(&cli_bin, &cli_args)
        };
        if args.quiet {
            println!("adb push {} {}", artifact.display(), remote_path);
            println!("adb shell su -c {}", quote_shell_arg(&remote_install_cmd));
        } else {
            Utils::info(format!(
                "Dry run: will execute 'adb push {} {}'",
                artifact.display(),
                remote_path
            ));
            Utils::info(format!(
                "Dry run: will execute 'adb shell su -c {}'",
                quote_shell_arg(&remote_install_cmd)
            ));
        }
        return Ok(());
    }

    let manager = resolve_adb_root_manager(args.manager.as_deref())?;
    let (cli_bin, cli_args) = install_cli_for_manager_name(&manager, &remote_path)?;
    let remote_install_cmd = shell_command(&cli_bin, &cli_args);

    if !args.quiet {
        Utils::info(format!(
            "Pushing module to device: {} -> {remote_path}",
            artifact.display()
        ));
    }
    let mkdir_status = run_status(
        {
            let mut cmd = Command::new("adb");
            cmd.arg("shell")
                .arg("mkdir")
                .arg("-p")
                .arg(adb_remote_package_dir());
            cmd
        },
        args.verbose,
    )?;
    if !mkdir_status.success() {
        return Err(KamError::CommandFailed(format!(
            "adb shell mkdir failed with status: {mkdir_status}"
        )));
    }

    let push_status = run_status(
        {
            let mut cmd = Command::new("adb");
            cmd.arg("push").arg(artifact).arg(&remote_path);
            cmd
        },
        args.verbose,
    )?;
    if !push_status.success() {
        return Err(KamError::CommandFailed(format!(
            "adb push failed with status: {push_status}"
        )));
    }

    if !args.quiet {
        Utils::info(format!(
            "Installing on device via adb shell su -c {}",
            quote_shell_arg(&remote_install_cmd)
        ));
    }
    let install_status = run_status(
        {
            let mut cmd = Command::new("adb");
            cmd.arg("shell")
                .arg("su")
                .arg("-c")
                .arg(&remote_install_cmd);
            cmd
        },
        true,
    )?;
    if install_status.success() {
        if !args.quiet {
            Utils::success(format!(
                "Installed {} on connected device via adb",
                artifact.display()
            ));
        }
        Ok(())
    } else {
        Err(KamError::CommandFailed(format!(
            "adb shell install failed with status: {install_status}. If multiple devices are connected, set ANDROID_SERIAL or use adb -s outside Kam."
        )))
    }
}

/// Perform the actual install once we have an artifact path. Extracted from the
/// original `run` implementation so both local and git-based flows can share it.
#[allow(clippy::too_many_lines)] // TODO: split into smaller helper functions
fn execute_install_from_artifact(artifact: &Path, args: &InstallArgs) -> Result<(), KamError> {
    if !args.quiet {
        Utils::section(&trf!("install.section", artifact.display()));
    }

    if args.adb {
        return execute_adb_install_from_artifact(artifact, args);
    }

    let (cli_bin, cli_args) = get_install_cli_for_manager(artifact, args.manager.as_deref())?;

    if args.dry_run {
        if args.quiet {
            println!("{} {}", cli_bin, cli_args.join(" "));
        } else {
            Utils::info(&trf!(
                "Dry run: will execute '{} {}'",
                cli_bin,
                cli_args.join(" ")
            ));
        }
        return Ok(());
    }

    if !args.quiet {
        Utils::info(&trf!("install.executing", cli_bin, cli_args.join(" ")));
    }

    // Stream the child process output live when verbose was requested.
    if args.verbose {
        let mut cmd = Command::new(&cli_bin);
        cmd.args(&cli_args);
        // Keep stdin inherited so interactive commands still work
        cmd.stdin(Stdio::inherit());
        match Utils::run_and_stream_no_stderr_header(cmd) {
            Ok(status) => {
                if status.success() {
                    if !args.quiet {
                        Utils::success(&trf!("install.installed", artifact.display(), cli_bin));
                    }
                    return Ok(());
                }

                // 对于 verbose 模式，我们需要更智能地判断错误类型
                // 由于 streaming 模式下我们无法捕获完整的输出，我们基于退出码和命令可用性来判断
                let code = status.code();
                let should_try_su = crate::utils::command_exists("su")
                    && ((code == Some(126) || code == Some(127)) // cannot execute or not found
                        || !crate::utils::command_exists(&cli_bin)); // CLI binary doesn't exist

                if should_try_su {
                    let cmd_str = shell_command(&cli_bin, &cli_args);
                    if !args.quiet {
                        Utils::info(&trf!("Attempting to execute via 'su -c': {}", cmd_str));
                    }
                    let mut su_cmd = Command::new("su");
                    su_cmd.arg("-c").arg(cmd_str).stdin(Stdio::inherit());
                    match Utils::run_and_stream_no_stderr_header(su_cmd) {
                        Ok(su_status) => {
                            if su_status.success() {
                                if !args.quiet {
                                    Utils::success(&trf!(
                                        "Installed {} via {}",
                                        artifact.display(),
                                        cli_bin
                                    ));
                                }
                                return Ok(());
                            }
                            return Err(KamError::CommandFailed(format!(
                                "Privilege escalation via 'su' failed with status: {su_status:?}"
                            )));
                        }
                        Err(e) => return Err(KamError::Io(e)),
                    }
                }

                // 退出代码为1直接报错
                if code == Some(1) {
                    return Err(KamError::CommandFailed(
                        "安装失败，检查安装脚本或者检查root授权".to_string(),
                    ));
                }

                // 其他退出代码提供详细信息
                let error_msg = format!(
                    "Install command '{cli_bin}' exited with status: {status}. Check the output above for details."
                );
                return Err(KamError::CommandFailed(error_msg));
            }
            Err(e) => return Err(KamError::Io(e)),
        }
    }

    match Command::new(&cli_bin).args(&cli_args).output() {
        Ok(out) => {
            if args.verbose {
                Utils::print_cmd_output(&out.stdout, &out.stderr);
            }
            if out.status.success() {
                if !args.quiet {
                    Utils::success(&trf!("install.installed", artifact.display(), cli_bin));
                }
                Ok(())
            } else {
                let s_out = String::from_utf8_lossy(&out.stdout);
                let s_err = String::from_utf8_lossy(&out.stderr);
                let combined = format!("{s_out}{s_err}");

                // 只有在真正的权限错误时才尝试使用 su
                if is_permission_error(&combined) && crate::utils::command_exists("su") {
                    let cmd_str = shell_command(&cli_bin, &cli_args);
                    if !args.quiet {
                        Utils::info(&trf!("Attempting to execute via 'su -c': {}", cmd_str));
                    }
                    let mut su_cmd = Command::new("su");
                    su_cmd.arg("-c").arg(cmd_str).stdin(Stdio::inherit());
                    match Utils::run_and_stream_no_stderr_header(su_cmd) {
                        Ok(status) => {
                            if status.success() {
                                if !args.quiet {
                                    Utils::success(&trf!(
                                        "Installed {} via {}",
                                        artifact.display(),
                                        cli_bin
                                    ));
                                }
                                Ok(())
                            } else {
                                Err(KamError::CommandFailed(format!(
                                    "Privilege escalation via 'su' failed with status: {status:?}"
                                )))
                            }
                        }
                        Err(e) => Err(KamError::Io(e)),
                    }
                } else if is_command_not_found_error(&combined) {
                    Err(KamError::CommandFailed(trf!(
                        "install.cli_not_found",
                        cli_bin
                    )))
                } else {
                    // 退出代码为1直接报错
                    if out.status.code() == Some(1) {
                        Err(KamError::CommandFailed(
                            "安装失败，检查安装脚本或者检查root授权".to_string(),
                        ))
                    } else {
                        // 其他退出代码显示详细信息
                        let error_msg = if args.verbose {
                            format!(
                                "Module installation failed. Exit status: {}. Output:\n{s_out}\nError:\n{s_err}",
                                out.status
                            )
                        } else {
                            format!(
                                "Install command '{cli_bin}' exited with status: {}. Re-run with -v/--verbose to see the command output.",
                                out.status
                            )
                        };
                        Err(KamError::CommandFailed(error_msg))
                    }
                }
            }
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::NotFound {
                if crate::utils::command_exists("su") {
                    let cmd_str = shell_command(&cli_bin, &cli_args);
                    if !args.quiet {
                        Utils::info(&trf!("install.trying_su", cmd_str));
                    }
                    let mut su_cmd = Command::new("su");
                    su_cmd.arg("-c").arg(cmd_str).stdin(Stdio::inherit());
                    match Utils::run_and_stream_no_stderr_header(su_cmd) {
                        Ok(status) => {
                            if status.success() {
                                if !args.quiet {
                                    Utils::success(&trf!(
                                        "install.installed",
                                        artifact.display(),
                                        cli_bin
                                    ));
                                }
                                Ok(())
                            } else {
                                Err(KamError::CommandFailed(trf!(
                                    "install.su_failed",
                                    format!("exit status: {:?}", status)
                                )))
                            }
                        }
                        Err(e) => Err(KamError::Io(e)),
                    }
                } else {
                    Err(KamError::CommandFailed(trf!(
                        "install.cli_not_found",
                        cli_bin
                    )))
                }
            } else {
                Err(KamError::Io(e))
            }
        }
    }
}

/// Execute the install operation.
///
/// Extended behavior:
/// - If the provided path token looks like a git repository, attempt to clone,
///   optionally execute the repo's `kam.sh` after user review, build the project,
///   and install the produced artifact.
/// - Otherwise fallback to resolving the artifact locally (as before).
///
/// Run the `install` command.
///
/// Resolves the artifact to install (local .zip or a git-based repo), optionally
/// runs repository-provided `kam.sh` after interactive review, builds the project
/// if needed, and executes the platform-specific install command.
///
/// # Errors
/// - Returns `KamError::PackageNotFound` when the artifact to install cannot be found.
/// - Returns `KamError::Io` when underlying I/O operations (file/FS) fail.
/// - Returns `KamError::CommandFailed` when the underlying install command fails
///   (for example, `git`/`magisk` exit status or privilege escalation failure).
pub fn run(args: &InstallArgs) -> Result<(), KamError> {
    // If an explicit path was provided and it looks like a git spec, do git-based install
    if let Some(p) = args.path.clone()
        && let Some(s) = p.to_str()
        && looks_like_git_spec(s)
    {
        let (artifact, _tmpdir) = handle_git_install(s, args)?;
        // Keep tmpdir alive for the duration of installation by holding `_tmpdir`
        return execute_install_from_artifact(&artifact, args);
    }

    // Default (local) behavior
    let artifact = resolve_artifact_path(args.path.clone())?;
    execute_install_from_artifact(&artifact, args)
}

#[cfg(test)]
mod install_flow_tests {
    use super::*;

    #[test]
    fn adb_remote_path_uses_sdcard_staging_dir() {
        let path = adb_remote_path(Path::new("/tmp/MagicNet.zip")).unwrap();
        assert_eq!(path, "/sdcard/kam/tmp/MagicNet.zip");
    }
}
