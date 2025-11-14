use crate::cache::KamCache;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::sections::dependency::{Dependency, VersionSpec};
use crate::types::source::Source;
use crate::types::modules::ModuleBackend;

use crate::venv::KamVenv;
use clap::Args;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile;

/// Arguments for the add command
#[derive(Args, Debug)]
pub struct AddArgs {
    /// Library module ID to add or workspace member path
    pub library: Option<String>,

    /// Version of the library (default: latest)
    #[arg(short, long, default_value = "latest")]
    pub version: String,

    /// Path to the project (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Add as development dependency
    #[arg(short, long)]
    pub dev: bool,

    /// Force download even if already cached
    #[arg(short, long)]
    pub force: bool,

    /// Don't link to virtual environment
    #[arg(long)]
    pub no_link: bool,

    /// Source repository URL or path
    #[arg(short = 'r', long)]
    pub repo: Option<String>,

    /// Add workspace member instead of dependency
    #[arg(long)]
    pub workspace: bool,
}

/// Run the add command
pub fn run(args: AddArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);

    if args.workspace {
        return add_workspace_member(&args, project_path);
    }

    let library = args.library.as_deref().unwrap_or_else(|| {
        eprintln!("Error: library ID is required when not using --workspace");
        std::process::exit(1);
    });

    println!(
        "{} Adding library: {}@{}",
        "→".cyan(),
        library.bold(),
        args.version
    );



    // Initialize cache
    let cache = KamCache::new()?;
    cache.ensure_dirs()?;



    let (actual_version, mut kam_toml) = fetch_library(&cache, library, &args.version, args.repo.as_deref())?;

    // Extract library metadata
    let lib_info = LibraryInfo {
        version: kam_toml.prop.version.clone(),
        versionCode: kam_toml.prop.versionCode,
    };

    // Create dependency entry
    let dependency_entry = Dependency {
        id: library.to_string(),
        versionCode: Some(VersionSpec::Exact(lib_info.versionCode)),
        source: args.repo.clone(),
    };

    if args.dev {
        println!("  {} Adding to dev dependencies", "•".dimmed());
        let devs = kam_toml
            .kam
            .dependency
            .get_or_insert_with(Default::default)
            .dev
            .get_or_insert_with(Vec::new);

        // Check if already exists
        if !devs.iter().any(|d| d.id == dependency_entry.id) {
            devs.push(dependency_entry);
        }
    } else {
        println!("  {} Adding to runtime dependencies", "•".dimmed());
        let deps = kam_toml
            .kam
            .dependency
            .get_or_insert_with(Default::default)
            .kam
            .get_or_insert_with(Vec::new);

        // Check if already exists
        if !deps.iter().any(|d| d.id == dependency_entry.id) {
            deps.push(dependency_entry);
        }
    }

    // Save updated kam.toml
    kam_toml.write_to_dir(project_path)?;
    println!("  {} Updated kam.toml", "✓".green());

    // Link to virtual environment if requested
    if !args.no_link {
        let venv_path = project_path.join(".kam_venv");
        if venv_path.exists() {
            let venv = KamVenv::load(&venv_path)?;

            // Link binaries
            if let Ok(entries) = fs::read_dir(cache.bin_dir()) {
                for entry in entries.flatten() {
                    if let Some(name_str) = entry.file_name().to_str() {
                        venv.link_binary(cache.bin_path(name_str).as_path())?;
                        println!("  {} Linked binary: {}", "✓".green(), name_str);
                    }
                }
            }

            // Link libraries
            venv.link_library(library, &actual_version, &cache)?;
            println!("  {} Linked library to venv", "✓".green());
        } else {
            println!(
                "  {} No virtual environment found, skipping linking",
                "!".yellow()
            );
        }
    }

    println!(
        "{} Added {}@{}",
        "✓".green().bold(),
        library,
        lib_info.version
    );
    Ok(())
}

/// Add a workspace member
fn add_workspace_member(args: &AddArgs, project_path: &Path) -> Result<(), KamError> {
    let member_path = args.library.as_deref().unwrap_or(".");

    println!(
        "{} Adding workspace member: {}",
        "→".cyan(),
        member_path.bold()
    );

    // Load project kam.toml
    let mut kam_toml = KamToml::load_from_dir(project_path)?;

    // Ensure workspace section exists
    let workspace = kam_toml.kam.workspace.get_or_insert_with(Default::default);
    let members = workspace.members.get_or_insert_with(Vec::new);

    // Check if already exists
    if members.contains(&member_path.to_string()) {
        println!(
            "  {} Member '{}' already exists in workspace",
            "!".yellow(),
            member_path
        );
        return Ok(());
    }

    // Add member
    members.push(member_path.to_string());

    // Save updated kam.toml
    kam_toml.write_to_dir(project_path)?;
    println!("  {} Updated kam.toml", "✓".green());
    println!(
        "{} Added workspace member: {}",
        "✓".green().bold(),
        member_path
    );
    Ok(())
}

