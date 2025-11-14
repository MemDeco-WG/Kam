use crate::cache::KamCache;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::sections::dependency::{Dependency, VersionSpec};
use crate::venv::KamVenv;
use clap::Args;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

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

    // Load project kam.toml
    let mut kam_toml = KamToml::load_from_dir(project_path)?;

    // Initialize cache
    let cache = KamCache::new()?;
    cache.ensure_dirs()?;

    let mut actual_version = args.version.clone();
    let lib_path_candidate = cache.lib_module_path(library, &actual_version);
    let lib_path = if lib_path_candidate.join("kam.toml").exists() && !args.force {
        if actual_version == "latest" {
            find_latest_version(&cache, library)?
        } else {
            lib_path_candidate
        }
    } else {
        actual_version = fetch_library(&cache, library, &args.version, args.repo.as_deref())?;
        cache.lib_module_path(library, &actual_version)
    };

    // Extract library metadata
    let lib_info = extract_library_info(&lib_path)?;

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
            if let Ok(entries) = fs::read_dir(lib_path.join("bin")) {
                for entry in entries.flatten() {
                    if let Some(name_str) = entry.file_name().to_str() {
                        venv.link_binary(&entry.path())?;
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

/// Find the latest version of a library in cache
fn find_latest_version(cache: &KamCache, library: &str) -> Result<PathBuf, KamError> {
    let lib_dir = cache.lib_dir();
    let pattern = format!("{}-", library);

    let mut versions: Vec<(String, PathBuf)> = Vec::new();

    if let Ok(entries) = fs::read_dir(&lib_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with(&pattern) && path.join("kam.toml").exists() {
                    let version = name.trim_start_matches(&pattern);
                    versions.push((version.to_string(), path));
                }
            }
        }
    }

    if versions.is_empty() {
        return Err(KamError::LibraryNotFound(format!(
            "No versions of '{}' found in cache",
            library
        )));
    }

    // Sort versions (simple string sort for now, could be improved)
    versions.sort_by(|a, b| b.0.cmp(&a.0));

    Ok(versions[0].1.clone())
}

/// Fetch library from repository
fn fetch_library(
    cache: &KamCache,
    library: &str,
    version: &str,
    repo: Option<&str>,
) -> Result<String, KamError> {
    println!("  {} Fetching {}@{}", "→".cyan(), library, version);

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
                    let actual_version = if version == "latest" {
                        meta.get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("latest")
                    } else {
                        version
                    };

                    // Copy package file
                    if let Some(package_file) = meta.get("package").and_then(|p| p.as_str()) {
                        let source = repo_path.join("packages").join(package_file);
                        if source.exists() {
                            let dest = cache.lib_module_path(library, actual_version);
                            fs::create_dir_all(&dest)?;

                            // Extract package
                            extract_package(&source, &dest)?;

                            println!("  {} Fetched from local repo", "✓".green());
                            return Ok(actual_version.to_string());
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

    Err(KamError::LibraryNotFound(format!(
        "Could not fetch {}@{} from any source",
        library, version
    )))
}

/// Compute index path based on library name (similar to cargo's index structure)
fn compute_index_path(index_base: &Path, library: &str) -> PathBuf {
    let name_lower = library.to_lowercase();
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
) -> Result<String, KamError> {
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

                            let dest = cache.lib_module_path(library, version);
                            fs::create_dir_all(&dest)?;
                            extract_package(&temp_path, &dest)?;

                            // Clean up temp file
                            let _ = fs::remove_file(&temp_path);

                            println!("  {} Downloaded and extracted", "✓".green());
                            return Ok(version.to_string());
                        }
                    }
                }
            }
        }
    }

    Err(KamError::LibraryNotFound(format!(
        "No suitable asset found for {}@{} in GitHub release",
        library, version
    )))
}

/// Extract library information from installed module
fn extract_library_info(lib_path: &Path) -> Result<LibraryInfo, KamError> {
    let kam_toml_path = lib_path.join("kam.toml");

    if kam_toml_path.exists() {
        let kam_toml = KamToml::load_from_dir(lib_path)?;

        Ok(LibraryInfo {
            version: kam_toml.prop.version,
            versionCode: kam_toml.prop.versionCode,
        })
    } else {
        // Fallback: extract version from directory name
        let version = lib_path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.split('-').last())
            .unwrap_or("unknown")
            .to_string();

        Ok(LibraryInfo {
            version,
            versionCode: 0,
        })
    }
}
