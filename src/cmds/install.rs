/*
src/cmds/install.rs

Implements a simple `kam install` command handler and helpers for detecting
the preferred root manager from environment or global config (~/.kam/config.toml).

Notes:
- This file intentionally keeps the command implementation compact (single-file)
  to simplify adding the command to the CLI. It exposes `InstallArgs` and
  `run` so the top-level dispatcher can call it.
*/

use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;
use clap::Args;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

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

    /// Suppress non-essential output
    #[arg(short, long)]
    pub quiet: bool,
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
    if let Some(home) = dirs::home_dir() {
        let cfg = home.join(".kam").join("config.toml");
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
    let manager = if let Some(m) = manager_override {
        crate::utils::normalize_root_manager(m)
    } else {
        get_root_manager()
    };

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
            "Unable to determine install CLI. Please set 'root.manager' in ~/.kam/config.toml or pass --manager",
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
        return Err(KamError::PackageNotFound(
            "No zip package found in project or dist directories".to_string(),
        ));
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

/// Execute the install operation.
pub fn run(args: InstallArgs) -> Result<(), KamError> {
    // Resolve artifact
    let artifact = resolve_artifact_path(args.path.clone())?;

    if !args.quiet {
        Utils::section(&trf!("install.section", artifact.display()));
    }

    // Determine CLI and args to run (respecting --manager if provided)
    let (cli_bin, cli_args) = get_install_cli_for_manager(&artifact, args.manager.as_deref())?;

    if args.dry_run {
        if !args.quiet {
            Utils::info(&trf!(
                "Dry run: will execute '{} {}'",
                cli_bin,
                cli_args.join(" ")
            ));
        } else {
            // In quiet mode print minimal output
            println!("{} {}", cli_bin, cli_args.join(" "));
        }
        return Ok(());
    }

    if !args.quiet {
        Utils::info(&trf!("install.executing", cli_bin, cli_args.join(" ")));
    }

    match Command::new(&cli_bin).args(&cli_args).output() {
        Ok(out) => {
            // Print outputs nicely using central utility
            Utils::print_cmd_output(&out.stdout, &out.stderr);
            if out.status.success() {
                if !args.quiet {
                    Utils::success(&trf!("install.installed", artifact.display(), cli_bin));
                }
                Ok(())
            } else {
                // If output suggests the command is missing or requires privilege,
                // attempt to escalate using `su -c '...'` on Android / Unix-like where `su` is available.
                let s_out = String::from_utf8_lossy(&out.stdout).to_lowercase();
                let s_err = String::from_utf8_lossy(&out.stderr).to_lowercase();
                let combined = format!("{}{}", s_out, s_err);
                if (combined.contains("not found")
                    || combined.contains("command not found")
                    || combined.contains("permission denied"))
                    && crate::utils::command_exists("su")
                {
                    // Build a safely quoted command string for `su -c`
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
                    match Command::new("su")
                        .arg("-c")
                        .arg(cmd_str)
                        .stdin(Stdio::inherit())
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .status()
                    {
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
                                Err(KamError::CommandFailed(trf!(
                                    "Privilege escalation via 'su' failed with status: {:?}",
                                    status
                                )))
                            }
                        }
                        Err(e) => Err(KamError::Io(e)),
                    }
                } else if combined.contains("not found") || combined.contains("command not found") {
                    Err(KamError::CommandFailed(trf!(
                        "Install CLI '{}' not found on PATH. Please install it or set 'root.manager' in ~/.kam/config.toml",
                        cli_bin
                    )))
                } else {
                    Err(KamError::CommandFailed(format!(
                        "Install command '{}' exited with status: {}",
                        cli_bin, out.status
                    )))
                }
            }
        }
        Err(e) => {
            // If the binary isn't found, try to escalate via su if available
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
                    match Command::new("su")
                        .arg("-c")
                        .arg(cmd_str)
                        .stdin(Stdio::inherit())
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .status()
                    {
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
