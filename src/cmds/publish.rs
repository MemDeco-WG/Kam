use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;
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

    if !(module_type == &ModuleType::Library && args.repo.is_none()) {
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
                println!(
                    "  {} No repository provided; package is available at: {}",
                    "i".cyan(),
                    package_path.display()
                );
                return Ok(());
            }
        };

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

            // If the destination is itself a Kam module repo (module_type = repo),
            // treat it as a module repository: update index with metadata only.
            let maybe_toml = KamToml::load_from_dir(&dest).ok();
            if let Some(kt) = maybe_toml {
                if kt.kam.module_type == ModuleType::Repo {
                    // Update repo index with metadata
                    let package_filename = package_path.file_name().ok_or_else(|| {
                        KamError::InvalidFilename("invalid package filename".to_string())
                    })?.to_string_lossy().to_string();
                    update_repo_index(&dest, &module_id, &version, &kam_toml, &package_filename)?;

                    // Copy package to repo/packages directory
                    let packages_dir = dest.join("packages");
                    fs::create_dir_all(&packages_dir)?;
                    let dest_package =
                        packages_dir.join(package_path.file_name().ok_or_else(|| {
                            KamError::InvalidFilename("invalid package filename".to_string())
                        })?);
                    fs::copy(&package_path, &dest_package)?;
                    println!(
                        "  {} Published package to module repo: {}",
                        "✓".green(),
                        dest_package.display()
                    );

                    println!("  {} Published metadata to module repo index", "✓".green());

                    // Create GitHub release
                    // let (owner, repo_name) = get_github_repo_info()?;
                    // create_github_release(&owner, &repo_name, &module_id, &version, &package_path, args.token.as_deref())?;
                    // println!("  {} Created GitHub release for {}", "✓".green(), module_id);
                    return Ok(());
                }
            }

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
        // Special handling for library modules - publish to local repo or cache by default
        if let Ok(local_repo) = std::env::var("KAM_LOCAL_REPO") {
            println!(
                "  {} Publishing library metadata to local repo: {}",
                "→".cyan(),
                local_repo
            );
            // Update repo index with metadata only
            let repo_path = PathBuf::from(local_repo);
            let package_filename = package_path.file_name().ok_or_else(|| {
                KamError::InvalidFilename("invalid package filename".to_string())
            })?.to_string_lossy().to_string();
            update_repo_index(&repo_path, &module_id, &version, &kam_toml, &package_filename)?;

            // Copy package to repo/packages directory
            let packages_dir = repo_path.join("packages");
            fs::create_dir_all(&packages_dir)?;
            let dest_package = packages_dir.join(package_path.file_name().ok_or_else(|| {
                KamError::InvalidFilename("invalid package filename".to_string())
            })?);
            fs::copy(&package_path, &dest_package)?;
            println!(
                "  {} Published package to local repo: {}",
                "✓".green(),
                dest_package.display()
            );

            println!("  {} Published metadata to local repo index", "✓".green());

            // Create GitHub release
            // let (owner, repo_name) = get_github_repo_info()?;
            // create_github_release(&owner, &repo_name, &module_id, &version, &package_path, args.token.as_deref())?;
            // println!("  {} Created GitHub release for {}", "✓".green(), module_id);
            return Ok(());
        } else {
            println!("  {} Publishing library to local cache", "→".cyan());

            let cache = crate::cache::KamCache::new()?;
            cache.ensure_dirs()?;

            // Copy package to cache/lib directory
            let lib_dir = cache.lib_module_path(&module_id, &version);
            fs::create_dir_all(&lib_dir)?;

            // Extract the package to cache
            if package_path.to_str().unwrap().ends_with(".tar.gz") {
                let tar_gz = fs::File::open(&package_path)?;
                let dec = GzDecoder::new(tar_gz);
                let mut archive = tar::Archive::new(dec);
                archive
                    .unpack(&lib_dir)
                    .map_err(|e| KamError::ExtractFailed(e.to_string()))?;
            } else if package_path.extension().and_then(|e| e.to_str()) == Some("zip") {
                let file = fs::File::open(&package_path)?;
                let mut archive = zip::ZipArchive::new(file)
                    .map_err(|e| KamError::ExtractFailed(e.to_string()))?;
                archive
                    .extract(&lib_dir)
                    .map_err(|e| KamError::ExtractFailed(e.to_string()))?;
            } else {
                return Err(KamError::UnsupportedFormat(
                    "Library packages must be .zip or .tar.gz format".to_string(),
                ));
            }

            // Update local index
            let package_filename = package_path.file_name().ok_or_else(|| {
                KamError::InvalidFilename("invalid package filename".to_string())
            })?.to_string_lossy().to_string();
            update_local_cache_index(&cache, &module_id, &version, &kam_toml, &package_filename)?;

            println!(
                "  {} Published to local cache: {}",
                "✓".green(),
                lib_dir.display()
            );
            println!(
                "  {} Library can now be added with: kam add {}@{}",
                "i".cyan(),
                module_id,
                version_string
            );
            return Ok(());
        }
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
        "provides": kam_toml.kam.lib.as_ref()
            .and_then(|l| l.provides.as_ref())
            .unwrap_or(&Vec::new()),
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

/// Update local cache index for a published library
fn update_local_cache_index(
    cache: &crate::cache::KamCache,
    module_id: &str,
    version: &str,
    kam_toml: &KamToml,
    package_filename: &str,
) -> Result<(), KamError> {
    update_repo_index(cache.root(), module_id, version, kam_toml, package_filename)
}

/// Get GitHub repo owner and name from git remote
fn get_github_repo_info() -> Result<(String, String), KamError> {
    let repo = Repository::open(".")
        .map_err(|e| KamError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    let remote = repo
        .find_remote("origin")
        .map_err(|e| KamError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    let url = remote
        .url()
        .ok_or(KamError::InvalidConfig("No remote url".to_string()))?;
    let url_str = url.to_string();
    let re = Regex::new(r"github\.com[\/:]([^\/]+)\/([^\/]+?)(\.git)?$")
        .map_err(|e| KamError::InvalidConfig(format!("Regex error: {}", e)))?;
    if let Some(captures) = re.captures(&url_str) {
        let owner = captures.get(1).unwrap().as_str().to_string();
        let repo = captures.get(2).unwrap().as_str().to_string();
        Ok((owner, repo))
    } else {
        Err(KamError::InvalidConfig("Not a GitHub repo".to_string()))
    }
}

/// Create GitHub release and upload asset
fn create_github_release(
    owner: &str,
    repo: &str,
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
    let create_release_url = format!("https://api.github.com/repos/{}/{}/releases", owner, repo);
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

/// Compute index path based on module name (similar to cargo's index structure)
fn compute_index_path(index_base: &Path, module_name: &str) -> PathBuf {
    let name_lower = module_name.to_lowercase();
    let chars: Vec<char> = name_lower.chars().collect();

    match chars.len() {
        0 => index_base.to_path_buf(),
        1 => index_base.join("1").join(&name_lower),
        2 => index_base.join("2").join(&name_lower),
        3 => index_base
            .join("3")
            .join(&chars[0].to_string())
            .join(&name_lower),
        _ => {
            let prefix1 = chars[0..2].iter().collect::<String>();
            let prefix2 = chars[2..4].iter().collect::<String>();
            index_base.join(&prefix1).join(&prefix2).join(&name_lower)
        }
    }
}
