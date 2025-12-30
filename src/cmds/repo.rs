#![allow(dead_code)]
/*
Kam/src/cmds/repo.rs

Repository helper: implement pacman-style remote search and download:
- -Ss <term>  -> search remote catalog
- -S  <module> -> download latest release ZIP for module

This module is intentionally compact and uses the blocking reqwest client
and indicatif progress bars to provide a simple, user-friendly interface.

Notes:
*/

use crate::errors::KamError;
use crate::utils::Utils;
use clap::{Args, Subcommand};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use std::fs::File;
use std::io::IsTerminal;
use std::io::{Read, Write, stdin, stdout};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use std::time::Duration;

const BASE_URL: &str = "https://modules.kernelsu.org";
const SEARCH_INDEX_PATH: &str = "/search-index.json";
const MODULE_JSON_PREFIX: &str = "/module/"; // {id}.json appended

/// CLI args for the `kam repo` subcommand
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

    // `--modules-url` flag is handled at top-level (global). Local override removed to avoid duplicate long option.
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
}

#[derive(Args, Debug, Clone)]
pub struct SyncArgs {
    /// Force refresh even if cached
    #[arg(long = "force")]
    pub force: bool,

    /// Number of concurrent module index fetch jobs (overrides env KAM_REPO_CONCURRENCY)
    #[arg(short = 'j', long = "jobs", value_name = "N")]
    pub jobs: Option<usize>,

