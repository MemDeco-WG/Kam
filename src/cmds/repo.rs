use crate::errors::KamError;
use clap::{Args, Subcommand};
use serde::Deserialize;

mod cache;
mod download;
mod package_ops;
mod repo_search;
mod search;
mod status;
mod sync;

pub(crate) use cache::cache_root_dir;
pub use download::download_module_latest;
pub(crate) use package_ops::{download_targets, handle_repo_urls};
pub use search::search_local;
pub(crate) use status::handle_repo_status;
pub use sync::repo_sync_with_jobs;

const BASE_URL: &str = "https://modules.kernelsu.org";
const SEARCH_INDEX_PATH: &str = "/search-index.json";
const MODULE_JSON_PREFIX: &str = "/module/";

/// CLI args for the `kam repo` subcommand.
#[derive(Args, Debug, Clone)]
pub struct RepoArgs {
    /// Subcommands for repo (e.g., `repo sync`)
    #[command(subcommand)]
    pub command: Option<RepoCommand>,

    /// Pacman-style sync (download) flag (equivalent to pacman -S)
    #[arg(short = 'S', long = "sync")]
    pub sync: bool,

    /// Pacman-style search modifier (use with -S as '-Ss' to search or '-s' alone to search)
    #[arg(short = 's', long = "search")]
    pub search: bool,

    /// Suppress progress output (quiet mode)
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Positional targets: module IDs or search terms (used with -S / -s)
    #[arg(value_name = "TARGETS", num_args = 0..)]
    pub targets: Vec<String>,
}

/// Entrypoint for `kam repo` subcommand.
#[derive(Subcommand, Debug, Clone)]
pub enum RepoCommand {
    /// Sync (refresh) the index cache from remote (similar to pacman -Sy)
    Sync(SyncArgs),
    /// Show local module package index/cache status
    Status(StatusArgs),
    /// Search the module repository
    Search(SearchArgs),
    /// Show package metadata from the local module index
    Info(InfoArgs),
    /// List packages from the local module index
    List(ListArgs),
    /// Print cached package download URLs without downloading
    Url(UrlArgs),
    /// Download one or more modules into Kam's package cache
    Fetch(FetchArgs),
    /// Download one or more modules from the repository
    Download(DownloadArgs),
}

/// Arguments for `kam repo sync`.
#[derive(Args, Debug, Clone)]
pub struct SyncArgs {
    /// Force refresh even if cached
    #[arg(long = "force")]
    pub force: bool,

    /// Number of concurrent module index fetch jobs (overrides env KAM_REPO_CONCURRENCY)
    #[arg(short = 'j', long = "jobs", value_name = "N")]
    pub jobs: Option<usize>,

    /// Suppress progress output (quiet mode)
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

/// Arguments for `kam repo status`.
#[derive(Args, Debug, Clone)]
pub struct StatusArgs {
    /// Suppress labels and print compact key=value records
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

/// Arguments for `kam repo search`.
#[derive(Args, Debug, Clone)]
pub struct SearchArgs {
    /// Search terms to match against the remote module catalog
    #[arg(value_name = "QUERY", required = true, num_args = 1..)]
    pub query: Vec<String>,
}

/// Arguments for `kam repo info`.
#[derive(Args, Debug, Clone)]
pub struct InfoArgs {
    /// Module IDs to inspect
    #[arg(value_name = "MODULE", required = true, num_args = 1..)]
    pub modules: Vec<String>,
}

/// Arguments for `kam repo list`.
#[derive(Args, Debug, Clone)]
pub struct ListArgs {
    /// Optional query to filter package names, descriptions, summaries, or authors
    #[arg(value_name = "QUERY", num_args = 0..)]
    pub query: Vec<String>,
}

/// Arguments for `kam repo url`.
#[derive(Args, Debug, Clone)]
pub struct UrlArgs {
    /// Module IDs whose selected package URL should be printed
    #[arg(value_name = "MODULE", required = true, num_args = 1..)]
    pub modules: Vec<String>,