/// Library information extracted from module
#[derive(Debug)]
#[allow(non_snake_case)]
struct LibraryInfo {
    version: String,
    versionCode: i64, // Magisk module field naming style, do not change
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

/// Fetch library from repository
fn fetch_library(
    cache: &KamCache,
    library: &str,
    version: &str,
    repo: Option<&str>,
) -> Result<(String, KamToml), KamError> {
    println!("  {} Fetching {}@{}", "→".cyan(), library, version);

    let mut actual_version = version.to_string();

    // Check local repo first (KAM_LOCAL_REPO or specified repo)
    let local_repo = repo
        .map(PathBuf::from)
        .or_else(|| std::env::var("KAM_LOCAL_REPO").ok().map(PathBuf::from));

    if let Some(repo_path) = local_repo {
        if repo_path.exists() {
            // Try to find in local repo's index
            let index_path = repo_path.join("index");
            let lib_index = compute_index_path(&index_path, library);

            if lib_index.exists() {
                // Read library metadata
                let metadata_path = lib_index.join(format!("{}.json", version));
                if metadata_path.exists() {
                    let metadata = fs::read_to_string(&metadata_path)?;
                    let meta: serde_json::Value = serde_json::from_str(&metadata)
                        .map_err(|e| KamError::JsonError(e.to_string()))?;

                    // Get actual version
                    actual_version = if version == "latest" {
                        meta.get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("latest")
                            .to_string()
                    } else {
                        version.to_string()
                    };

                    // Copy package file
                    if let Some(package_file) = meta.get("package").and_then(|p| p.as_str()) {
                        let source = repo_path.join("packages").join(package_file);
                        if source.exists() {
                            let temp_dir = tempfile::tempdir()?;
                            let temp_path = temp_dir.path();

                            // Extract package to temp
                            extract_package(&source, temp_path)?;

                            // Load kam.toml
                            let kam_toml = KamToml::load_from_dir(temp_path)?;

                            // Install artifacts to cache
                            install_library_to_cache(temp_path, &cache)?;

                            // Update local index
                            update_local_cache_index(&cache, library, &actual_version, &kam_toml, package_file)?;

                            println!("  {} Fetched from local repo", "✓".green());
                            return Ok((actual_version.to_string(), kam_toml));
                        }
                    }
                }
            }
        }
    }

    // Try GitHub releases if repo URL is provided
    if let Some(repo_url) = repo {
        if repo_url.starts_with("https://github.com/") {
            return fetch_from_github(cache, library, version, repo_url);
        }
    }

    // Try network sources
    let source_base = repo.unwrap_or("https://github.com/MemDeco-WG/Kam-Index");
    let zip_name = format!("{}-{}.zip", library, actual_version);
    let candidates = vec![
        format!("{}/{}", source_base.trim_end_matches('/'), zip_name),
        format!(
            "{}/releases/download/{}/{}",
            source_base.trim_end_matches('/'),
            actual_version,
            zip_name
        ),
        format!(
            "{}/raw/main/{}",
            source_base.trim_end_matches('/'),
            zip_name
        ),
    ];

    for url in candidates {
        match Source::parse(&url) {
            Ok(src) => {
                let temp_dir = tempfile::tempdir()?;
                let temp_path = temp_dir.path();

                // Fetch to temp
                match src {
                    Source::Url { url } => {
                        let mut resp = reqwest::blocking::get(&url).map_err(|e| KamError::FetchFailed(format!("failed to download {}: {}", url, e)))?;
                        if !resp.status().is_success() {
                            continue;
                        }
                        let mut data = Vec::new();
                        resp.copy_to(&mut data).map_err(|e| KamError::FetchFailed(format!("read download body: {}", e)))?;
                        let file_path = temp_path.join("download.zip");
                        fs::write(&file_path, &data)?;
                        extract_package(&file_path, temp_path)?;
                    }
                    _ => continue,
                }

                // Load kam.toml
                let kam_toml = KamToml::load_from_dir(temp_path)?;

                // Install artifacts
                install_library_to_cache(temp_path, &cache)?;

                // Update index
                update_local_cache_index(&cache, library, &actual_version, &kam_toml, &zip_name)?;

                println!("  {} Fetched from network", "✓".green());
                return Ok((actual_version.clone(), kam_toml));
            }
            Err(_) => continue,
        }
    }

    Err(KamError::LibraryNotFound(format!(
        "Could not fetch {}@{} from any source",
        library, version
    )))
}

/// Extract package archive (zip or tar.gz)
fn extract_package(source: &Path, dest: &Path) -> Result<(), KamError> {
    let ext = source.extension().and_then(|e| e.to_str());

    match ext {
        Some("zip") => {
            let file = fs::File::open(source)?;
            let mut archive =
                zip::ZipArchive::new(file).map_err(|e| KamError::ExtractFailed(e.to_string()))?;
            archive
                .extract(dest)
                .map_err(|e| KamError::ExtractFailed(e.to_string()))?;
        }
        Some("gz") => {
            let file = fs::File::open(source)?;
            let gz = flate2::read::GzDecoder::new(file);
            let mut archive = tar::Archive::new(gz);
            archive
                .unpack(dest)
                .map_err(|e| KamError::ExtractFailed(e.to_string()))?;
        }
        _ => {
            return Err(KamError::UnsupportedFormat(format!(
                "Unsupported package format: {:?}",
                ext
            )));
        }
    }

    Ok(())
}

/// Fetch from GitHub releases
fn fetch_from_github(
    cache: &KamCache,
    library: &str,
    version: &str,
    repo_url: &str,
) -> Result<(String, KamToml), KamError> {
    // Parse GitHub repo from URL
    let parts: Vec<&str> = repo_url.trim_end_matches('/').split('/').collect();
    if parts.len() < 5 {
        return Err(KamError::InvalidUrl(format!(
            "Invalid GitHub URL: {}",
            repo_url
        )));
    }

    let owner = parts[3];
    let repo = parts[4];

    // Construct release API URL
    let api_url = if version == "latest" {
        format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            owner, repo
        )
    } else {
        format!(
            "https://api.github.com/repos/{}/{}/releases/tags/{}",
            owner, repo, version
        )
    };

