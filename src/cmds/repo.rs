use crate::errors::KamError;
use clap::{Args, Subcommand};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;

mod cache;
mod download;
mod repo_search;
mod search;
mod sync;

pub(crate) use cache::cache_root_dir;
pub use download::download_module_latest;
pub use search::search_local;
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
    /// Search the module repository
    Search(SearchArgs),
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

/// Arguments for `kam repo search`.
#[derive(Args, Debug, Clone)]
pub struct SearchArgs {
    /// Search terms to match against the remote module catalog
    #[arg(value_name = "QUERY", required = true, num_args = 1..)]
    pub query: Vec<String>,
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
            RepoCommand::Search(search_args) => handle_pacman_style(
                false,
                true,
                &search_args.query,
                false,
                modules_url,
                args.quiet,
            ),
            RepoCommand::Download(download_args) => handle_pacman_style(
                true,
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
        &args.targets,
        false,
        modules_url,
        args.quiet,
    )
}

/// # Errors
/// Returns `KamError` when network, I/O, or parsing operations fail.
#[allow(clippy::fn_params_excessive_bools)]
pub fn handle_pacman_style(
    sync: bool,
    search: bool,
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

    if sync {
        return download_targets(targets, &base, assume_yes, quiet);
    }
    Ok(())
}

fn download_targets(
    targets: &[String],
    base_url: &str,
    assume_yes: bool,
    quiet: bool,
) -> Result<(), KamError> {
    if targets.is_empty() {
        return Err(KamError::CommandFailed(
            "Download requires a module id(s), e.g. `-S <moduleId>`".into(),
        ));
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| KamError::FetchFailed(format!("Failed to build HTTP client: {e}")))?;

    for module_id in targets {
        if !quiet {
            crate::utils::Utils::section(&trf!("repo.download", module_id));
        }
        match cache::find_entry_by_name(base_url, module_id)
            .and_then(|_| download::read_module_detail_from_cache(module_id))
        {
            Ok(md) => {
                download::process_module_download(&md, module_id, &client, assume_yes, quiet)?;
            }
            Err(KamError::PackageNotFound(_)) => {
                handle_missing_module(module_id, base_url, &client, assume_yes, quiet)?;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn handle_missing_module(
    module_id: &str,
    base_url: &str,
    client: &Client,
    assume_yes: bool,
    quiet: bool,
) -> Result<(), KamError> {
    crate::utils::Utils::warn(trf!("repo.module_not_found_showing_similar", module_id));
    if let Some(selected_module) = search::search_local_interactive(module_id, base_url)? {
        crate::utils::Utils::info(trf!("repo.selected_module", selected_module));
        let md = download::read_module_detail_from_cache(&selected_module)?;
        download::process_module_download(&md, &selected_module, client, assume_yes, quiet)?;
    } else {
        crate::utils::Utils::info(crate::i18n::tr("repo.skipped_selection"));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub(super) struct SearchEntry {
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) authors: Option<String>,
    pub(super) url: Option<String>,
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
