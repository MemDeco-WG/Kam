/*
src/cmds/install.rs

Implements a simple `kam install` command handler and helpers for detecting
the preferred root manager from environment or global config (~/.kam/config.toml).

Notes:
- This file intentionally keeps the command implementation compact (single-file)
  to simplify adding the command to the CLI. It exposes `InstallArgs` and
  `run` so the top-level dispatcher can call it.
*/

use crate::cmds::build::args::BuildArgs;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;
use clap::Args;
use dialoguer::Confirm;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;
use tempfile::TempDir;

/// CLI arguments for `kam install`
#[derive(Args, Debug, Clone)]
pub struct InstallArgs {
    /// Path to module package (.zip) to install. If omitted, attempts to find
    /// the artifact in the project (dist) output directory.
    pub path: Option<PathBuf>,

    /// Preferred root manager (overrides config). Valid values: Magisk, KernelSU, APatchSU
    #[arg(long)]
    pub manager: Option<String>,

    /// Print the derived install command without executing it
    #[arg(long)]
    pub dry_run: bool,

    /// Verbose output showing install command output (stdout/stderr)
    #[arg(short = 'v', long, conflicts_with = "quiet")]
    pub verbose: bool,

    /// Suppress non-essential output
    #[arg(short, long)]
    pub quiet: bool,

    /// Assume "yes" to all confirmation prompts (equivalent to -y). Use `-y` or `--yes` to skip confirmation.
    #[arg(short = 'y', long = "yes", action = clap::ArgAction::SetTrue, global = true)]
    pub assume_yes: bool,
}

/// Public helper: read preferred root manager from environment or global config.
/// Priority:
/// 1) KAM_ROOT_MANAGER environment variable
/// 2) ~/.kam/config.toml -> root.manager (or root_manager / manager fallback)
///    Returns normalized manager name: "Magisk", "KernelSU", "APatchSU", or "Unknown"
pub fn get_root_manager() -> String {
    // 1) env override
    if let Ok(env_val) = std::env::var("KAM_ROOT_MANAGER") {
        let n = crate::utils::normalize_root_manager(&env_val);
        if n != "Unknown" {
            return n;
        }
    }

    // 2) global config (~/.kam/config.toml)
    // Use `KAM_HOME` if provided; otherwise fall back to the default Kam home directory.
    if let Ok(cfg_home) = crate::utils::kam_home_dir() {
        let cfg = cfg_home.join("config.toml");
        if cfg.exists()
            && let Ok(content) = fs::read_to_string(&cfg)
            && let Ok(v) = toml::from_str::<toml::Value>(&content)
        {
            // Try table: [root] manager = "Magisk"
            if let Some(root_tbl) = v.get("root")
                && let Some(m) = root_tbl.get("manager").and_then(|x| x.as_str())
            {
                let n = crate::utils::normalize_root_manager(m);
                if n != "Unknown" {
                    return n;
                }
            }
            // Try fallback keys: root_manager or manager (looser)
            if let Some(m) = v.get("root_manager").and_then(|x| x.as_str()) {
                let n = crate::utils::normalize_root_manager(m);
                if n != "Unknown" {
                    return n;
                }
            }
            if let Some(m) = v.get("manager").and_then(|x| x.as_str()) {
                let n = crate::utils::normalize_root_manager(m);
                if n != "Unknown" {
                    return n;
                }
            }
        }
    }

    "Unknown".to_string()
}

// Normalization moved to the public utility `crate::utils::normalize_root_manager`.
// This keeps the canonicalization logic in a shared, testable place.

/// Resolve the install CLI and its arguments for a chosen manager.
/// If `manager_override` is Some it will be normalized and used; otherwise the
/// configured manager (get_root_manager) is consulted.
fn get_install_cli_for_manager(
    path: &Path,
    manager_override: Option<&str>,
) -> Result<(String, Vec<String>), KamError> {
    let manager = manager_override.map_or_else(get_root_manager, |m| {
        crate::utils::normalize_root_manager(m)
    });

    let p = path.to_string_lossy().to_string();

    match manager.as_str() {
        "Magisk" => Ok((
            "magisk".to_string(),
            vec!["--install-module".to_string(), p],
        )),
        "KernelSU" => Ok((
            "ksud".to_string(),
            vec!["module".to_string(), "install".to_string(), p],
        )),
        "APatchSU" => Ok((
            "apd".to_string(),
            vec!["module".to_string(), "install".to_string(), p],
        )),
        _ => Err(KamError::CommandFailed(crate::i18n::tr(
            "install.unable_to_determine",
        ))),
    }
}