    // Per-sync `--modules-url` override removed; use the global/top-level setting instead.
    /// Suppress progress output (quiet mode)
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

/// Entrypoint for `kam repo` subcommand.
///
/// The original `run(args)` signature is preserved for backwards compatibility
/// and forwards to `run_with_modules_url` which accepts an optional
/// `modules_url` override (useful when callers want to forward the
/// top-level `--modules-url` value into the repo command).
pub fn run(args: RepoArgs) -> Result<(), KamError> {
    run_with_modules_url(args, None)
}

/// Entrypoint variant that accepts a global/top-level `modules_url` override.
///
/// Callers that wish to forward the top-level `--modules-url` flag into the
/// repo command should use this variant. The implementation respects the
/// override for both `repo sync` and pacman-style `-S`/`-s` handling.
pub fn run_with_modules_url(args: RepoArgs, modules_url: Option<String>) -> Result<(), KamError> {
    // If a nested repo subcommand is provided, handle it first (e.g., `repo sync`)
    if let Some(RepoCommand::Sync(sync_args)) = args.command {
        // Determine effective base URL (allow a forwarded override)
        let base = effective_base_url(modules_url.as_deref());
        // Use the CLI-provided jobs value (if any) to control concurrency for this sync.
        // Pass through the quiet flag so the sync implementation can suppress progress output.
        return repo_sync_with_jobs(&base, sync_args.force, sync_args.jobs, sync_args.quiet);
    }

    // fallback to existing pacman-style handling (-S / -s)
    handle_pacman_style(
        args.sync,
        args.search,
        args.targets,
        false,
        modules_url,
        args.quiet,
    )
}

#[derive(Debug, Deserialize)]
struct SearchEntry {
    pub name: String,
    pub description: Option<String>,
    pub summary: Option<String>,
    pub authors: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModuleDetail {
    pub module_id: String,
    pub module_name: Option<String>,
    pub url: Option<String>,
    pub homepage_url: Option<String>,
    pub authors: Option<Vec<Author>>,
    pub latest_release: Option<String>,
    pub latest_release_time: Option<String>,
    pub releases: Option<Vec<Release>>,
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Author {
    name: String,
    link: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Release {
    name: Option<String>,
    url: Option<String>,
    #[serde(rename = "releaseAssets")]
    release_assets: Option<Vec<Asset>>,
    version: Option<String>,
    version_code: Option<String>,
    created_at: Option<String>,
    published_at: Option<String>,
    updated_at: Option<String>,
    tag_name: Option<String>,
    is_prerelease: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Asset {
    name: String,
    content_type: Option<String>,
    download_url: String,
    download_count: Option<u64>,
    size: Option<u64>,
}

/// Handle pacman-style flags invoked at the top-level CLI.
///
/// Semantics:
/// - If `search` is true (e.g. user passed `-s` or `-Ss`) then treat the
///   joined `targets` as the search query and perform remote search.
/// - Else if `sync` is true (user passed `-S`) then treat `targets` as a list
///   of module ids to download (latest release ZIP).
pub fn handle_pacman_style(
    sync: bool,
    search: bool,
    targets: Vec<String>,
    yes: bool,
    modules_url: Option<String>,
    quiet: bool,
) -> Result<(), KamError> {
    // Respect explicit --yes/-y flag passed from CLI (as param) or environment arg fallback
    let assume_yes = yes || std::env::args().any(|a| a == "-y" || a == "--yes");
    // Determine effective base URL (override -> env -> config -> default)
    let base = effective_base_url(modules_url.as_deref());

    // Search mode (supports fuzzy search)
    if search {
        let q = targets.join(" ");
        if q.is_empty() {
            return Err(KamError::CommandFailed(
                "Search requires a query e.g. `-Ss <term>`".into(),
            ));
        }
        return search_remote(&q, &base);
    }

    // Download mode with interactive confirmation per-target
    if sync {
        if targets.is_empty() {
            return Err(KamError::CommandFailed(
                "Download requires a module id(s), e.g. `-S <moduleId>`".into(),
            ));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| KamError::FetchFailed(format!("Failed to build HTTP client: {}", e)))?;

        for module_id in targets.iter() {
            let section_title = crate::i18n::tr_fmt("repo.download", &[&module_id]);
            if !quiet {
                Utils::section(&section_title);
            }

            // Fetch module details
            let _md = match fetch_module_detail(&client, module_id, &base) {
                Ok(md) => {
                    // Process the module normally
                    process_module_download(&md, module_id, &client, assume_yes, quiet)?;
                    continue;
                }
                Err(KamError::FetchFailed(ref e))
                    if e.contains("404") || e.contains("not found") || e.contains("Not Found") =>
                {
                    // If the exact module wasn't found, suggest similar modules via interactive search
                    Utils::warn(&crate::i18n::tr_fmt(
                        "repo.module_not_found_showing_similar",
                        &[&module_id],
                    ));
                    if let Some(selected_module) = search_remote_interactive(module_id, &base)? {
                        Utils::info(&crate::i18n::tr_fmt(
                            "repo.selected_module",
                            &[&selected_module],
                        ));
                        // Now fetch the details for the selected module
                        let md = fetch_module_detail(&client, &selected_module, &base)?;
                        // Continue with the download process using the selected module
                        process_module_download(&md, &selected_module, &client, assume_yes, quiet)?;
                    } else {
                        Utils::info(crate::i18n::tr_key("repo.skipped_selection"));
                    }
                    continue;
                }
                Err(e) => return Err(e),
            };
        }

        return Ok(());
    }

    // Nothing to do
    Ok(())
}

/// Helper function to process module download with asset selection and confirmation
fn parse_confirm_input(input: &str, default_yes: bool) -> bool {
    let s = input.trim();
    if s.is_empty() {
        default_yes
    } else {
        let s = s.to_ascii_lowercase();
        s == "y" || s == "yes"
    }
}

fn process_module_download(
    md: &ModuleDetail,
    module_id: &str,
    client: &Client,
    assume_yes: bool,
    quiet: bool,
) -> Result<(), KamError> {
    // Select an asset (first zip-like asset in releases)
    let mut chosen_asset: Option<(&Asset, &str)> = None; // (asset, release_name)
    if let Some(rels) = md.releases.as_ref() {
        for r in rels.iter() {
            if let Some(assets) = r.release_assets.as_ref()
                && let Some(a) = assets.iter().find(|x| {
                    x.content_type
                        .as_deref()
                        .map(|ct| ct.to_lowercase().contains("zip"))
                        .unwrap_or(false)
                        || x.name.to_lowercase().ends_with(".zip")
                })
            {
                let release_label = r
                    .name
                    .as_deref()
                    .or(r.version.as_deref())
                    .unwrap_or("latest");
                chosen_asset = Some((a, release_label));
                break;
            }
        }
    }

    let (asset, release_label) = if let Some((a, rname)) = chosen_asset {
        (a, rname)
    } else {
        Utils::warn(&crate::i18n::tr_fmt(
            "repo.no_downloadable_zip_asset",
            &[&module_id],
        ));
        return Ok(());
    };

    // Print detailed info and ask for confirmation unless assume_yes
    let size_str = asset
        .size
        .map(|s| format!(" ({})", format_size(s)))
        .unwrap_or_default();
    println!(
        "{}",
        crate::i18n::tr_fmt(
            "repo.module_detail.title",
            &[&module_id, &md.module_name.as_deref().unwrap_or("")]
        )
    );
    println!(
        "{}",
        crate::i18n::tr_fmt("repo.module_detail.release", &[&release_label])
    );
    println!(
        "{}",
        crate::i18n::tr_fmt("repo.module_detail.asset", &[&asset.name, &size_str])
    );
    println!(
        "{}",
        crate::i18n::tr_fmt("repo.module_detail.download_url", &[&asset.download_url])
    );

    let confirmed = if assume_yes {
        true
    } else {
        print!(
            "{}",
            crate::i18n::tr_fmt("repo.confirm_download", &[&module_id, &asset.name])
        );
        stdout().flush().map_err(KamError::Io)?;
        let mut input = String::new();
        stdin().read_line(&mut input).map_err(KamError::Io)?;
        let input_trimmed = input.trim();
        // Interpret empty input according to the prompt's default (here: (y/N) -> default is false)
        let ok = parse_confirm_input(input_trimmed, false);
        if !ok {
            Utils::warn(&crate::i18n::tr_fmt("repo.skipped_download", &[&module_id]));
        }
        ok
    };

    if !confirmed {
        return Ok(());
    }

    // Proceed to download
    match download_asset(client, asset, None, quiet) {
        Ok(path) => {
            if !quiet {
                Utils::success(&crate::i18n::tr_fmt(
                    "repo.saved",
                    &[&path.display().to_string()],
                ));
            }
        }
        Err(e) => {
            Utils::error(&crate::i18n::tr_fmt(
                "repo.failed_to_download",
                &[&module_id, &e.to_string()],
            ));
        }
    }

    Ok(())
}

/// Search the remote catalog (search-index.json) for the provided query
/// (case-insensitive substring match across name/description/summary/authors).
pub(crate) fn cache_root_dir() -> Result<PathBuf, KamError> {
    // Allow override for tests or custom installs via env var.
    if let Ok(dir) = std::env::var("KAM_CACHE_DIR") {
        let p = PathBuf::from(dir);
        std::fs::create_dir_all(&p)?;
        return Ok(p);
    }

    // Default to user-level Kam directory (defaults to ~/.kam). Respect KAM_HOME via crate::utils::kam_home_dir().
    let base = crate::utils::kam_home_dir()?;
    std::fs::create_dir_all(&base)?;
    Ok(base)
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn index_cache_path(base_url: &str) -> Result<PathBuf, KamError> {
    let mut p = cache_root_dir()?;
    let fname = format!("index_{}.json", sanitize_filename(base_url));
    p.push(fname);
    Ok(p)
}

fn module_cache_path(module_id: &str) -> Result<PathBuf, KamError> {
    let mut p = cache_root_dir()?;
    p.push("modules");
    std::fs::create_dir_all(&p)?;
    p.push(format!("{}.json", module_id));
    Ok(p)
}

fn is_fresh(path: &Path, ttl_secs: u64) -> bool {
    path.metadata()
        .and_then(|m| m.modified())
        .is_ok_and(|modified| {
            std::time::SystemTime::now()
                .duration_since(modified)
                .is_ok_and(|dur| dur.as_secs() < ttl_secs)
        })
}

fn write_atomic(path: &Path, contents: &str) -> Result<(), KamError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn try_find_index_in_cache_dir(dir: &Path) -> Option<Vec<SearchEntry>> {
    // Look for files matching index_*.json and pick the most recently modified one that parses.
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file()
                && let Some(name) = p.file_name().and_then(|n| n.to_str())
                && name.starts_with("index_")
                && name.ends_with(".json")
                && let Ok(meta) = p.metadata()
                && let Ok(m) = meta.modified()
            {
                candidates.push((m, p));
            }
        }
    }
    // Sort by modified desc
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    for (_mtime, p) in candidates {
        if let Ok(buf) = std::fs::read_to_string(&p)
            && let Ok(entries) = serde_json::from_str::<Vec<SearchEntry>>(&buf)
        {
            return Some(entries);
        }
    }
    None
}

fn fetch_index_cached(client: &Client, base_url: &str) -> Result<Vec<SearchEntry>, KamError> {
    let path = index_cache_path(base_url)?;
    let force_refresh = std::env::var("KAM_FORCE_INDEX_REFRESH").is_ok();

    // 1) If a cached index exists and no explicit force refresh is requested, try it first.
    if path.exists()
        && !force_refresh
        && let Ok(buf) = std::fs::read_to_string(&path)
    {
        if let Ok(entries) = serde_json::from_str::<Vec<SearchEntry>>(&buf) {
            return Ok(entries);
        } else {
            // corrupted or incompatible cache; we'll try fallback scan or network below
        }
    }

    // 2) Try to find an alternative cached index in the same cache directory (index_*.json)
    if let Some(parent) = path.parent()
        && let Some(entries) = try_find_index_in_cache_dir(parent)
    {
        return Ok(entries);
    }

    // 3) Attempt network fetch; if network fails or non-success status, try fallback cache scan before returning error.
    let url = format!("{}{}", base_url, SEARCH_INDEX_PATH);
    match client
        .get(&url)
        .header(USER_AGENT, "kam/repo-search")
        .send()
    {
        Ok(resp) => {
            if !resp.status().is_success() {
                // network returned non-success; try fallback cache scan
                if let Some(parent) = path.parent()
                    && let Some(entries) = try_find_index_in_cache_dir(parent)
                {
                    return Ok(entries);
                }
                return Err(KamError::FetchFailed(format!(
                    "{} returned status {}",
                    url,
                    resp.status()
                )));
            }
            let body = resp.text().map_err(|e| {
                KamError::FetchFailed(format!("Failed to read {} body: {}", url, e))
            })?;
            let entries: Vec<SearchEntry> = serde_json::from_str(&body)
                .map_err(|e| KamError::Json(format!("Failed to parse {} JSON: {}", url, e)))?;
            // Write to primary cache path (atomic)
            let _ = write_atomic(&path, &body);
            Ok(entries)
        }
        Err(e) => {
            // Network error - try fallback cache scan
            if let Some(parent) = path.parent()
                && let Some(entries) = try_find_index_in_cache_dir(parent)
            {
                return Ok(entries);
            }
            Err(KamError::FetchFailed(format!("GET {} failed: {}", url, e)))
        }
    }
}

/// Resolve a possibly-relative entry URL to an absolute URL using the registry base URL.
fn resolve_entry_url(base_url: &str, url: &str) -> String {
    let u = url.trim();
    if u.starts_with("http://") || u.starts_with("https://") {
        return u.to_string();
    }
    if u.starts_with('/') {
        return format!("{}{}", base_url.trim_end_matches('/'), u);
    }
    format!("{}/{}", base_url.trim_end_matches('/'), u)
}

#[allow(clippy::literal_string_with_formatting_args)]
pub fn repo_sync_with_jobs(
    base_url: &str,
    force: bool,
    jobs: Option<usize>,
    quiet: bool,
) -> Result<(), KamError> {
    // Always attempt to fetch the latest index from the remote and write it to cache.
    // `jobs` overrides env var and default parallelism to control number of worker threads.
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| KamError::FetchFailed(format!("Failed to build HTTP client: {}", e)))?;

    let op_start = std::time::Instant::now();
    let url = format!("{}{}", base_url, SEARCH_INDEX_PATH);
    let mut resp = client
        .get(&url)
        .header(USER_AGENT, "kam/repo-sync")
        .send()
        .map_err(|e| KamError::FetchFailed(format!("GET {} failed: {}", url, e)))?;

    if !resp.status().is_success() {
        return Err(KamError::FetchFailed(format!(
            "{} returned status {}",
            url,
            resp.status()
        )));
    }

    // Whether we should show progress bars at all (respect quiet and TTY)
    let show_progress = !quiet && std::io::stdout().is_terminal();

    // Show a progress indicator while fetching the master index (hidden when quiet)
    let index_len = resp.content_length();
    let index_pb = if show_progress {
        index_len.map_or_else(
            || {
                let pb = ProgressBar::new_spinner();
                pb.set_style(
                    ProgressStyle::with_template(
                        "{spinner} Fetching index {bytes}/{total_bytes} ({eta})",
                    )
                    .unwrap_or_else(|_| ProgressStyle::default_bar()),
                );
                pb.enable_steady_tick(Duration::from_millis(100));
                pb
            },
            |len| {
                let pb = ProgressBar::new(len);
                pb.set_style(
                    ProgressStyle::with_template(
                        "{spinner} Fetching index {bytes}/{total_bytes} ({eta})",
                    )
                    .unwrap_or_else(|_| ProgressStyle::default_bar()),
                );
                pb.enable_steady_tick(Duration::from_millis(100));
                pb
            },
        )
    } else {
        ProgressBar::hidden()
    };

    // Read the response body in chunks so the progress bar moves (hidden if quiet)
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 8 * 1024];
    loop {
        let n = resp
            .read(&mut tmp)
            .map_err(|e| KamError::FetchFailed(format!("Failed to read {} body: {}", url, e)))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        index_pb.inc(n as u64);
    }
    index_pb.finish();

    let body = String::from_utf8(buf)
        .map_err(|e| KamError::Json(format!("Failed to parse {} body as UTF-8: {}", url, e)))?;

    let path = index_cache_path(base_url)?;
    write_atomic(&path, &body)?;

    if !quiet {
        if force {
            let msg = crate::i18n::tr_fmt(
                "repo.index_force_synced",
                &[&url, &path.display().to_string()],
            );
            Utils::section(&msg);
        } else {
            Utils::success(&crate::i18n::tr_fmt(
                "repo.index_synced",
                &[&path.display().to_string()],
            ));
        }
    }

    // Parse the index and, if present, attempt to fetch each module's individual index with per-module progress
    let entries: Vec<SearchEntry> = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return Err(KamError::Json(format!(
                "Failed to parse {} JSON: {}",
                url, e
            )));
        }
    };

