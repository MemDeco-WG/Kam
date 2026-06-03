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
use std::process::{Command, Output, Stdio};
use std::time::SystemTime;
use tempfile::TempDir;

/// CLI arguments for `kam install`
#[allow(clippy::struct_excessive_bools)]
#[derive(Args, Debug, Clone)]
pub struct InstallArgs {
    /// Path to module package (.zip) to install. If omitted, attempts to find
    /// the artifact in the project (dist) output directory.
    pub path: Option<PathBuf>,

    /// Preferred root manager (overrides config). Valid values: Auto, Magisk, KernelSU, APatchSU
    #[arg(long)]
    pub manager: Option<String>,

    /// Print the derived install command without executing it
    #[arg(long)]
    pub dry_run: bool,

    /// Install through adb: push the module ZIP to /data/local/tmp, then run the
    /// selected root manager on the connected device via `adb shell su -c`.
    #[arg(long)]
    pub adb: bool,

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
///    Returns normalized manager name: "Auto", "Magisk", "KernelSU", "APatchSU", or "Unknown"
#[must_use]
pub fn get_root_manager() -> String {
    // 1) env override
    if let Ok(env_val) = std::env::var("KAM_ROOT_MANAGER") {
        let n = normalize_root_manager_or_auto(&env_val);
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
                let n = normalize_root_manager_or_auto(m);
                if n != "Unknown" {
                    return n;
                }
            }
            // Try fallback keys: root_manager or manager (looser)
            if let Some(m) = v.get("root_manager").and_then(|x| x.as_str()) {
                let n = normalize_root_manager_or_auto(m);
                if n != "Unknown" {
                    return n;
                }
            }
            if let Some(m) = v.get("manager").and_then(|x| x.as_str()) {
                let n = normalize_root_manager_or_auto(m);
                if n != "Unknown" {
                    return n;
                }
            }
        }
    }

    "Auto".to_string()
}

// Normalization moved to the public utility `crate::utils::normalize_root_manager`.
// This keeps the canonicalization logic in a shared, testable place.

fn normalize_root_manager_or_auto(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("auto") {
        "Auto".to_string()
    } else {
        crate::utils::normalize_root_manager(trimmed)
    }
}

fn detect_local_root_manager() -> String {
    for (manager, cli_bin) in [
        ("KernelSU", "ksud"),
        ("Magisk", "magisk"),
        ("APatchSU", "apd"),
    ] {
        if crate::utils::command_exists(cli_bin) {
            return manager.to_string();
        }
    }
    "Unknown".to_string()
}

fn resolve_root_manager(manager_override: Option<&str>) -> Result<String, KamError> {
    let requested = manager_override.map_or_else(get_root_manager, normalize_root_manager_or_auto);
    match requested.as_str() {
        "Auto" => {
            let detected = detect_local_root_manager();
            if detected == "Unknown" {
                Err(KamError::CommandFailed(
                    "Unable to auto-detect root manager CLI. Install magisk/ksud/apd, set root.manager, or pass --manager.".to_string(),
                ))
            } else {
                Ok(detected)
            }
        }
        "Unknown" => Err(KamError::CommandFailed(crate::i18n::tr(
            "install.unable_to_determine",
        ))),
        manager => Ok(manager.to_string()),
    }
}

/// Resolve the install CLI and its arguments for a chosen manager.
/// If `manager_override` is Some it will be normalized and used; otherwise the
/// configured manager (get_root_manager) is consulted.
fn get_install_cli_for_manager(
    path: &Path,
    manager_override: Option<&str>,
) -> Result<(String, Vec<String>), KamError> {
    get_install_cli_for_manager_path(&path.to_string_lossy(), manager_override)
}

fn get_install_cli_for_manager_path(
    package_path: &str,
    manager_override: Option<&str>,
) -> Result<(String, Vec<String>), KamError> {
    let manager = resolve_root_manager(manager_override)?;
    install_cli_for_manager_name(&manager, package_path)
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
            .and_then(|b| b.target_dir.as_deref())
            .unwrap_or("dist");
        // Determine basename
        let basename = crate::cmds::build::build_project::determine_basename(&kt)?;
        let candidate = cwd.join(target_dir).join(format!("{basename}.zip"));
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
        .ok_or_else(|| KamError::PackageNotFound(crate::i18n::tr("install.package_not_found")))?;

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
    if lower.starts_with("git@")
        || std::path::Path::new(&lower)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("git"))
    {
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

    if s_trim.contains("://")
        || s_trim.contains('@')
        || std::path::Path::new(&s_trim)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("git"))
    {
        s_trim
    } else if s_trim.contains('/') {
        format!("https://github.com/{s_trim}.git")
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
            Utils::warn(format!("Default tempdir() failed: {e}"));
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

