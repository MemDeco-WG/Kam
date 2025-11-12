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
/// # Sync kam dependencies
/// kam sync
///
/// # Sync including dev dependencies
/// kam sync --dev
/// ```

use clap::Args;
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::fs;
use crate::cache::KamCache;
use crate::types::source::Source;
use crate::types::modules::KamModule;
use crate::types::modules::ModuleBackend;
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
}

/// Ensure a dependency module exists in the cache. Returns `Ok(true)` if a new
/// placeholder was created, `Ok(false)` if it already existed.
fn ensure_module_synced(
    cache: &KamCache,
    dep: &crate::types::kam_toml::sections::Dependency,
) -> Result<bool, KamError> {
    // Resolve a concrete version string to use for cache paths. If the
    // dependency specifies an exact versionCode, use it. If it specifies a
    // range, try to choose the highest cached version matching the range.
    // If nothing is available, fall back to the lower bound or 0.
    use crate::types::kam_toml::sections::VersionSpec;

    let version = match &dep.versionCode {
        Some(VersionSpec::Exact(v)) => v.to_string(),
        Some(VersionSpec::Range(s)) => {
            // parse a range like "[1000,2000)" or "[1000,)" or "(,2000]"
            // extract min and max if present
            let s = s.trim();
            let min_incl = s.starts_with('[');
            let max_incl = s.ends_with(']');
            let inner = s.trim_start_matches('[').trim_start_matches('(').trim_end_matches(']').trim_end_matches(')');
            let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
            let min_opt = if parts.get(0).map(|p| !p.is_empty()).unwrap_or(false) {
                parts[0].parse::<i64>().ok()
            } else { None };
            let max_opt = if parts.len() > 1 && parts[1].len() > 0 { parts[1].parse::<i64>().ok() } else { None };

            // list cached versions for id
            let mut candidates: Vec<i64> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(cache.lib_dir()) {
                for e in entries.flatten() {
                    if let Some(name) = e.file_name().to_str() {
                        if let Some(rest) = name.strip_prefix(&format!("{}-", dep.id)) {
                            if let Ok(n) = rest.parse::<i64>() {
                                // test against range
                                let mut ok = true;
                                if let Some(minv) = min_opt { ok = ok && (if min_incl { n >= minv } else { n > minv }); }
                                if let Some(maxv) = max_opt { ok = ok && (if max_incl { n <= maxv } else { n < maxv }); }
                                if ok { candidates.push(n); }
                            }
                        }
                    }
                }
            }

            if let Some(max_match) = candidates.into_iter().max() {
                max_match.to_string()
            } else if let Some(minv) = min_opt { minv.to_string() } else { "0".to_string() }
        }
        None => "0".to_string(),
    };

    let module_path = cache.lib_module_path(&dep.id, &version);

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

    // Try network sources using KamToml's effective source and the new Source/KamModule
    let source_base = crate::types::kam_toml::KamToml::get_effective_source(dep);
    let candidates = vec![
        format!("{}/{}", source_base.trim_end_matches('/'), zip_name),
        format!("{}/releases/download/{}/{}", source_base.trim_end_matches('/'), version, zip_name),
        format!("{}/raw/main/{}", source_base.trim_end_matches('/'), zip_name),
    ];

    for url in candidates {
        // Parse the candidate into a Source and attempt to install into cache using KamModule
        match Source::parse(&url) {
            Ok(src) => {
                let module = KamModule::new(crate::types::kam_toml::KamToml::default(), Some(src));
                match install_backend_into_cache(&module, cache) {
                    Ok(_dst) => {
                        let marker = module_path.join(".synced");
                        fs::write(marker, format!("Synced: {} @ {} ({})", dep.id, version, url))?;
                        return Ok(true);
                    }
                    Err(_e) => {
                        // try next candidate
                        continue;
                    }
                }
            }
            Err(_) => continue,
        }
    }

    // If we reach here, we couldn't obtain the module
    Err(KamError::FetchFailed(format!("Failed to fetch module '{}@{}' from local repo or source", dep.id, version)))
}

/// Install a ModuleBackend into the provided cache via the trait.
///
/// This small adapter centralizes the place where callers depend on the
/// trait rather than the concrete `KamModule` type. It simply delegates to
/// the backend's `install_into_cache` and exists to make call-sites accept
/// `impl ModuleBackend` / `&dyn ModuleBackend` more explicitly.
fn install_backend_into_cache(backend: &impl ModuleBackend, cache: &KamCache) -> Result<std::path::PathBuf, KamError> {
    backend.install_into_cache(cache)
}