    if entries.is_empty() {
        return Ok(());
    }
    // Report how long it took to resolve the index into module entries
    let duration_ms = op_start.elapsed().as_millis();
    if !quiet {
        Utils::info(&format!(
            "Resolved {} modules in {}ms",
            entries.len(),
            duration_ms
        ));
    }

    // MultiProgress will nicely render the master spinner above per-module bars.
    let mp = MultiProgress::new();
    // Use a visible spinner only when we actually want to show progress (not quiet and TTY).
    let top_pb = if show_progress {
        let pb = mp.add(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::with_template("{spinner} Preparing modules... ({pos}/{len})")
                .unwrap_or_else(|_| ProgressStyle::default_bar()),
        );
        pb.set_length(entries.len() as u64);
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    } else {
        // Hidden spinner so later `.inc()` calls are safe but nothing is rendered.
        ProgressBar::hidden()
    };

    // Build per-module progress bars and a list of fetch tasks; cached modules are shown immediately,
    // but limit visible items to MAX_VISIBLE (20). If nothing needs fetching, show 'Everything up to date'.
    const MAX_VISIBLE: usize = 20;

    // Count how many modules actually need fetching (not cached or forced)
    let mut need_fetch_count: usize = 0;
    for e in entries.iter() {
        let module_cache = module_cache_path(&e.name);
        if let Ok(p) = &module_cache
            && !force
            && p.exists()
        {
            continue;
        }
        need_fetch_count += 1;
    }

