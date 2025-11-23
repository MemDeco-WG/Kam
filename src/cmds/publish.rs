use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::compute_index_path;
use chrono;
use clap::Args;
use colored::Colorize;
use flate2::read::GzDecoder;
use git2::Repository;
use regex::Regex;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

/// Arguments for the publish command
#[derive(Args, Debug)]
pub struct PublishArgs {
    /// Path to the project (default: current directory)
    #[arg(short, long, default_value = ".")]
    pub path: String,

    /// Repository URL or local path to publish to
    #[arg(short = 'r', long)]
    pub repo: Option<String>,

    /// Authorization token for HTTP uploads
    #[arg(long)]
    pub token: Option<String>,

    /// Dry-run: build but don't actually upload
    #[arg(long)]
    pub dry_run: bool,

    /// Output directory to place the built package before publishing
    #[arg(long)]
    pub output: Option<String>,
}

/// Run the publish command
///
/// Steps:
/// 1. Build the module (delegates to the build command logic)
/// 2. Find the package file (zip) in the output directory
/// 3. Upload the file to the repository (file copy for local paths or HTTP POST/PUT)
pub fn run(args: PublishArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);

    println!("{} Publishing module...", "→".cyan());

    // Load kam.toml to determine module id/version
    let kam_toml = KamToml::load_from_dir(&project_path)?;
    let module_id = kam_toml.prop.id.clone();
    let version_string = kam_toml.prop.version.clone();
    let version_code = kam_toml.prop.versionCode;
    let version = version_code.to_string();
    let module_type = &kam_toml.kam.module_type;

    // Determine output directory to build into
    let output_dir: PathBuf = args
        .output
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| project_path.join("dist"));

    // Ensure output exists
    fs::create_dir_all(&output_dir)?;

    // Build package by invoking the existing build logic
    // We call the build command implementation directly to avoid duplicating logic.
    let build_args = crate::cmds::build::BuildArgs {
        path: args.path.clone(),
        all: false,
        output: Some(output_dir.to_string_lossy().to_string()),
    };

    crate::cmds::build::run(build_args)?;

    // Find the produced package file — prefer pattern "{id}-{versionCode}.zip"
    let default_name = format!("{}-{}.zip", module_id, version_code);
    let candidate = output_dir.join(&default_name);
    let package_path = if candidate.exists() {
        candidate
    } else {
        // Fallback: pick the first zip file in the output dir
        let mut found: Option<PathBuf> = None;
        for entry in fs::read_dir(&output_dir)? {
            let p = entry?.path();
            if p.is_file() {
                if let Some(ext) = p.extension() {
                    if ext == "zip"
                        || p.file_name()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .ends_with(".tar.gz")
                    {
                        found = Some(p);
                        break;
                    }
                }
            }
        }
        found.ok_or_else(|| {
            KamError::PackageNotFound(format!("Package not found in {}", output_dir.display()))
        })?
    };

    println!("  {} Package: {}", "✓".green(), package_path.display());

    if args.dry_run {
        println!("  {} Dry-run: skipping upload", "•".yellow());
        return Ok(());
    }

    // Determine repository target:
    // Priority: CLI `--repo` (-r) -> kam.toml [mmrl.repo].repository -> none (print and exit)
    let repo_opt = if let Some(r) = args.repo.as_ref().cloned() {
        Some(r)
    } else {
        // Use chained option access to avoid deep nesting
        let repo_from_kam = kam_toml
            .mmrl
            .as_ref()
            .and_then(|m| m.repo.as_ref())
            .and_then(|r| r.repository.as_ref())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        repo_from_kam
    };

    if let Some(repo) = repo_opt {
        // Local filesystem publish (file:// or plain path)
        if repo.starts_with("file://") || !repo.contains("://") {
            // Normalize path
            let dest = if repo.starts_with("file://") {
                PathBuf::from(repo.trim_start_matches("file://"))
            } else {
                PathBuf::from(repo.clone())
            }
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(repo));

            fs::create_dir_all(&dest)?;

            // Fallback: plain directory copy
            let dest_file = dest.join(package_path.file_name().ok_or_else(|| {
                KamError::InvalidFilename("invalid package filename".to_string())
            })?);
            fs::copy(&package_path, &dest_file)?;
            println!(
                "  {} Published to local repository: {}",
                "✓".green(),
                dest_file.display()
            );
            return Ok(());
        }

        // Otherwise try HTTP upload (simple PUT)
        // If the repo is an HTTP(S) URL, append the package filename so we don't overwrite the repository root.
        let mut upload_target = repo.clone();
        if repo.starts_with("http://") || repo.starts_with("https://") {
            let file_name = package_path
                .file_name()
                .ok_or_else(|| KamError::InvalidFilename("invalid package filename".to_string()))?
                .to_string_lossy()
                .to_string();
            if upload_target.ends_with('/') {
                upload_target.push_str(&file_name);
            } else {
                upload_target.push('/');
                upload_target.push_str(&file_name);
            }
        }

        println!("  {} Uploading to {}", "→".cyan(), upload_target);
        // Resolve token: prefer CLI arg, then common environment vars (GITHUB_TOKEN, KAM_PUBLISH_TOKEN)
        let token_opt: Option<String> = args
            .token
            .clone()
            .or_else(|| std::env::var("GITHUB_TOKEN").ok())
            .or_else(|| std::env::var("KAM_PUBLISH_TOKEN").ok());

        let client = reqwest::blocking::Client::new();
        let mut req = client.put(&upload_target).body(fs::read(&package_path)?);
        if let Some(tok) = token_opt.as_ref() {
            req = req.header("Authorization", format!("Bearer {}", tok));
        }
        let resp = req
            .send()
            .map_err(|e| KamError::UploadFailed(format!("upload failed: {}", e)))?;
        if !resp.status().is_success() {
            return Err(KamError::UploadFailed(format!(
                "upload failed: HTTP {}",
                resp.status()
            )));
        }

        println!("  {} Published to {}", "✓".green(), repo);
        Ok(())
    } else {
        println!(
            "  {} No repository provided; package is available at: {}",
            "i".cyan(),
            package_path.display()
        );
        Ok(())
    }
}

