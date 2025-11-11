use clap::Args;
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::fs;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::sections::module::ModuleType;
use serde_json::json;
use chrono;

/// Arguments for the publish command
#[derive(Args, Debug)]
pub struct PublishArgs {
    /// Path to the project (default: current directory)
    #[arg(default_value = ".")]
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
    let kam_toml = KamToml::load_from_dir(project_path)?;
    let module_id = kam_toml.prop.id.clone();
    let version = kam_toml.prop.version.clone();
    let version_code = kam_toml.prop.versionCode;

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
                    if ext == "zip" {
                        found = Some(p);
                        break;
                    }
                }
            }
        }
        found.ok_or_else(|| KamError::Other(format!("Package not found in {}", output_dir.display())))?
    };

    println!("  {} Package: {}", "✓".green(), package_path.display());

    if args.dry_run {
        println!("  {} Dry-run: skipping upload", "•".yellow());
        return Ok(());
    }

    // Determine repository target:
    // Priority: CLI `--repo` (-r) -> kam.toml [mmrl.repo].repository -> none (print and exit)
    let repo = if let Some(r) = args.repo.as_ref().cloned() {
        r
    } else {
        // Use chained option access to avoid deep nesting
        let repo_from_kam = kam_toml
            .mmrl
            .as_ref()
            .and_then(|m| m.repo.as_ref())
            .and_then(|r| r.repository.as_ref())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if let Some(r) = repo_from_kam {
            r
        } else {
            println!("  {} No repository provided; package is available at: {}", "i".cyan(), package_path.display());
            return Ok(());
        }
    };

    // Local filesystem publish (file:// or plain path)
    if repo.starts_with("file://") || !repo.contains("://") {
        // Normalize path
        let dest = if repo.starts_with("file://") {
            PathBuf::from(repo.trim_start_matches("file://"))
        } else {
            PathBuf::from(repo)
        };

        fs::create_dir_all(&dest)?;

        // If the destination is itself a Kam module repo (module_type = repo),
        // treat it as a module repository: copy package into `packages/` and update an index.
        let maybe_toml = KamToml::load_from_dir(&dest).ok();
        if let Some(kt) = maybe_toml {
            if kt.kam.module_type == ModuleType::Repo {
                let packages_dir = dest.join("packages");
                fs::create_dir_all(&packages_dir)?;
                let dest_file = packages_dir.join(package_path.file_name().ok_or_else(|| KamError::Other("invalid package filename".to_string()))?);
                fs::copy(&package_path, &dest_file)?;

                // Update simple index.json (array of entries) under packages/index.json
                let index_path = packages_dir.join("index.json");
                let mut entries: Vec<serde_json::Value> = if index_path.exists() {
                    let s = std::fs::read_to_string(&index_path)?;
                    serde_json::from_str(&s).unwrap_or_else(|_| Vec::new())
                } else {
                    Vec::new()
                };

                let file_name = dest_file.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
                // Include both version (string) and versionCode (numeric) for compatibility.
                let entry = json!({
                    "id": module_id,
                    "version": version,
                    "versionCode": version_code,
                    "file": file_name,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                });
                entries.push(entry);
                let idx_s = serde_json::to_string_pretty(&entries).map_err(|e| KamError::Other(format!("json index serialize error: {}", e)))?;
                std::fs::write(&index_path, idx_s)?;

                println!("  {} Published to module repo: {}", "✓".green(), dest_file.display());
                return Ok(());
            }
        }

        // Fallback: plain directory copy
        let dest_file = dest.join(package_path.file_name().ok_or_else(|| KamError::Other("invalid package filename".to_string()))?);
        fs::copy(&package_path, &dest_file)?;
        println!("  {} Published to local repository: {}", "✓".green(), dest_file.display());
        return Ok(());
    }

    // Otherwise try HTTP upload (simple PUT)
    // If the repo is an HTTP(S) URL, append the package filename so we don't overwrite the repository root.
    let mut upload_target = repo.clone();
    if repo.starts_with("http://") || repo.starts_with("https://") {
        let file_name = package_path.file_name().ok_or_else(|| KamError::Other("invalid package filename".to_string()))?.to_string_lossy();
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
    let resp = req.send().map_err(|e| KamError::Other(format!("upload failed: {}", e)))?;
    if !resp.status().is_success() {
        return Err(KamError::Other(format!("upload failed: HTTP {}", resp.status())));
    }

    println!("  {} Published to {}", "✓".green(), repo);
    Ok(())
}