    // If nothing requires network fetch, show a concise 'Everything up to date' and return.
    if need_fetch_count == 0 {
        // If quiet mode, just finish the progress spinner and return silently.
        if quiet {
            top_pb.finish();
            return Ok(());
        }
        let msg = crate::i18n::tr_key("repo.everything_up_to_date");
        Utils::success(&msg);
        top_pb.finish_with_message(msg.clone());
        return Ok(());
    }

    // Prepare up to MAX_VISIBLE visible bars; subsequent modules are kept hidden (no visual).
    // Determine number of workers (CLI `--jobs` > env `KAM_REPO_CONCURRENCY` > default=core count).
    let default_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let mut num_workers = match jobs {
        Some(j) if j > 0 => j,
        _ => std::env::var("KAM_REPO_CONCURRENCY")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(default_workers),
    };
    // Don't spawn more workers than modules
    num_workers = std::cmp::min(num_workers, entries.len());
    let display_limit = num_workers;

    // Build per-module progress bars and a list of fetch tasks; visible entries limited to `display_limit`.
    let mut tasks: Vec<(String, String, PathBuf, ProgressBar)> = Vec::with_capacity(entries.len());
    let mut visible_cnt: usize = 0;
    let updated_count = Arc::new(AtomicUsize::new(0));

    for e in entries.iter() {
        let module_id = e.name.clone();
        let module_cache = module_cache_path(&module_id);

        // Decide whether to skip fetching (cached & not forcing)
        let cached = matches!((&module_cache, force), (Ok(p), false) if p.exists());

        if visible_cnt < display_limit {
            // visible progress bar (or hidden placeholder when quiet)
            let pb = if show_progress {
                let p = mp.add(ProgressBar::new_spinner());
                p.set_style(
                    ProgressStyle::with_template(
                        "{msg:20} {bar:40.cyan/blue} {bytes}/{total_bytes}",
                    )
                    .unwrap_or_else(|_| ProgressStyle::default_bar()),
                );
                p.set_message(format!("{:20}", module_id));
                p
            } else {
                // Hidden progress bar so later operations still work but nothing is shown.
                let p = ProgressBar::hidden();
                p.set_message(format!("{:20}", module_id));
                p
            };

            if cached {
                // show cached as full bar immediately
                pb.set_length(1);
                pb.set_position(1);
                pb.finish_with_message(format!("{:20} (cached)", module_id));
                visible_cnt += 1;
                top_pb.inc(1);
                continue;
            }

            // needs fetch - add as visible task (or hidden placeholder when quiet)
            let murl = format!("{}{}{}.json", base_url, MODULE_JSON_PREFIX, module_id);
            if let Ok(p) = module_cache {
                tasks.push((module_id, murl, p, pb));
            } else {
                // if we couldn't compute cache path, mark as done
                pb.finish_with_message(format!("{:20}", module_id));
                top_pb.inc(1);
            }
            visible_cnt += 1;
        } else {
            // hidden (not displayed) modules
            if cached {
                top_pb.inc(1);
            } else {
                // fetch but hide progress bar
                let pb = ProgressBar::hidden();
                let murl = format!("{}{}{}.json", base_url, MODULE_JSON_PREFIX, module_id);
                if let Ok(p) = module_cache {
                    tasks.push((module_id, murl, p, pb));
                } else {
                    top_pb.inc(1);
                }
            }
        }
    }

    // Dispatch tasks to worker threads using a channel-backed thread-pool (dynamic scheduling).
    if !tasks.is_empty() {
        // `num_workers` was determined earlier and equals the visible count; ensure at least 1 worker
        let num_workers = std::cmp::max(1, visible_cnt.min(entries.len()));

        // Send all tasks into a channel consumed by workers (dynamic work-stealing via channel clones)
        let (tx, rx) = crossbeam_channel::unbounded::<(String, String, PathBuf, ProgressBar)>();
        for t in tasks.into_iter() {
            let _ = tx.send(t);
        }
        // Close the sender to signal workers when done
        drop(tx);

        // Spawn exactly `num_workers` worker threads (one per visible slot), each consuming from the receiver.
        let mut handles = Vec::new();
        for _ in 0..num_workers {
            let rx_clone = rx.clone();
            let client_clone = client.clone();
            let top = top_pb.clone();
            let updated = Arc::clone(&updated_count);
            let handle = std::thread::spawn(move || {
                while let Ok((module_id, murl, ppath, pb)) = rx_clone.recv() {
                    // Defensive check: if cache exists and not forcing, treat as cached
                    if ppath.exists() && !force {
                        pb.finish_with_message(format!("{:20} (cached)", module_id));
                        top.inc(1);
                        continue;
                    }

                    match client_clone
                        .get(&murl)
                        .header(USER_AGENT, "kam/repo-sync-module")
                        .send()
                    {
                        Ok(mut r) => {
                            if !r.status().is_success() {
                                Utils::warn(&format!("{} returned status {}", murl, r.status()));
                                pb.finish_with_message(format!("{:20} (failed)", module_id));
                                top.inc(1);
                                continue;
                            }
                            if let Some(len) = r.content_length() {
                                pb.set_length(len);
                            }

                            let mut mbuf: Vec<u8> = Vec::new();
                            let mut tmp2 = [0u8; 8 * 1024];
                            loop {
                                match r.read(&mut tmp2) {
                                    Ok(n) if n > 0 => {
                                        mbuf.extend_from_slice(&tmp2[..n]);
                                        pb.inc(n as u64);
                                    }
                                    Ok(_) => break,
                                    Err(e) => {
                                        Utils::warn(&format!(
                                            "Failed to read {} body: {}",
                                            murl, e
                                        ));
                                        pb.finish_with_message(format!(
                                            "{:20} (failed)",
                                            module_id
                                        ));
                                        break;
                                    }
                                }
                            }

                            // Validate and write cache atomically
                            match String::from_utf8(mbuf) {
                                Ok(s) => {
                                    if let Err(e) = write_atomic(&ppath, &s) {
                                        Utils::warn(&format!(
                                            "Failed to write cache {}: {}",
                                            module_id, e
                                        ));
                                        pb.finish_with_message(format!(
                                            "{:20} (failed)",
                                            module_id
                                        ));
                                    } else {
                                        updated.fetch_add(1, Ordering::SeqCst);
                                        pb.finish_with_message(format!("{:20}", module_id));
                                    }
                                }
                                Err(_) => {
                                    Utils::warn(&format!("Failed to parse {} body as UTF-8", murl));
                                    pb.finish_with_message(format!("{:20} (invalid)", module_id));
                                }
                            }
                        }
                        Err(e) => {
                            Utils::warn(&format!("GET {} failed: {}", murl, e));
                            pb.finish_with_message(format!("{:20} (failed)", module_id));
                        }
                    }

                    top.inc(1);
                }
            });
            handles.push(handle);
        }

        // Wait for all workers to finish
        for h in handles {
            let _ = h.join();
        }
    }