    /// Suppress module labels and print URLs only
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

/// Arguments for `kam repo fetch`.
#[derive(Args, Debug, Clone)]
pub struct FetchArgs {
    /// Module IDs to download into the local package cache
    #[arg(value_name = "MODULE", required = true, num_args = 1..)]
    pub modules: Vec<String>,

    /// Assume "yes" to all confirmation prompts
    #[arg(short = 'y', long = "yes")]
    pub assume_yes: bool,

    /// Suppress progress output (quiet mode)
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

/// Arguments for `kam repo download`.
#[derive(Args, Debug, Clone)]
pub struct DownloadArgs {
    /// Module IDs to download
    #[arg(value_name = "MODULE", required = true, num_args = 1..)]
    pub modules: Vec<String>,

    /// Assume "yes" to all confirmation prompts
    #[arg(short = 'y', long = "yes")]
    pub assume_yes: bool,

    /// Suppress progress output (quiet mode)
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

/// # Errors
/// Returns `KamError` when network, I/O, or parsing operations fail.
pub fn run(args: RepoArgs) -> Result<(), KamError> {
    run_with_modules_url(args, None)
}

/// # Errors
/// Returns `KamError` on failures performing network requests, parsing remote
/// payloads, or underlying I/O operations.
pub fn run_with_modules_url(args: RepoArgs, modules_url: Option<&str>) -> Result<(), KamError> {
    if let Some(command) = args.command {
        return match command {
            RepoCommand::Sync(sync_args) => {
                let base = effective_base_url(modules_url);
                repo_sync_with_jobs(&base, sync_args.force, sync_args.jobs, sync_args.quiet)
            }
            RepoCommand::Status(status_args) => {
                handle_repo_status(modules_url, args.quiet || status_args.quiet)
            }
            RepoCommand::Search(search_args) => handle_pacman_style(
                false,
                true,
                false,
                false,
                false,
                false,
                &search_args.query,
                false,
                modules_url,
                args.quiet,
            ),
            RepoCommand::Info(info_args) => {
                handle_repo_info(&info_args.modules, modules_url, args.quiet)
            }
            RepoCommand::List(list_args) => {
                handle_repo_list(&list_args.query.join(" "), modules_url, args.quiet)
            }
            RepoCommand::Url(url_args) => {
                handle_repo_urls(&url_args.modules, modules_url, args.quiet || url_args.quiet)
            }
            RepoCommand::Fetch(fetch_args) => handle_pacman_style(
                true,
                false,
                false,
                false,
                false,
                true,
                &fetch_args.modules,
                fetch_args.assume_yes,
                modules_url,
                fetch_args.quiet,
            ),
            RepoCommand::Download(download_args) => handle_pacman_style(
                true,
                false,
                false,
                false,
                false,
                false,
                &download_args.modules,
                download_args.assume_yes,
                modules_url,
                download_args.quiet,
            ),
        };
    }

    handle_pacman_style(
        args.sync,
        args.search,
        false,
        false,
        false,
        false,
        &args.targets,
        false,
        modules_url,
        args.quiet,
    )
}

/// # Errors
/// Returns `KamError` when network, I/O, or parsing operations fail.
#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
pub fn handle_pacman_style(
    sync: bool,
    search: bool,
    info: bool,
    list: bool,
    print_url: bool,
    fetch_only: bool,
    targets: &[String],
    yes: bool,
    modules_url: Option<&str>,
    quiet: bool,
) -> Result<(), KamError> {
    let assume_yes = yes || std::env::args().any(|a| a == "-y" || a == "--yes");
    let base = effective_base_url(modules_url);

    if search {
        let q = targets.join(" ");
        if q.is_empty() {
            return Err(KamError::CommandFailed(
                "Search requires a query e.g. `-Ss <term>`".into(),
            ));
        }
        return search_local(&q, &base);
    }

    if info {
        return handle_repo_info(targets, modules_url, quiet);
    }

    if list {
        return handle_repo_list(&targets.join(" "), modules_url, quiet);
    }

    if print_url {
        return handle_repo_urls(targets, modules_url, quiet);
    }

    if sync {
        return download_targets(targets, &base, assume_yes, quiet, fetch_only);
    }
    Ok(())
}

pub(crate) fn handle_repo_info(
    modules: &[String],
    modules_url: Option<&str>,
    quiet: bool,
) -> Result<(), KamError> {
    if modules.is_empty() {
        return Err(KamError::CommandFailed(
            "Info requires a module id, e.g. `-Si <moduleId>`".into(),
        ));
    }
    let base = effective_base_url(modules_url);
    for module_id in modules {
        cache::find_entry_by_name(&base, module_id)?;
        let md = download::read_module_detail_from_cache(module_id)?;
        if !quiet && modules.len() > 1 {
            crate::utils::Utils::section(module_id);
        }
        download::print_module_info(&md);
    }
    Ok(())
}

pub(crate) fn cached_entry_exists(base_url: &str, module_id: &str) -> Result<(), KamError> {
    cache::find_entry_by_name(base_url, module_id).map(|_| ())
}

pub(crate) fn read_cached_index(modules_url: Option<&str>) -> Result<Vec<SearchEntry>, KamError> {
    let base = effective_base_url(modules_url);
    cache::read_local_index(&base)
}

pub(crate) fn cached_module_update_metadata(
    module_id: &str,
) -> Result<Option<(String, String)>, KamError> {
    let Some(detail) = download::try_read_module_detail_from_cache(module_id, true)? else {
        return Ok(None);
    };
    let Some(version) = latest_release_version(detail.releases.as_ref()) else {
        return Ok(None);
    };
    Ok(Some((
        detail.module_name.unwrap_or_else(|| module_id.to_string()),
        version.to_string(),
    )))
}

fn latest_release_version(releases: Option<&Vec<Release>>) -> Option<&str> {
    releases?.iter().find_map(|release| {
        release
            .version
            .as_deref()
            .or(release.name.as_deref())
            .filter(|value| !value.trim().is_empty())
    })
}

pub(crate) fn handle_repo_list(
    query: &str,
    modules_url: Option<&str>,
    quiet: bool,
) -> Result<(), KamError> {
    let base = effective_base_url(modules_url);
    let mut entries = cache::read_local_index(&base)?;
    entries.sort_by_key(|entry| entry.name.to_ascii_lowercase());
    let query = query.trim();
    for entry in entries {
        if !query.is_empty() && repo_search::score_search_entry(&entry, query) < 0.60 {
            continue;
        }
        if quiet {
            println!("{}", entry.name);
        } else {
            let desc = entry
                .description
                .as_deref()
                .or(entry.summary.as_deref())
                .unwrap_or("");
            println!("{name} — {desc}", name = entry.name);
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchEntry {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) summary: Option<String>,
    pub(crate) authors: Option<String>,
    pub(crate) url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ModuleDetail {
    pub(super) module_id: String,
    pub(super) module_name: Option<String>,
    pub(super) url: Option<String>,
    pub(super) homepage_url: Option<String>,
    pub(super) authors: Option<Vec<Author>>,
    pub(super) releases: Option<Vec<Release>>,
    pub(super) summary: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Author {
    pub(super) name: String,
    pub(super) link: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Release {
    pub(super) name: Option<String>,
    #[serde(rename = "releaseAssets")]
    pub(super) assets: Option<Vec<Asset>>,
    pub(super) version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Asset {
    pub(super) name: String,
    pub(super) content_type: Option<String>,
    pub(super) download_url: String,
    pub(super) size: Option<u64>,
}

#[must_use]
pub fn effective_base_url(override_url: Option<&str>) -> String {
    if let Some(u) = override_url
        && !u.trim().is_empty()
    {
        return u.to_string();
    }
    if let Ok(env) = std::env::var("KAM_MODULES_URL")
        && !env.trim().is_empty()
    {
        return env;
    }
    if let Ok(cfg_home) = crate::utils::kam_home_dir() {
        let cfg = cfg_home.join("config.toml");
        if cfg.exists()
            && let Ok(content) = std::fs::read_to_string(&cfg)
            && let Ok(v) = toml::from_str::<toml::Value>(&content)
            && let Some(m) = v
                .get("modules")
                .and_then(|m| m.get("base_url"))
                .and_then(|x| x.as_str())
        {
            return m.to_string();
        }
    }
    BASE_URL.to_string()
}