/// Update repo index for a published library
fn update_repo_index(
    repo_path: &Path,
    module_id: &str,
    version: &str,
    kam_toml: &KamToml,
    package_filename: &str,
) -> Result<(), KamError> {
    // Create index directory structure based on module name
    let index_dir = repo_path.join("index");
    let module_index_path = compute_index_path(&index_dir, module_id);
    fs::create_dir_all(&module_index_path)?;

    // Create metadata JSON for this version
    let metadata = serde_json::json!({
        "id": module_id,
        "version": version,
        "versionCode": kam_toml.prop.versionCode,
        "author": kam_toml.prop.author,
        "description": kam_toml.prop.description.get("en").unwrap_or(&String::new()),
        "provides": &Vec::<serde_json::Value>::new(),
        "package": package_filename,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    let metadata_file = module_index_path.join(format!("{}.json", version));
    let metadata_str =
        serde_json::to_string_pretty(&metadata).map_err(|e| KamError::JsonError(e.to_string()))?;
    fs::write(&metadata_file, &metadata_str)?;

    // Update latest.json to point to this version if it's newer
    let latest_file = module_index_path.join("latest.json");
    let should_update_latest = if latest_file.exists() {
        let latest_content = fs::read_to_string(&latest_file)?;
        let latest: serde_json::Value = serde_json::from_str(&latest_content)
            .map_err(|e| KamError::JsonError(e.to_string()))?;

        // Simple version comparison (could be improved)
        latest
            .get("version")
            .and_then(|v| v.as_str())
            .map(|v| version > v)
            .unwrap_or(true)
    } else {
        true
    };

    if should_update_latest {
        fs::write(&latest_file, &metadata_str)?;
    }

    Ok(())
}



/// Copy directory recursively
fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), KamError> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Get GitHub repo owner and name from git remote
#[allow(dead_code)]
/// Get GitHub repo owner and name from the `origin` remote.
/// Returns (owner, repo_name) if available.
fn get_github_repo_info() -> Result<(String, String), KamError> {
    let git_repo = Repository::open(".").map_err(|e| KamError::Io(std::io::Error::other(e)))?;
    let origin_remote = git_repo
        .find_remote("origin")
        .map_err(|e| KamError::Io(std::io::Error::other(e)))?;
    let url = origin_remote
        .url()
        .ok_or(KamError::InvalidConfig("No remote url".to_string()))?;
    let url_str = url.to_string();

    let re = Regex::new(r"github\.com[\/:]([^\/]+)\/([^\/]+?)(\.git)?$")
        .map_err(|e| KamError::InvalidConfig(format!("Regex error: {}", e)))?;
    if let Some(captures) = re.captures(&url_str) {
        let owner = captures.get(1).unwrap().as_str().to_string();
        let repo_name = captures.get(2).unwrap().as_str().to_string();
        Ok((owner, repo_name))
    } else {
        Err(KamError::InvalidConfig("Not a GitHub repo".to_string()))
    }
}

/// Create GitHub issue for module submission
fn create_github_issue(
    owner: &str,
    repo: &str,
    module_id: &str,
    version: &str,
    kam_toml: &KamToml,
    package_filename: &str,
    token: Option<&str>,
) -> Result<(), KamError> {
    let github_token = std::env::var("GITHUB_TOKEN").ok();
    let kam_token = std::env::var("KAM_PUBLISH_TOKEN").ok();
    let token = token
        .or_else(|| github_token.as_deref())
        .or_else(|| kam_token.as_deref())
        .ok_or(KamError::InvalidConfig("GitHub token required".to_string()))?;

    let client = reqwest::blocking::Client::new();

    // Create module metadata JSON
    let metadata = serde_json::json!({
        "id": module_id,
        "name": kam_toml.prop.name.get("en").unwrap_or(&module_id.to_string()),
        "version": version,
        "versionCode": kam_toml.prop.versionCode,
        "author": kam_toml.prop.author,
        "description": kam_toml.prop.description.get("en").unwrap_or(&String::new()),
        "license": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.license.as_ref()).unwrap_or(&"MIT".to_string()),
        "homepage": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.homepage.as_ref()).unwrap_or(&String::new()),
        "support": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.support.as_ref()).unwrap_or(&String::new()),
        "donate": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.donate.as_ref()).unwrap_or(&String::new()),
        "cover": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.cover.as_ref()).unwrap_or(&String::new()),
        "icon": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.icon.as_ref()).unwrap_or(&String::new()),
        "readme": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.readme.as_ref()).unwrap_or(&String::new()),
        "changelog": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.changelog.as_ref()).unwrap_or(&String::new()),
        "categories": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.categories.as_ref()).unwrap_or(&Vec::new()),
        "keywords": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.keywords.as_ref()).unwrap_or(&Vec::new()),
        "require": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.require.as_ref()).unwrap_or(&Vec::new()),
        "antifeatures": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.antifeatures.as_ref()).unwrap_or(&Vec::new()),
        "provides": &Vec::<serde_json::Value>::new(),
        "versions": [{
            "version": version,
            "versionCode": kam_toml.prop.versionCode,
            "zipUrl": format!("https://github.com/{}/{}/releases/download/{}-{}/{}", owner, repo, module_id, version, package_filename),
            "changelog": kam_toml.mmrl.as_ref().and_then(|m| m.repo.as_ref()).and_then(|r| r.changelog.as_ref()).unwrap_or(&String::new()),
            "size": 0, // TODO: get actual size
            "timestamp": chrono::Utc::now().timestamp() as f64
        }],
        "timestamp": chrono::Utc::now().timestamp() as f64
    });

    let create_issue_url = format!("https://api.github.com/repos/{}/{}/issues", owner, repo);
    let title = format!("Module Submission: {} v{}", module_id, version);
    let body = format!(
        "```json\n{}\n```",
        serde_json::to_string_pretty(&metadata).unwrap()
    );

    let issue_body = json!({
        "title": title,
        "body": body,
        "labels": ["module-submission"]
    });

    let resp = client
        .post(&create_issue_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "kam-cli")
        .json(&issue_body)
        .send()
        .map_err(|e| KamError::UploadFailed(format!("create issue failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(KamError::UploadFailed(format!(
            "create issue failed: HTTP {}",
            resp.status()
        )));
    }

    Ok(())
}