    // If the workers did not actually update anything, tell the user that everything is up to date.
    let updated_total = updated_count.load(Ordering::SeqCst);
    if !quiet {
        if updated_total == 0 {
            let msg = crate::i18n::tr_key("repo.everything_up_to_date");
            Utils::success(msg);
        } else {
            // Always show a concise summary so users see an effect even when progress bars
            // are suppressed (quiet/non-tty).
            Utils::success(&crate::i18n::tr_fmt(
                "repo.updated_modules",
                &[&updated_total.to_string()],
            ));
        }
    }

    top_pb.finish();
    Ok(())
}

pub fn search_remote(query: &str, base_url: &str) -> Result<(), KamError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| KamError::FetchFailed(format!("Failed to build HTTP client: {}", e)))?;

    // Use cached index if available & fresh; otherwise fetch and cache.
    let entries = fetch_index_cached(&client, base_url)?;

    let q = query.to_lowercase().trim().to_string();
    if q.is_empty() {
        Utils::warn(crate::i18n::tr_key("repo.search.empty_query"));
        return Ok(());
    }

    // Score candidates: substring match => 1.0, otherwise use similarity or token coverage
    let mut scored: Vec<(f64, &SearchEntry)> = Vec::new();
    for e in entries.iter() {
        let name = e.name.to_lowercase();
        let desc = e.description.as_deref().unwrap_or("").to_lowercase();
        let sum = e.summary.as_deref().unwrap_or("").to_lowercase();
        let auth = e.authors.as_deref().unwrap_or("").to_lowercase();
        let hay = format!("{} {} {} {}", name, desc, sum, auth);

        if hay.contains(&q) {
            scored.push((1.0, e));
            continue;
        }

        // token coverage
        let tokens: Vec<&str> = q.split_whitespace().collect();
        let mut matched_tokens = 0usize;
        for token in tokens.iter() {
            if hay.contains(token) {
                matched_tokens += 1;
            }
        }
        let token_ratio = if tokens.is_empty() {
            0.0
        } else {
            matched_tokens as f64 / tokens.len() as f64
        };

        // fuzzy similarity via Levenshtein-based similarity (normalized)
        let sim_name = similarity(&q, &name);
        let sim_desc = similarity(&q, &desc);
        let sim_sum = similarity(&q, &sum);
        let sim_auth = similarity(&q, &auth);
        let sim_max = sim_name.max(sim_desc).max(sim_sum).max(sim_auth);

        let score = token_ratio.max(sim_max);

        // threshold for fuzzy match
        if score >= 0.60 {
            scored.push((score, e));
        }
    }

    if scored.is_empty() {
        Utils::warn(&crate::i18n::tr_fmt("repo.no_results_for", &[&query]));
        return Ok(());
    }

    // sort by score descending
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Print top results
    for (i, (score, e)) in scored.iter().take(50).enumerate() {
        let desc = e.description.as_deref().unwrap_or("");
        let score_suffix = if (*score - 1.0).abs() > f64::EPSILON {
            crate::i18n::tr_fmt("repo.score_format", &[&format!("{:.2}", score)])
        } else {
            "".to_string()
        };
        println!(
            "{}",
            crate::i18n::tr_fmt("repo.result_line_simple", &[&e.name, &desc, &score_suffix])
        );
        if let Some(s) = &e.summary {
            println!("    {}", s);
        }
        if let Some(a) = &e.authors {
            println!("    {}: {}", crate::i18n::tr_key("repo.authors"), a);
        }
        if let Some(u) = &e.url {
            let pretty = resolve_entry_url(base_url, u);
            println!("    {}: {}", crate::i18n::tr_key("repo.url"), pretty);
        }

        // Try to fetch module details for the top few results to show version/time info
        if i < 5
            && let Ok(md) = fetch_module_detail(&client, &e.name, base_url)
        {
            // Version
            if let Some(lr) = md.latest_release.as_deref() {
                println!("    {}: {}", crate::i18n::tr_key("repo.version"), lr);
            } else if let Some(rels) = md.releases.as_ref()
                && let Some(first) = rels.first()
                && let Some(v) = first.version.as_deref().or(first.name.as_deref())
            {
                println!("    {}: {}", crate::i18n::tr_key("repo.version"), v);
            }
            // Time
            if let Some(lt) = md.latest_release_time.as_deref() {
                println!("    {}: {}", crate::i18n::tr_key("repo.updated"), lt);
            } else if let Some(rels) = md.releases.as_ref()
                && let Some(first) = rels.first()
                && let Some(pub_at) = first
                    .published_at
                    .as_deref()
                    .or(first.updated_at.as_deref())
                    .or(first.created_at.as_deref())
            {
                println!("    {}: {}", crate::i18n::tr_key("repo.updated"), pub_at);
            }
        }

        println!();
    }

    Ok(())
}