/// Resolve an artifact path to install.
/// Priority:
/// 1) explicit path argument (must exist)
/// 2) if project (kam.toml) exists: use kam.kam.build.target_dir (default dist) and the computed basename
/// 3) fallback: search current dir and ./dist for the newest .zip file
fn resolve_artifact_path(explicit: Option<PathBuf>) -> Result<PathBuf, KamError> {
    if let Some(p) = explicit {
        if !p.exists() || !p.is_file() {
            return Err(KamError::PackageNotFound(format!(
                "Package not found: {}",
                p.display()
            )));
        }
        return Ok(p.canonicalize().unwrap_or(p));
    }

    let cwd = std::env::current_dir().map_err(KamError::Io)?;

    // Try project output first if kam.toml exists
    if cwd.join("kam.toml").exists() {
        let kt = KamToml::load_from_dir(&cwd)?;
        // Determine target_dir
        let target_dir = kt
            .kam
            .build
            .as_ref()
            .and_then(|b| b.target_dir.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("dist");
        // Determine basename
        let basename = crate::cmds::build::build_project::determine_basename(&kt)?;
        let candidate = cwd.join(target_dir).join(format!("{}.zip", basename));
        if candidate.exists() && candidate.is_file() {
            return Ok(candidate.canonicalize().unwrap_or(candidate));
        }
    }

    // Fallback: find newest .zip in cwd or cwd/dist
    let mut candidates: Vec<PathBuf> = Vec::new();
    for dir in &[cwd.clone(), cwd.join("dist")] {
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
        return Err(KamError::PackageNotFound(crate::i18n::tr(
            "install.package_not_found",
        )));
    }

    // Pick the most recently modified candidate
    let latest = candidates
        .into_iter()
        .max_by_key(|p| {
            fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        })
        .unwrap();

    Ok(latest.canonicalize().unwrap_or(latest))
}

/// Helper: heuristic detection whether a given token looks like a Git repository spec
fn looks_like_git_spec(s: &str) -> bool {
    let s_trim = s.trim();
    if s_trim.is_empty() {
        return false;
    }
    // If it exists locally, treat as a local file/path, not a git spec.
    if Path::new(s_trim).exists() {
        return false;
    }

    let lower = s_trim.to_ascii_lowercase();

    // Accept a variety of common git spec prefixes including "+git" shorthand.
    if lower.starts_with("git+")
        || lower.starts_with("gh+")
        || lower.starts_with("+git")
        || lower.starts_with("+gh")
    {
        return true;
    }
    if lower.starts_with("git@") || lower.ends_with(".git") {
        return true;
    }
    if lower.contains("://")
        && (lower.contains("github.com")
            || lower.contains("gitlab.com")
            || lower.contains("bitbucket.org")
            || lower.contains("git"))
    {
        return true;
    }

    // Accept shorthand "owner/repo"
    if s_trim.contains('/')
        && !s_trim.starts_with("./")
        && !s_trim.starts_with('/')
        && !s_trim.contains('\\')
        && !s_trim.contains(':')
    {
        let parts: Vec<_> = s_trim.split('/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return true;
        }
    }

    false
}

/// Expand a lightweight "owner/repo" shorthand to a default HTTPS repo URL usable by `git clone`.
fn expand_git_shorthand(s: &str) -> String {
    // Normalize and strip common prefixes used to indicate a git shorthand.
    // Support:
    //   - git+owner/repo
    //   - gh+owner/repo
    //   - +gitowner/repo
    //   - +git:owner/repo
    let mut s_trim = s.trim().to_string();
    let lower = s_trim.to_ascii_lowercase();
    // Prefer longer/colon-terminated prefixes first to avoid matching the shorter
    // '+git' before '+git:' which would leave a leading ':' in the remainder.
    for prefix in &["+git:", "+gh:", "git+", "gh+", "+git", "+gh"] {
        if lower.starts_with(prefix) {
            s_trim = s_trim[prefix.len()..].to_string();
            break;
        }
    }

    if s_trim.contains("://") || s_trim.contains('@') || s_trim.ends_with(".git") {
        s_trim
    } else if s_trim.contains('/') {
        format!("https://github.com/{}.git", s_trim)
    } else {
        s_trim
    }
}

/// Clone a repository into a temporary directory preferring `gh` then `git`.
/// Returns the TempDir (kept alive by caller) and the checkout path inside it.
fn create_tempdir_with_fallback_override(
    override_tmpdir: Option<&Path>,
) -> Result<TempDir, io::Error> {
    // If caller provided an override directory, try that first (useful for tests).
    if let Some(ov) = override_tmpdir {
        if fs::create_dir_all(ov).is_ok() {
            if let Ok(td) = tempfile::tempdir_in(ov) {
                return Ok(td);
            }
            Utils::warn(format!(
                "Failed to create tempdir inside override: {}",
                ov.display()
            ));
        } else {
            Utils::warn(format!(
                "Failed to create override tmp dir '{}'",
                ov.display()
            ));
        }
    }

    // 1) If the user has explicitly set TMPDIR, try to use/create it first.
    if let Ok(tmp_env) = std::env::var("TMPDIR") {
        let p = PathBuf::from(tmp_env);
        if let Err(e) = fs::create_dir_all(&p) {
            Utils::warn(format!("Failed to create TMPDIR '{}': {}", p.display(), e));
        } else if let Ok(td) = tempfile::tempdir_in(&p) {
            return Ok(td);
        }
        Utils::warn(format!(
            "Failed to create tempdir inside TMPDIR '{}'",
            p.display()
        ));
    }

    // 2) Try the system default temp dir
    match tempfile::tempdir() {
        Ok(td) => Ok(td),
        Err(e) => {
            Utils::warn(format!("Default tempdir() failed: {}", e));
            // 3) Try $HOME/.cache/kam/tmp
            if let Ok(home) = std::env::var("HOME") {
                let p = PathBuf::from(home).join(".cache").join("kam").join("tmp");
                if fs::create_dir_all(&p).is_ok() {
                    if let Ok(td2) = tempfile::tempdir_in(&p) {
                        Utils::warn(format!("Using fallback tempdir: {}", p.display()));
                        return Ok(td2);
                    }
                    Utils::warn(format!(
                        "Failed to create tempdir inside fallback: {}",
                        p.display()
                    ));
                }
            }

            // 4) Ensure std::env::temp_dir exists and try it
            {
                let p = std::env::temp_dir();
                if fs::create_dir_all(&p).is_ok() {
                    if let Ok(td2) = tempfile::tempdir_in(&p) {
                        Utils::warn(format!("Using fallback tempdir: {}", p.display()));
                        return Ok(td2);
                    }
                    Utils::warn(format!(
                        "Failed to create tempdir inside fallback: {}",
                        p.display()
                    ));
                }
            }

            // 5) Try a per-project fallback (./.kam_tmp)
            if let Ok(cwd) = std::env::current_dir() {
                let p = cwd.join(".kam_tmp");
                if fs::create_dir_all(&p).is_ok() {
                    if let Ok(td2) = tempfile::tempdir_in(&p) {
                        Utils::warn(format!("Using fallback tempdir: {}", p.display()));
                        return Ok(td2);
                    }
                    Utils::warn(format!(
                        "Failed to create tempdir inside fallback: {}",
                        p.display()
                    ));
                }
            }

            // If all attempts failed, return the original error
            Err(e)
        }
    }
}

fn create_tempdir_with_fallback() -> Result<TempDir, io::Error> {
    create_tempdir_with_fallback_override(None)
}

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
            "Cloning '{}' using 'gh' into: {}",
            gh_spec,
            dest.display()
        ));
        let mut cmd = Command::new("gh");
        cmd.arg("repo")
            .arg("clone")
            .arg(gh_spec)
            .arg(dest.to_str().unwrap());
        cmd.stdin(Stdio::inherit());
        match Utils::run_and_stream_no_stderr_header(cmd) {
            Ok(status) if status.success() => return Ok((tmp, dest)),
            Ok(status) => {
                Utils::warn(format!("'gh' clone failed with status: {:?}", status));
                // fallthrough to git
            }
            Err(e) => {
                Utils::warn(format!("'gh' clone failed: {}", e));
                // fallthrough to git
            }
        }
    }

    // Fallback to git
    if crate::utils::command_exists("git") {
        let url = expand_git_shorthand(spec);
        Utils::info(format!(
            "Cloning '{}' using 'git' into: {}",
            url,
            dest.display()
        ));
        let mut cmd = Command::new("git");
        cmd.arg("clone").arg(&url).arg(dest.to_str().unwrap());
        cmd.stdin(Stdio::inherit());
        match Utils::run_and_stream_no_stderr_header(cmd) {
            Ok(status) if status.success() => return Ok((tmp, dest)),
            Ok(status) => {
                return Err(KamError::CommandFailed(format!(
                    "git clone failed with status: {:?}",
                    status
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
                        .and_then(|l| l.strip_prefix("#!").map(|s| s.to_string()))
                        .unwrap_or_else(|| "sh".to_string());
                    // Use a simple heuristic: if shebang contains 'bash' prefer 'bash', otherwise fall back to 'sh'
                    let exec = if interpreter.contains("bash") {
                        "bash"
                    } else {
                        "sh"
                    };
                    Utils::info(format!("Executing '{}' {}", exec, kam_sh.display()));
                    let mut cmd = Command::new(exec);
                    cmd.arg(kam_sh.to_str().unwrap()).stdin(Stdio::inherit());
                    let status =
                        Utils::run_and_stream_no_stderr_header(cmd).map_err(KamError::Io)?;
                    if !status.success() {
                        return Err(KamError::CommandFailed(format!(
                            "'kam.sh' execution failed with status: {:?}",
                            status
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
                .and_then(|b| b.target_dir.as_ref())
                .map(|s| s.as_str())
                .unwrap_or("dist");
            let basename = crate::cmds::build::build_project::determine_basename(kt_ref)?;
            let candidate = workdir.join(target_dir).join(format!("{}.zip", basename));
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
                .unwrap();
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

/// Perform the actual install once we have an artifact path. Extracted from the
/// original `run` implementation so both local and git-based flows can share it.
fn execute_install_from_artifact(artifact: &Path, args: &InstallArgs) -> Result<(), KamError> {
    if !args.quiet {
        Utils::section(&trf!("install.section", artifact.display()));
    }

    let (cli_bin, cli_args) = get_install_cli_for_manager(artifact, args.manager.as_deref())?;

    if args.dry_run {
        if !args.quiet {
            Utils::info(&trf!(
                "Dry run: will execute '{} {}'",
                cli_bin,
                cli_args.join(" ")
            ));
        } else {
            println!("{} {}", cli_bin, cli_args.join(" "));
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
                    let cmd_str = std::iter::once(cli_bin.clone())
                        .chain(cli_args.iter().cloned())
                        .map(|s| {
                            if s.contains('\'') {
                                format!("'{}'", s.replace("'", "'\"'\"'"))
                            } else {
                                format!("'{}'", s)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
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
                                "Privilege escalation via 'su' failed with status: {:?}",
                                su_status
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
                    "Install command '{}' exited with status: {}. Check the output above for details.",
                    cli_bin, status
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
                let combined = format!("{}{}", s_out, s_err);

                // 只有在真正的权限错误时才尝试使用 su
                if is_permission_error(&combined) && crate::utils::command_exists("su") {
                    let cmd_str = std::iter::once(cli_bin.clone())
                        .chain(cli_args.iter().cloned())
                        .map(|s| {
                            if s.contains('\'') {
                                format!("'{}'", s.replace("'", "'\"'\"'"))
                            } else {
                                format!("'{}'", s)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
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
                                return Ok(());
                            }
                            return Err(KamError::CommandFailed(format!(
                                "Privilege escalation via 'su' failed with status: {:?}",
                                status
                            )));
                        }
                        Err(e) => return Err(KamError::Io(e)),
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
                                "Module installation failed. Exit status: {}. Output:\n{}\nError:\n{}",
                                out.status, s_out, s_err
                            )
                        } else {
                            format!(
                                "Install command '{}' exited with status: {}. Re-run with -v/--verbose to see the command output.",
                                cli_bin, out.status
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
                    let cmd_str = std::iter::once(cli_bin.clone())
                        .chain(cli_args.iter().cloned())
                        .map(|s| {
                            if s.contains('\'') {
                                format!("'{}'", s.replace("'", "'\"'\"'"))
                            } else {
                                format!("'{}'", s)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
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
pub fn run(args: InstallArgs) -> Result<(), KamError> {
    // If an explicit path was provided and it looks like a git spec, do git-based install
    if let Some(p) = args.path.clone()
        && let Some(s) = p.to_str()
        && looks_like_git_spec(s)
    {
        let (artifact, _tmpdir) = handle_git_install(s, &args)?;
        // Keep tmpdir alive for the duration of installation by holding `_tmpdir`
        return execute_install_from_artifact(&artifact, &args);
    }

    // Default (local) behavior
    let artifact = resolve_artifact_path(args.path.clone())?;
    execute_install_from_artifact(&artifact, &args)
}
