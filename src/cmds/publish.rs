use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;
use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Publish build artifacts to a GitHub Release.
#[derive(Args, Debug, Clone)]
pub struct PublishArgs {
    /// Target GitHub repository in owner/repo format. Defaults to GITHUB_REPOSITORY, KAM_RELEASE_REPO, or git origin.
    #[arg(short = 'R', long = "repo")]
    pub repo: Option<String>,

    /// Release tag. Defaults to KAM_RELEASE_TAG or the module version in kam.toml.
    #[arg(long)]
    pub tag: Option<String>,

    /// Build output directory containing release assets.
    #[arg(long, default_value = "dist")]
    pub dist: PathBuf,

    /// Release title. Defaults to <module-id>-<versionCode>-<version>.
    #[arg(short, long)]
    pub title: Option<String>,

    /// Release notes text.
    #[arg(short, long)]
    pub notes: Option<String>,

    /// Read release notes from a file.
    #[arg(short = 'F', long = "notes-file")]
    pub notes_file: Option<PathBuf>,

    /// Mark the release as a prerelease.
    #[arg(short = 'p', long = "prerelease")]
    pub prerelease: bool,

    /// Keep the release as a draft instead of publishing it.
    #[arg(short = 'd', long = "draft")]
    pub draft: bool,

    /// Upload every file in --dist. By default only module ZIPs and their sidecar files are uploaded.
    #[arg(long = "all-assets")]
    pub all_assets: bool,

    /// Print the gh commands without executing them.
    #[arg(long)]
    pub dry_run: bool,
}