/// Search the remote catalog and allow interactive selection of results
pub(crate) fn search_remote_interactive(
    query: &str,
    base_url: &str,
) -> Result<Option<String>, KamError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| KamError::FetchFailed(format!("Failed to build HTTP client: {}", e)))?;

    // Use cached index if available & fresh; otherwise fetch and cache.
    let entries = fetch_index_cached(&client, base_url)?;

    let q = query.to_lowercase().trim().to_string();
    if q.is_empty() {
        Utils::warn(crate::i18n::tr_key("repo.search.empty_query"));
        return Ok(None);
    }

    // Score candidates: substring match => 1.0, otherwise use similarity or token coverage
    let mut scored: Vec<(f64, &SearchEntry)> = Vec::new();
    for e in entries.iter() {
        let name = e.name.to_lowercase();
        let desc = e.description.as_deref().unwrap_or("").to_lowercase();
        let sum = e.summary.as_deref().unwrap_or("").to_lowercase();
        let auth = e.authors.as_deref().unwrap_or("").to_lowercase();
        let hay = format!("{} {} {} {}", name, desc, sum, auth);

        if hay.contains(&q) {
            scored.push((1.0, e));
            continue;
        }

        // token coverage
        let tokens: Vec<&str> = q.split_whitespace().collect();
        let mut matched_tokens = 0usize;
        for token in tokens.iter() {
            if hay.contains(token) {
                matched_tokens += 1;
            }
        }
        let token_ratio = if tokens.is_empty() {
            0.0
        } else {
            matched_tokens as f64 / tokens.len() as f64
        };

        // fuzzy similarity via Levenshtein-based similarity (normalized)
        let sim_name = similarity(&q, &name);
        let sim_desc = similarity(&q, &desc);
        let sim_sum = similarity(&q, &sum);
        let sim_auth = similarity(&q, &auth);
        let sim_max = sim_name.max(sim_desc).max(sim_sum).max(sim_auth);

        let score = token_ratio.max(sim_max);

        // threshold for fuzzy match
        if score >= 0.30 {
            // Lower threshold to show more results for selection
            scored.push((score, e));
        }
    }

    if scored.is_empty() {
        Utils::warn(&crate::i18n::tr_fmt("repo.no_results_for", &[&query]));
        return Ok(None);
    }

    // sort by score descending
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Print numbered results
    println!(
        "{}",
        crate::i18n::tr_fmt(
            "repo.similar_packages_header",
            &[&scored.len().to_string(), &query]
        )
    );
    println!();
    for (i, (score, e)) in scored.iter().take(20).enumerate() {
        // Show up to 20 results
        let desc = e.description.as_deref().unwrap_or("");
        let score_suffix = if (*score - 1.0).abs() > f64::EPSILON {
            crate::i18n::tr_fmt("repo.score_format", &[&format!("{:.2}", score)])
        } else {
            "".to_string()
        };
        println!(
            "{}",
            crate::i18n::tr_fmt(
                "repo.result_line",
                &[&(i + 1).to_string(), &e.name, &desc, &score_suffix]
            )
        );
        if let Some(s) = &e.summary {
            println!("    {}", s);
        }
        if let Some(a) = &e.authors {
            println!("    {}: {}", crate::i18n::tr_key("repo.authors"), a);
        }
        if let Some(u) = &e.url {
            let pretty = resolve_entry_url(base_url, u);
            println!("    {}: {}", crate::i18n::tr_key("repo.url"), pretty);
        }

        // Try to fetch module details for the top few results to show version/time info
        if i < 5
            && let Ok(md) = fetch_module_detail(&client, &e.name, base_url)
        {
            if let Some(lr) = md.latest_release.as_deref() {
                println!("    {}: {}", crate::i18n::tr_key("repo.version"), lr);
            } else if let Some(rels) = md.releases.as_ref()
                && let Some(first) = rels.first()
                && let Some(v) = first.version.as_deref().or(first.name.as_deref())
            {
                println!("    {}: {}", crate::i18n::tr_key("repo.version"), v);
            }
            if let Some(lt) = md.latest_release_time.as_deref() {
                println!("    {}: {}", crate::i18n::tr_key("repo.updated"), lt);
            } else if let Some(rels) = md.releases.as_ref()
                && let Some(first) = rels.first()
                && let Some(pub_at) = first
                    .published_at
                    .as_deref()
                    .or(first.updated_at.as_deref())
                    .or(first.created_at.as_deref())
            {
                println!("    {}: {}", crate::i18n::tr_key("repo.updated"), pub_at);
            }
        }

        println!();
    }

    // Ask user to select
    print!("{}", crate::i18n::tr_key("repo.prompt.enter_number"));
    stdout().flush().map_err(KamError::Io)?;
    let mut input = String::new();
    stdin().read_line(&mut input).map_err(KamError::Io)?;
    let input_trimmed = input.trim();

    if input_trimmed.is_empty() {
        return Ok(None);
    }

    match input_trimmed.parse::<usize>() {
        Ok(num) if num > 0 && num <= scored.len() => {
            let selected = &scored[num - 1].1;
            Ok(Some(selected.name.clone()))
        }
        Ok(_) => {
            Utils::warn(crate::i18n::tr_key("repo.invalid_selection_out_of_range"));
            Ok(None)
        }
        Err(_) => {
            Utils::warn(crate::i18n::tr_key("repo.invalid_input_number"));
            Ok(None)
        }
    }
}

/// Simple Levenshtein distance (char-based) - used for a lightweight fuzzy similarity.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let n = a_chars.len();
    let m = b_chars.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur: Vec<usize> = vec![0; m + 1];
    for (i, ca) in a_chars.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b_chars.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = std::cmp::min(std::cmp::min(cur[j] + 1, prev[j + 1] + 1), prev[j] + cost);
        }
        prev.copy_from_slice(&cur);
    }
    prev[m]
}

/// Normalized similarity in [0.0, 1.0]
fn similarity(a: &str, b: &str) -> f64 {
    let a_trim = a.trim();
    let b_trim = b.trim();
    if a_trim.is_empty() && b_trim.is_empty() {
        return 1.0;
    }
    let max_len = a_trim.chars().count().max(b_trim.chars().count());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein(a_trim, b_trim) as f64;
    1.0 - (dist / (max_len as f64))
}

