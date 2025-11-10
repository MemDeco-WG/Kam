/// # Kam Sync Command
/// 
/// Synchronize dependencies similar to `uv sync`, creating symbolic links.
/// 
/// ## Functionality
/// 
/// - Resolves dependencies from `kam.toml`
/// - Downloads and caches modules
/// - Creates symbolic links to cached modules
/// - Supports dev dependencies with `--dev` flag
/// 
/// ## Example
/// 
/// ```bash
/// # Sync normal dependencies
/// kam sync
/// 
/// # Sync including dev dependencies
/// kam sync --dev
/// ```

use clap::Args;
use colored::Colorize;
use std::path::Path;
use std::fs;
use crate::cache::KamCache;
use crate::types::kam_toml::KamToml;
use crate::venv::{KamVenv, VenvType};
use crate::errors::KamError;

/// Arguments for the sync command
#[derive(Args, Debug)]
pub struct SyncArgs {
    /// Path to the project (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,
    
    /// Include dev dependencies
    #[arg(long)]
    pub dev: bool,
    
    /// Create virtual environment
    #[arg(long)]
    pub venv: bool,
}

/// Ensure a dependency module exists in the cache. Returns `Ok(true)` if a new
/// placeholder was created, `Ok(false)` if it already existed.
fn ensure_module_synced(
    cache: &KamCache,
    dep: &crate::types::kam_toml::sections::dependency::Dependency,
) -> Result<bool, KamError> {
    use std::io::Write as _;
    let version = dep.version.as_deref().unwrap_or("latest");
    let module_path = cache.lib_module_path(&dep.id, version);

    // Already cached
    if module_path.exists() {
        return Ok(false);
    }

    // Ensure parent exists
    fs::create_dir_all(&module_path)?;

    // Candidate local repo locations
    let mut local_candidates = Vec::new();
    if let Some(p) = std::env::var_os("KAM_LOCAL_REPO") {
        local_candidates.push(std::path::PathBuf::from(p));
    }
    if let Ok(cwd) = std::env::current_dir() {
        local_candidates.push(cwd.join("tmpl").join("repo_templeta"));
        local_candidates.push(cwd.join("repo_templeta"));
    }

    let zip_name = format!("{}-{}.zip", dep.id, version);

    // Try local candidates first
    for repo_root in local_candidates {
        let candidate = repo_root.join(&zip_name);
        if candidate.exists() {
            // Extract zip into module_path
            let file = std::fs::File::open(&candidate)?;
            let mut archive = zip::ZipArchive::new(file)?;
            archive.extract(&module_path).map_err(KamError::from)?;
            let marker = module_path.join(".synced");
            fs::write(marker, format!("Synced: {} @ {} (local)", dep.id, version))?;
            return Ok(true);
        }
    }

    // Try network sources using KamToml's effective source
    let source = crate::types::kam_toml::KamToml::get_effective_source(dep);
    let candidates = vec![
        format!("{}/{}", source.trim_end_matches('/'), zip_name),
        format!("{}/releases/download/{}/{}", source.trim_end_matches('/'), version, zip_name),
        format!("{}/raw/main/{}", source.trim_end_matches('/'), zip_name),
    ];

    for url in candidates {
        // Attempt download
        match reqwest::blocking::get(&url) {
            Ok(resp) => {
                if resp.status().is_success() {
                    let bytes = resp.bytes().map_err(|e| KamError::Other(format!("download error: {}", e)))?;
                    // Write to a temp file then extract
                    let mut tmp = tempfile::NamedTempFile::new()?;
                    tmp.write_all(&bytes)?;
                    tmp.flush()?;
                    let f = tmp.reopen()?;
                    let mut archive = zip::ZipArchive::new(f)?;
                    archive.extract(&module_path).map_err(KamError::from)?;
                    let marker = module_path.join(".synced");
                    fs::write(marker, format!("Synced: {} @ {} ({})", dep.id, version, url))?;
                    return Ok(true);
                }
            }
            Err(_) => continue,
        }
    }

    // If we reach here, we couldn't obtain the module
    Err(KamError::Other(format!("Failed to fetch module '{}@{}' from local repo or source", dep.id, version)))
}