/// Create GitHub release and upload asset
#[allow(dead_code)]
/// Create a GitHub release and upload an asset to it. This helper is optional and
/// primarily used by automation or CI; it's kept in the codebase for convenience.
fn create_github_release(
    owner: &str,
    repo_name: &str,
    module_id: &str,
    version: &str,
    package_path: &Path,
    token: Option<&str>,
) -> Result<(), KamError> {
    let github_token = std::env::var("GITHUB_TOKEN").ok();
    let kam_token = std::env::var("KAM_PUBLISH_TOKEN").ok();
    let token = token
        .or_else(|| github_token.as_deref())
        .or_else(|| kam_token.as_deref())
        .ok_or(KamError::InvalidConfig("GitHub token required".to_string()))?;

    let client = reqwest::blocking::Client::new();
    let create_release_url = format!(
        "https://api.github.com/repos/{}/{}/releases",
        owner, repo_name
    );
    let tag_name = format!("{}-{}", module_id, version);
    let body = json!({
        "tag_name": tag_name,
        "name": format!("Release {} {}", module_id, version),
        "body": format!("Auto release for {} {}", module_id, version),
        "draft": false,
        "prerelease": false
    });

    let resp = client
        .post(&create_release_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "kam-cli")
        .json(&body)
        .send()
        .map_err(|e| KamError::UploadFailed(format!("create release failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(KamError::UploadFailed(format!(
            "create release failed: HTTP {}",
            resp.status()
        )));
    }

    let release: serde_json::Value = resp
        .json()
        .map_err(|e| KamError::JsonError(e.to_string()))?;
    let upload_url = release["upload_url"]
        .as_str()
        .unwrap()
        .replace("{?name,label}", "");
    let file_name = package_path.file_name().unwrap().to_str().unwrap();

    let upload_resp = client
        .post(&format!("{}?name={}", upload_url, file_name))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/octet-stream")
        .body(fs::read(package_path)?)
        .send()
        .map_err(|e| KamError::UploadFailed(format!("upload failed: {}", e)))?;

    if !upload_resp.status().is_success() {
        return Err(KamError::UploadFailed(format!(
            "upload failed: HTTP {}",
            upload_resp.status()
        )));
    }

    Ok(())
}

// compute_index_path moved to crate::utils::compute_index_path()