/// Format bytes into a human readable string
fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GiB", b / GB)
    } else if b >= MB {
        format!("{:.2} MiB", b / MB)
    } else if b >= KB {
        format!("{:.2} KiB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// Fetch module JSON details using provided client
/// Derive the effective base URL for modules registry.
///
/// Priority:
/// 1) `override_url` passed to the function call
/// 2) `KAM_MODULES_URL` environment variable
/// 3) `~/.kam/config.toml` -> `[modules] base_url = "..."`
/// 4) default builtin URL
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
    "https://modules.kernelsu.org".to_string()
}

fn fetch_module_detail(
    client: &Client,
    module_id: &str,
    base_url: &str,
) -> Result<ModuleDetail, KamError> {
    let path = module_cache_path(module_id)?;
    let force_refresh = std::env::var("KAM_FORCE_MODULE_REFRESH").is_ok();

    // Prefer cached copy if present and not explicitly forced to refresh.
    if path.exists() && !force_refresh {
        // Try to read & parse cached module JSON; if successful, return it.
        if let Ok(mut f) = File::open(&path) {
            let mut buf = String::new();
            if f.read_to_string(&mut buf).is_ok() {
                if let Ok(md) = serde_json::from_str::<ModuleDetail>(&buf) {
                    return Ok(md);
                } else {
                    Utils::warn(&format!(
                        "Cached module JSON for '{}' could not be parsed; will attempt to refresh from registry",
                        module_id
                    ));
                }
            } else {
                Utils::warn(&format!(
                    "Failed to read cached module JSON for '{}'; will attempt to refresh from registry",
                    module_id
                ));
            }
        } else {
            // Could not open file; fall through to network fetch.
            Utils::warn(&format!(
                "Failed to open cached module JSON for '{}'; will attempt to refresh from registry",
                module_id
            ));
        }
    }

    let url = format!("{}{}{}.json", base_url, MODULE_JSON_PREFIX, module_id);
    let resp = client
        .get(&url)
        .header(USER_AGENT, "kam/repo-module")
        .send()
        .map_err(|e| KamError::FetchFailed(format!("GET {} failed: {}", url, e)))?;

    if !resp.status().is_success() {
        // Fallback to cached copy if available
        if path.exists() {
            let mut s = String::new();
            File::open(&path)?.read_to_string(&mut s)?;
            let md: ModuleDetail = serde_json::from_str(&s)?;
            return Ok(md);
        } else {
            return Err(KamError::FetchFailed(format!(
                "{} returned status {}",
                url,
                resp.status()
            )));
        }
    }

    let body = resp
        .text()
        .map_err(|e| KamError::FetchFailed(format!("Failed to read {} body: {}", url, e)))?;
    let md: ModuleDetail = serde_json::from_str(&body)
        .map_err(|e| KamError::Json(format!("Failed to parse {} JSON: {}", url, e)))?;
    let _ = write_atomic(&path, &body);
    Ok(md)
}

/// Download the latest release ZIP for `module_id`.
///
/// If `dest_dir` is provided it will be saved there; otherwise it will be
/// saved to the current working directory. Returns the saved file path. When
/// `quiet` is true, progress output will be suppressed.
pub fn download_module_latest(
    module_id: &str,
    dest_dir: Option<&Path>,
    quiet: bool,
) -> Result<PathBuf, KamError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| KamError::FetchFailed(format!("Failed to build HTTP client: {}", e)))?;

    let url = format!("{}{}{}.json", BASE_URL, MODULE_JSON_PREFIX, module_id);
    let resp = client
        .get(&url)
        .header(USER_AGENT, "kam/repo-module")
        .send()
        .map_err(|e| KamError::FetchFailed(format!("GET {} failed: {}", url, e)))?;

    if !resp.status().is_success() {
        return Err(KamError::FetchFailed(format!(
            "{} returned status {}",
            url,
            resp.status()
        )));
    }

    // Note: ModuleDetail uses camelCase -> snake_case mapping via serde.
    let md: ModuleDetail = resp
        .json()
        .map_err(|e| KamError::Json(format!("Failed to parse {} JSON: {}", url, e)))?;

    if let Some(rels) = md.releases.as_ref() {
        for r in rels.iter() {
            if let Some(assets) = r.release_assets.as_ref() {
                // Prefer zip-like assets
                if let Some(a) = assets.iter().find(|x| {
                    x.content_type
                        .as_deref()
                        .map(|ct| ct.to_lowercase().contains("zip"))
                        .unwrap_or(false)
                        || x.name.to_lowercase().ends_with(".zip")
                }) {
                    return download_asset(&client, a, dest_dir, quiet);
                }
            }
        }
    }

    Err(KamError::PackageNotFound(format!(
        "No downloadable zip asset found for module {}",
        module_id
    )))
}