/// Run the sync command
///
/// ## Steps
///
/// 1. Load `kam.toml` configuration
/// 2. Ensure cache directories exist
/// 3. Ensure virtual environment exists
/// 4. Resolve dependency groups
/// 5. Create symbolic links to cached modules
pub fn run(args: SyncArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);

    // Load kam.toml
    let kam_toml = crate::types::kam_toml::KamToml::load_from_dir(project_path)?;
    println!("  {} {}", "✓".green(), format!("Loaded kam.toml for '{}'", kam_toml.prop.id).dimmed());

    // Initialize cache, honoring project-local `.env` KAM_CACHE_ROOT.
    // If the value in `.env` is a relative path, resolve it relative to the
    // project directory (the location of the `.env`), using a canonicalized
    // absolute base when possible. This allows `.env` to contain `./.kam`.
    fn read_project_env_value(project_path: &Path, key: &str) -> Option<String> {
        let env_file = project_path.join(".env");
        if !env_file.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&env_file).ok()?;
        content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .find_map(|line| {
                line.find('=').and_then(|pos| {
                    let k = line[..pos].trim();
                    if k != key {
                        return None;
                    }
                    let mut val = line[pos + 1..].trim().to_string();
                    // strip optional surrounding quotes
                    if (val.starts_with('"') && val.ends_with('"')) || (val.starts_with('\'') && val.ends_with('\'')) {
                        if val.len() >= 2 {
                            val = val[1..val.len() - 1].to_string();
                        }
                    }
                    Some(val)
                })
            })
    }

    // Initialize cache, honoring project-local `.env` KAM_CACHE_ROOT.
    // If the value in `.env` is a relative path, resolve it relative to the
    // project directory (the location of the `.env`), using a canonicalized
    // absolute base when possible. This allows `.env` to contain `./.kam`.
    let cache = if let Some(root_val) = read_project_env_value(project_path, "KAM_CACHE_ROOT") {
        let p = PathBuf::from(root_val);
        // Try to get an absolute base path for the project. If the project
        // path cannot be canonicalized (missing), fall back to current_dir().
        let base = match project_path.canonicalize() {
            Ok(abs) => abs,
            Err(_) => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        let abs = if p.is_absolute() { p } else { base.join(p) };
        KamCache::with_root(abs)?
    } else {
        KamCache::new()?
    };
    cache.ensure_dirs()?;
    println!("  {} {}", "✓".green(), format!("Cache: {}", cache.root().display()).dimmed());
    println!();

    // Ensure virtual environment exists and is up-to-date.
    // Per project policy, `kam sync` should always ensure the venv is present
    // and refreshed. The dedicated `kam venv` command remains available for
    // manual management.
    println!();
    println!("{} Ensuring virtual environment is present...", "→".cyan());
    let venv_path = project_path.join(".kam_venv");
    let venv_type = if args.dev { VenvType::Development } else { VenvType::Runtime };
    if venv_path.exists() {
        // recreate to ensure it's the latest
        fs::remove_dir_all(&venv_path)?;
    }
    let venv = KamVenv::create(&venv_path, venv_type)
        .map_err(|e| KamError::VenvCreateFailed(format!("Venv error: {}", e)))?;
    println!("  {} Created/updated at: {}", "✓".green(), venv.root().display());
    let maybe_venv: Option<KamVenv> = Some(venv);

    println!("{}", "Synchronizing dependencies...".bold().cyan());
    println!();

    // Resolve dependencies
    let resolved = kam_toml.resolve_dependencies().map_err(|e| KamError::FetchFailed(format!("dependency resolution failed: {}", e)))?;

    // Determine which groups to sync
    let groups_to_sync = if args.dev {
        vec!["kam", "dev"]
    } else {
        vec!["kam"]
    };

    // Process each group
    let mut total_synced = 0;
    for group_name in groups_to_sync {
        let group = match resolved.get(group_name) {
            Some(g) => g,
            None => continue,
        };

        println!("{} {} dependencies:", "Syncing".bold(), group_name.yellow());

        for dep in &group.dependencies {
            // Use versionCode for dependency selection (fall back to 0 when absent)
            let version_code = dep.versionCode.as_ref().map(|v| v.as_display()).unwrap_or_else(|| "0".to_string());
            println!("  {} {}@{}",
                "→".cyan(),
                dep.id.bold(),
                version_code.dimmed()
            );

            // Delegate the (simulated) cache write to a helper to keep the
            // loop body small and focused on presentation.
            if ensure_module_synced(&cache, dep)? {
                total_synced += 1;
            }

            // If a venv was requested, link the library into it
            if let Some(venv) = &maybe_venv {
                let ver = dep.versionCode.as_ref().map(|v| v.as_display()).unwrap_or_else(|| "0".to_string());
                match venv.link_library(&dep.id, &ver, &cache) {
                    Ok(_) => println!("  {} Linked {}@{} into venv", "✓".green(), dep.id, ver),
                    Err(e) => println!("  {} Failed to link {}@{}: {}", "!".yellow(), dep.id, ver, e),
                }
            }
        }

        println!();
    }

    println!("{} Synced {} dependencies", "✓".green().bold(), total_synced.to_string().green().bold());

    // Print activation instructions for the always-managed venv
    println!();
    println!("{} To activate the virtual environment:", "•".dimmed());
    println!("  {}: source .kam_venv/activate", "Unix".yellow());
    println!("  {}: .kam_venv\\activate.bat", "Windows".yellow());
    println!("  {}: .kam_venv\\activate.ps1", "PowerShell".yellow());

    Ok(())
}