/// Run the sync command
/// 
/// ## Steps
/// 
/// 1. Load `kam.toml` configuration
/// 2. Resolve dependency groups
/// 3. Ensure cache directories exist
/// 4. Create symbolic links to cached modules
pub fn run(args: SyncArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);
    
    println!("{}", "Synchronizing dependencies...".bold().cyan());
    println!();
    
    // Load kam.toml
    let kam_toml = KamToml::load_from_dir(project_path)?;
    println!("  {} {}", "✓".green(), format!("Loaded kam.toml for '{}'", kam_toml.prop.id).dimmed());

    // Resolve dependencies
    let resolved = kam_toml
        .resolve_dependencies()
        .map_err(|e| KamError::Other(format!("dependency resolution failed: {}", e)))?;
    
    // Determine which groups to sync
    let groups_to_sync = if args.dev {
        vec!["normal", "dev"]
    } else {
        vec!["normal"]
    };
    
    // Initialize cache
    let cache = KamCache::new()?;
    cache.ensure_dirs()?;
    println!("  {} {}", "✓".green(), format!("Cache: {}", cache.root().display()).dimmed());
    println!();
    
    // Prepare virtual environment if requested (create now so we can link as we sync)
    let mut maybe_venv: Option<KamVenv> = None;
    if args.venv {
        println!();
        println!("{} Creating virtual environment...", "→".cyan());
        let venv_path = project_path.join(".kam-venv");
        let venv_type = if args.dev { VenvType::Development } else { VenvType::Runtime };
        if venv_path.exists() {
            fs::remove_dir_all(&venv_path)?;
        }
        let venv = KamVenv::create(&venv_path, venv_type)
            .map_err(|e| KamError::Other(format!("Venv error: {}", e)))?;
        println!("  {} Created at: {}", "✓".green(), venv.root().display());
        println!();
        println!("{} To activate the virtual environment:", "•".dimmed());
        println!("  {}: source .kam-venv/activate", "Unix".yellow());
        println!("  {}: .kam-venv\\activate.bat", "Windows".yellow());
        println!("  {}: .kam-venv\\activate.ps1", "PowerShell".yellow());
        maybe_venv = Some(venv);
    }

    // Process each group
    let mut total_synced = 0;
    for group_name in groups_to_sync {
        if let Some(group) = resolved.get(group_name) {
            println!("{} {} dependencies:", "Syncing".bold(), group_name.yellow());
            
            for dep in &group.dependencies {
                let version = dep.version.as_deref().unwrap_or("latest");
                println!("  {} {}{}{}", 
                    "→".cyan(), 
                    dep.id.bold(), 
                    "@".dimmed(),
                    version.dimmed()
                );

                // Delegate the (simulated) cache write to a helper to keep the
                // loop body small and focused on presentation.
                if ensure_module_synced(&cache, dep)? {
                    total_synced += 1;
                }

                // If a venv was requested, link the library into it
                if let Some(venv) = &maybe_venv {
                    let ver = dep.version.as_deref().unwrap_or("latest");
                    match venv.link_library(&dep.id, ver, &cache) {
                        Ok(_) => println!("  {} Linked {}@{} into venv", "✓".green(), dep.id, ver),
                        Err(e) => println!("  {} Failed to link {}@{}: {}", "!".yellow(), dep.id, ver, e),
                    }
                }
            }
            
            println!();
        }
    }
    
    println!("{} Synced {} dependencies", "✓".green().bold(), total_synced.to_string().green().bold());
    
    // Create virtual environment if requested
    if args.venv {
        println!();
        println!("{}", "Creating virtual environment...".bold().cyan());
        
        let venv_path = project_path.join(".kam-venv");
        let venv_type = if args.dev {
            VenvType::Development
        } else {
            VenvType::Runtime
        };
        
        // Remove existing venv if it exists
        if venv_path.exists() {
            fs::remove_dir_all(&venv_path)?;
        }

        let _venv = KamVenv::create(&venv_path, venv_type)
            .map_err(|e| KamError::Other(format!("Venv error: {}", e)))?;
        println!("  {} Created at: {}", "✓".green(), venv_path.display());
        println!();
        println!("{}", "To activate the virtual environment:".dimmed());
        println!("  {}: source .kam-venv/activate", "Unix".yellow());
        println!("  {}: .kam-venv\\activate.bat", "Windows".yellow());
        println!("  {}: .kam-venv\\activate.ps1", "PowerShell".yellow());
    }
    
    Ok(())
}