    println!("  {} Fetching from GitHub: {}/{}", "→".cyan(), owner, repo);

    // Make request
    let client = reqwest::blocking::Client::new();
    let mut req = client
        .get(&api_url)
        .header("User-Agent", "kam-package-manager");

    // Add auth token if available
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("token {}", token));
    }

    let response = req
        .send()
        .map_err(|e| KamError::FetchFailed(e.to_string()))?;

    if !response.status().is_success() {
        return Err(KamError::FetchFailed(format!(
            "GitHub API returned {}",
            response.status()
        )));
    }

    let release: serde_json::Value = response
        .json()
        .map_err(|e| KamError::JsonError(e.to_string()))?;

    // Find asset matching library name
    if let Some(assets) = release.get("assets").and_then(|a| a.as_array()) {
        for asset in assets {
            if let Some(name) = asset.get("name").and_then(|n| n.as_str()) {
                if name.contains(library) && (name.ends_with(".zip") || name.ends_with(".tar.gz")) {
                    if let Some(download_url) =
                        asset.get("browser_download_url").and_then(|u| u.as_str())
                    {
                        // Download asset
                        println!("  {} Downloading: {}", "→".cyan(), name);

                        let response = client
                            .get(download_url)
                            .header("User-Agent", "kam-package-manager")
                            .send()
                            .map_err(|e| KamError::FetchFailed(e.to_string()))?;

                        if response.status().is_success() {
                            let bytes = response
                                .bytes()
                                .map_err(|e| KamError::FetchFailed(e.to_string()))?;

                            // Save to temp and extract
                            let temp_path = cache.root().join(name);
                            fs::write(&temp_path, bytes)?;

                            let temp_dir = tempfile::tempdir()?;
                            let temp_extract_path = temp_dir.path();
                            extract_package(&temp_path, temp_extract_path)?;

                            // Load kam.toml
                            let kam_toml = KamToml::load_from_dir(temp_extract_path)?;

                            // Install artifacts to cache
                            install_library_to_cache(temp_extract_path, &cache)?;

                            // Update local index
                            update_local_cache_index(&cache, library, &version, &kam_toml, name)?;

                            // Clean up temp file
                            let _ = fs::remove_file(&temp_path);

                            println!("  {} Downloaded and extracted", "✓".green());
                            return Ok((version.to_string(), kam_toml));
                        }
                    }
                }
            }
        }
    }

    Err(KamError::LibraryNotFound(format!(
        "Could not fetch {}@{} from any source",
        library, version
    )))
}

/// Compute index path based on module name (similar to cargo's index structureInstall backend into cache
/// Install backend into cache
fn install_backend_into_cache(
    backend: &impl ModuleBackend,
    cache: &KamCache,
) -> Result<PathBuf, KamError> {
    backend.install_into_cache(cache)
}

/// Install library artifacts to cache (lib, lib64, bin)
fn install_library_to_cache(
    temp_path: &Path,
    cache: &KamCache,
) -> Result<(), KamError> {
    // Copy lib to cache/lib
    let src_lib = temp_path.join("lib");
    if src_lib.exists() {
        copy_dir_all(&src_lib, &cache.lib_dir())?;
    }

    // Copy lib64 to cache/lib64
    let src_lib64 = temp_path.join("lib64");
    if src_lib64.exists() {
        copy_dir_all(&src_lib64, &cache.lib64_dir())?;
    }

    // Copy bin to cache/bin
    let src_bin = temp_path.join("bin");
    if src_bin.exists() {
        copy_dir_all(&src_bin, &cache.bin_dir())?;
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

/// Update local cache index for a published library
fn update_local_cache_index(
    cache: &KamCache,
    module_id: &str,
    version: &str,
    kam_toml: &KamToml,
    package_filename: &str,
) -> Result<(), KamError> {
    use serde_json::json;
    use chrono;

    // Create index directory structure based on module name
    let index_dir = cache.root().join("index");
    let module_index_path = compute_index_path(&index_dir, module_id);
    fs::create_dir_all(&module_index_path)?;

    // Create metadata JSON for this version
    let metadata = json!({
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