fn download_asset(
    client: &Client,
    asset: &Asset,
    dest_dir: Option<&Path>,
    quiet: bool,
) -> Result<PathBuf, KamError> {
    let url = &asset.download_url;

    let mut resp = client
        .get(url)
        .header(USER_AGENT, "kam/repo-download")
        .send()
        .map_err(|e| KamError::FetchFailed(format!("GET {} failed: {}", url, e)))?;

    if !resp.status().is_success() {
        return Err(KamError::FetchFailed(format!(
            "{} returned status {}",
            url,
            resp.status()
        )));
    }

    let filename = &asset.name;
    let dest = dest_dir
        .map(|d| d.join(filename))
        .unwrap_or_else(|| PathBuf::from(filename));

    // Try to determine size for progress
    let size = asset.size.or_else(|| resp.content_length());
    let show_progress = !quiet && std::io::stdout().is_terminal();

    let pb = if show_progress {
        size.map_or_else(ProgressBar::new_spinner, ProgressBar::new)
    } else {
        ProgressBar::hidden()
    };

    pb.set_style(
        ProgressStyle::with_template("{bar:40.cyan/blue} {bytes}/{total_bytes} ({eta})")
            .unwrap_or_else(|_| ProgressStyle::default_bar()),
    );

    let mut out = File::create(&dest).map_err(KamError::Io)?;

    let mut buf = [0u8; 8 * 1024];
    let mut written: u64 = 0;

    loop {
        let n = resp
            .read(&mut buf)
            .map_err(|e| KamError::FetchFailed(format!("Failed to read response body: {}", e)))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(KamError::Io)?;
        written += n as u64;
        if size.is_some() {
            pb.set_position(written);
        } else {
            pb.inc(n as u64);
        }
    }

    pb.finish();

    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::sync::Mutex;
    use tempfile;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_search_asl() {
        let base = effective_base_url(None);
        let res = search_remote("asl", &base);
        assert!(res.is_ok());
    }

    #[test]
    fn test_resolve_asl_module() {
        // Fetch module JSON for 'asl' and validate structure contains releases & assets.
        let client = Client::new();
        let url = format!("{}{}{}.json", BASE_URL, MODULE_JSON_PREFIX, "asl");
        let resp = client.get(&url).send().unwrap();
        assert!(resp.status().is_success());
        let md: ModuleDetail = resp.json().unwrap();
        assert_eq!(md.module_id, "asl"); // sanity check
        assert!(md.releases.is_some());
        let rels = md.releases.unwrap();
        assert!(!rels.is_empty());
        assert!(rels.iter().any(|r| {
            r.release_assets
                .as_ref()
                .map(|a| !a.is_empty())
                .unwrap_or(false)
        }));
    }

    #[test]
    fn test_parse_confirm_input() {
        // empty -> default false (prompt shows (y/N))
        assert!(!parse_confirm_input("", false));
        // empty -> default true when default_yes is true
        assert!(parse_confirm_input("", true));
        assert!(parse_confirm_input("y", false));
        assert!(parse_confirm_input("Y", false));
        assert!(parse_confirm_input("yes", false));
        assert!(!parse_confirm_input("n", true));
        // whitespace counts as empty
        assert!(!parse_confirm_input("   ", false));
    }

    #[test]
    fn test_resolve_entry_url() {
        // Ensure relative and absolute entry URLs are resolved correctly.
        let base = "https://modules.kernelsu.org";
        assert_eq!(
            resolve_entry_url(base, "/module/asl"),
            "https://modules.kernelsu.org/module/asl"
        );
        assert_eq!(
            resolve_entry_url(base, "module/asl"),
            "https://modules.kernelsu.org/module/asl"
        );
        assert_eq!(
            resolve_entry_url(base, "https://example.org/test"),
            "https://example.org/test"
        );
    }

    #[test]
    fn test_fetch_module_detail_latest_release_time() {
        // Fetch module JSON for 'asl' and ensure latest_release and latest_release_time are parsed.
        let client = Client::new();
        let base = effective_base_url(None);
        let md = fetch_module_detail(&client, "asl", &base).unwrap();
        assert_eq!(md.module_id, "asl"); // sanity check
        assert!(md.latest_release.is_some());
        assert!(md.latest_release_time.is_some());
    }

    #[test]
    fn test_fetch_index_cached_uses_local_cache() {
        // Create a temporary cache dir and write a fake index file, then ensure
        // fetch_index_cached reads it (no network needed).
        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = TEST_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("KAM_CACHE_DIR", tmp.path().to_str().unwrap());
        }

        let base = "https://example.test";
        let path = index_cache_path(base).unwrap();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }

        let sample = r#"[{"name":"testpkg","description":"desc","summary":"sum","authors":"me","url":"https://example"}]"#;
        std::fs::write(&path, sample).unwrap();

        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();

        let entries = fetch_index_cached(&client, base).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "testpkg");

        // Clean up env
        unsafe {
            std::env::remove_var("KAM_CACHE_DIR");
        }
    }

    #[test]
    fn test_fetch_module_detail_cached_reads_local_file() {
        // Create a temporary cache dir and write a fake module JSON, then ensure
        // fetch_module_detail returns the cached content.
        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = TEST_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("KAM_CACHE_DIR", tmp.path().to_str().unwrap());
        }

        let base = effective_base_url(None);
        let module_id = "testmodule";
        let path = module_cache_path(module_id).unwrap();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }

        // Use camelCase keys as the deserializer expects.
        let sample = r#"{
            "moduleId": "testmodule",
            "moduleName": "Test Module",
            "url": "https://example",
            "authors": null,
            "latestRelease": null,
            "releases": [],
            "summary": "a test module"
        }"#;
        std::fs::write(&path, sample).unwrap();

        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();

        let md = fetch_module_detail(&client, module_id, &base).unwrap();
        assert_eq!(md.module_id, "testmodule");
        assert_eq!(md.module_name.unwrap(), "Test Module");

        // Clean up env
        unsafe {
            std::env::remove_var("KAM_CACHE_DIR");
        }
    }

    #[test]
    fn test_cache_root_dir_respects_kam_home_env() {
        // Ensure cache_root_dir() uses KAM_HOME when provided.
        let _guard = TEST_LOCK.lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();

        // Preserve original KAM_HOME and set our test value
        let orig = std::env::var_os("KAM_HOME");
        unsafe {
            std::env::set_var("KAM_HOME", tmp.path().to_str().unwrap());
        }

        // cache_root_dir should return the directory pointed to by KAM_HOME
        let base = cache_root_dir().expect("cache_root_dir should succeed");
        assert_eq!(base, tmp.path().to_path_buf());

        // Restore original KAM_HOME
        if let Some(v) = orig {
            unsafe {
                std::env::set_var("KAM_HOME", v);
            }
        } else {
            unsafe {
                std::env::remove_var("KAM_HOME");
            }
        }
    }

    // ---- Added parsing tests for `kam repo sync` ----

    #[test]
    fn test_parsing_repo_sync_sets_subcommand() {
        let cli = crate::cli::Cli::parse_from(["kam", "repo", "sync"]);
        match cli.command {
            Some(crate::cli::Commands::Repo(repo_args)) => match repo_args.command {
                Some(RepoCommand::Sync(sync_args)) => {
                    assert!(!sync_args.force, "expected --force to be false by default");
                }
                _ => panic!("expected RepoCommand::Sync"),
            },
            _ => panic!("expected Commands::Repo"),
        }
    }

    #[test]
    fn test_parsing_repo_sync_force_sets_force() {
        let cli = crate::cli::Cli::parse_from(["kam", "repo", "sync", "--force"]);
        match cli.command {
            Some(crate::cli::Commands::Repo(repo_args)) => match repo_args.command {
                Some(RepoCommand::Sync(sync_args)) => {
                    assert!(sync_args.force, "expected --force to be true");
                }
                _ => panic!("expected RepoCommand::Sync"),
            },
            _ => panic!("expected Commands::Repo"),
        }
    }

    #[test]
    fn test_parsing_repo_sync_modules_url() {
        let url = "https://example.test";
        let cli = crate::cli::Cli::parse_from(["kam", "repo", "sync", "--modules-url", url]);
        // `--modules-url` is a global/top-level option; ensure it's parsed into the top-level `Cli`.
        assert_eq!(cli.modules_url.as_deref(), Some(url));
    }

    #[test]
    fn test_parsing_repo_sync_jobs_sets_jobs() {
        let cli = crate::cli::Cli::parse_from(["kam", "repo", "sync", "--jobs", "4"]);
        match cli.command {
            Some(crate::cli::Commands::Repo(repo_args)) => match repo_args.command {
                Some(RepoCommand::Sync(sync_args)) => {
                    assert_eq!(sync_args.jobs, Some(4));
                }
                _ => panic!("expected RepoCommand::Sync"),
            },
            _ => panic!("expected Commands::Repo"),
        }
    }
}