fn command_output(args: &[&str]) -> Option<String> {
    let output = Command::new(args.first()?).args(&args[1..]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn repo_from_git_origin() -> Option<String> {
    let remote = command_output(&["git", "remote", "get-url", "origin"])?;
    let remote = remote.trim_end_matches(".git").trim_end_matches('/');
    let path = if let Some(rest) = remote.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = remote.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = remote.strip_prefix("http://github.com/") {
        rest
    } else {
        remote
    };
    let mut parts = path.rsplitn(2, '/').collect::<Vec<_>>();
    if parts.len() != 2 {
        return None;
    }
    parts.reverse();
    Some(format!("{}/{}", parts[0], parts[1]))
}

fn resolve_repo(args: &PublishArgs) -> Result<String, KamError> {
    if let Some(repo) = args.repo.as_deref().filter(|repo| !repo.trim().is_empty()) {
        return Ok(repo.to_string());
    }
    if let Ok(repo) = std::env::var("GITHUB_REPOSITORY")
        && !repo.trim().is_empty()
    {
        return Ok(repo);
    }
    if let Ok(repo) = std::env::var("KAM_RELEASE_REPO")
        && !repo.trim().is_empty()
    {
        return Ok(repo);
    }
    repo_from_git_origin().ok_or_else(|| {
        KamError::CommandFailed(
            "Could not determine release repository. Pass --repo owner/repo.".to_string(),
        )
    })
}

fn resolve_project_metadata() -> Result<KamToml, KamError> {
    KamToml::load_from_dir(&std::env::current_dir()?)
}

fn resolve_tag(args: &PublishArgs, kam_toml: &KamToml) -> String {
    args.tag
        .clone()
        .or_else(|| std::env::var("KAM_RELEASE_TAG").ok())
        .unwrap_or_else(|| kam_toml.prop.version.clone())
}

fn resolve_title(args: &PublishArgs, kam_toml: &KamToml) -> String {
    args.title.clone().unwrap_or_else(|| {
        format!(
            "{}-{}-{}",
            kam_toml.prop.id, kam_toml.prop.versionCode, kam_toml.prop.version
        )
    })
}

fn is_module_zip(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| !name.eq_ignore_ascii_case("templates.zip"))
}

fn sidecar_candidates(zip: &Path) -> Vec<PathBuf> {
    let Some(file_name) = zip.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let Some(parent) = zip.parent() else {
        return Vec::new();
    };
    [
        format!("{file_name}.sig"),
        format!("{file_name}.sigstore.json"),
        format!("{file_name}.cert.pem"),
        format!("{file_name}.tsr"),
        format!("{file_name}.attestation.json"),
    ]
    .into_iter()
    .map(|name| parent.join(name))
    .filter(|path| path.is_file())
    .collect()
}

fn collect_assets(dist: &Path, all_assets: bool) -> Result<Vec<PathBuf>, KamError> {
    if !dist.exists() || !dist.is_dir() {
        return Err(KamError::PackageNotFound(format!(
            "Dist directory not found: {}",
            dist.display()
        )));
    }

    let files = fs::read_dir(dist)
        .map_err(KamError::Io)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();

    let mut assets = if all_assets {
        files
    } else {
        let mut selected = Vec::new();
        for zip in files.iter().filter(|path| is_module_zip(path)) {
            selected.push(zip.clone());
            selected.extend(sidecar_candidates(zip));
        }
        selected
    };

    assets.sort();
    if assets.is_empty() {
        return Err(KamError::PackageNotFound(format!(
            "No release assets found in {}",
            dist.display()
        )));
    }
    Ok(assets)
}

fn release_exists(repo: &str, tag: &str) -> bool {
    Command::new("gh")
        .args(["release", "view", tag, "--repo", repo])
        .status()
        .is_ok_and(|status| status.success())
}

fn run_gh(args: &[String], dry_run: bool) -> Result<(), KamError> {
    if dry_run {
        println!("gh {}", args.join(" "));
        return Ok(());
    }

    let status = Command::new("gh")
        .args(args)
        .status()
        .map_err(KamError::Io)?;
    if status.success() {
        Ok(())
    } else {
        Err(KamError::UploadFailed(format!(
            "gh {} exited with status {}",
            args.join(" "),
            status
        )))
    }
}

fn build_create_args(
    repo: &str,
    tag: &str,
    title: &str,
    args: &PublishArgs,
    assets: &[PathBuf],
) -> Vec<String> {
    let mut gh_args = vec![
        "release".to_string(),
        "create".to_string(),
        tag.to_string(),
        "--repo".to_string(),
        repo.to_string(),
        "--title".to_string(),
        title.to_string(),
    ];

    if let Some(notes_file) = args.notes_file.as_ref() {
        gh_args.push("--notes-file".to_string());
        gh_args.push(notes_file.display().to_string());
    } else if let Some(notes) = args.notes.as_ref() {
        gh_args.push("--notes".to_string());
        gh_args.push(notes.clone());
    } else {
        gh_args.push("--generate-notes".to_string());
    }

    if args.prerelease {
        gh_args.push("--prerelease".to_string());
    }

    if args.draft {
        gh_args.push("--draft".to_string());
    }

    gh_args.extend(assets.iter().map(|asset| asset.display().to_string()));
    gh_args
}

/// Run the publish command.
///
/// # Errors
/// Returns `KamError` when metadata cannot be loaded, assets are missing, or GitHub CLI fails.
pub fn run(args: &PublishArgs) -> Result<(), KamError> {
    let kam_toml = resolve_project_metadata()?;
    let repo = resolve_repo(args)?;
    let tag = resolve_tag(args, &kam_toml);
    let title = resolve_title(args, &kam_toml);
    let assets = collect_assets(&args.dist, args.all_assets)?;

    if !args.dry_run && release_exists(&repo, &tag) {
        return Err(KamError::UploadFailed(format!(
            "Release {tag} already exists in {repo}; immutable releases cannot be modified"
        )));
    }

    Utils::info(format!(
        "Publishing {} asset(s) to {repo} release {tag}",
        assets.len()
    ));

    let create_args = build_create_args(&repo, &tag, &title, args, &assets);
    run_gh(&create_args, args.dry_run)
}
