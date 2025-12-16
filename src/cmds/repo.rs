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
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use std::fs::File;
use std::io::{Read, Write, stdin, stdout};
use std::path::{Path, PathBuf};
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

    /// URL for the modules registry API (default: https://modules.kernelsu.org). Overrides the built-in modules endpoint.
    #[arg(long = "modules-url", value_name = "URL")]
    pub modules_url: Option<String>,

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
    /// Optional modules registry base url (override)
    #[arg(long = "modules-url", value_name = "URL")]
    pub modules_url: Option<String>,
}

pub fn run(args: RepoArgs) -> Result<(), KamError> {
    // If a nested repo subcommand is provided, handle it first (e.g., `repo sync`)
    if let Some(RepoCommand::Sync(sync_args)) = args.command {
        // Determine effective base URL: priority - sync command arg -> repo arg -> default
        let base = effective_base_url(
            sync_args
                .modules_url
                .as_deref()
                .or(args.modules_url.as_deref()),
        );
        return repo_sync(&base, sync_args.force);
    }

    // fallback to existing pacman-style handling (-S / -s)
    handle_pacman_style(
        args.sync,
        args.search,
        args.targets,
        false,
        args.modules_url,
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
            Utils::section(&format!("Download: {}", module_id));

            // Fetch module details
            let _md = match fetch_module_detail(&client, module_id, &base) {
                Ok(md) => {
                    // Process the module normally
                    process_module_download(&md, module_id, &client, assume_yes)?;
                    continue;
                }
                Err(KamError::FetchFailed(ref e)) if e.contains("404") || e.contains("not found") || e.contains("Not Found") => {
                    // If the exact module wasn't found, suggest similar modules via interactive search
                    Utils::warn(&format!("Module '{}' not found. Showing similar modules:", module_id));
                    if let Some(selected_module) = search_remote_interactive(module_id, &base)? {
                        Utils::info(&format!("Selected module: {}", selected_module));
                        // Now fetch the details for the selected module
                        let md = fetch_module_detail(&client, &selected_module, &base)?;
                        // Continue with the download process using the selected module
                        process_module_download(&md, &selected_module, &client, assume_yes)?;
                        continue; // Skip to the next target
                    } else {
                        Utils::info("Skipped selection.");
                        continue;
                    }
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
fn process_module_download(
    md: &ModuleDetail, 
    module_id: &str, 
    client: &Client, 
    assume_yes: bool
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

    let (asset, release_label) = match chosen_asset {
        Some((a, rname)) => (a, rname),
        None => {
            Utils::warn(&format!(
                "No downloadable zip asset found for module {}",
                module_id
            ));
            return Ok(());
        }
    };

    // Print detailed info and ask for confirmation unless assume_yes
    let size_str = asset
        .size
        .map(|s| format!(" ({})", format_size(s)))
        .unwrap_or_default();
    println!(
        "Module: {} - {}",
        module_id,
        md.module_name.as_deref().unwrap_or("")
    );
    println!("Release: {}", release_label);
    println!("Asset: {}{}", asset.name, size_str);
    println!("Download URL: {}", asset.download_url);

    let confirmed = if assume_yes {
        true
    } else {
        print!(
            "Confirm download '{}' [{}] ? [Y/n] ",
            module_id, asset.name
        );
        stdout().flush().map_err(KamError::Io)?;
        let mut input = String::new();
        stdin().read_line(&mut input).map_err(KamError::Io)?;
        let input_trimmed = input.trim();
        let ok = input_trimmed.is_empty() || input_trimmed.to_lowercase() == "y" || input_trimmed.to_lowercase() == "yes";
        if !ok {
            Utils::warn(&format!("Skipped download: {}", module_id));
        }
        ok
    };

    if !confirmed {
        return Ok(());
    }

    // Proceed to download
    match download_asset(client, asset, None) {
        Ok(path) => {
            Utils::success(&format!("Saved: {}", path.display()));
        }
        Err(e) => {
            Utils::error(&format!("Failed to download {}: {}", module_id, e));
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

    // Default to user-level Kam directory (~/.kam) so module cache is colocated with templates.
    let home = dirs::home_dir().ok_or_else(|| {
        KamError::InvalidDirectory("Could not determine home directory".to_string())
    })?;
    let base = home.join(".kam");
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
    match path.metadata().and_then(|m| m.modified()) {
        Ok(modified) => match std::time::SystemTime::now().duration_since(modified) {
            Ok(dur) => dur.as_secs() < ttl_secs,
            Err(_) => false,
        },
        Err(_) => false,
    }
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

pub fn repo_sync(base_url: &str, force: bool) -> Result<(), KamError> {
    // Always attempt to fetch the latest index from the remote and write it to cache.
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| KamError::FetchFailed(format!("Failed to build HTTP client: {}", e)))?;

    let url = format!("{}{}", base_url, SEARCH_INDEX_PATH);
    let resp = client
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

    let body = resp
        .text()
        .map_err(|e| KamError::FetchFailed(format!("Failed to read {} body: {}", url, e)))?;

    let path = index_cache_path(base_url)?;
    write_atomic(&path, &body)?;

    if force {
        Utils::section(&format!(
            "Index force-synced from {} -> {}",
            url,
            path.display()
        ));
    } else {
        Utils::success(&format!("Index synced to {}", path.display()));
    }
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
        Utils::warn("Empty query");
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
        Utils::warn(&format!("No results found for '{}'.", query));
        return Ok(());
    }

    // sort by score descending
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Print top results
    for (score, e) in scored.iter().take(50) {
        println!(
            "{} - {}{}",
            e.name,
            e.description.as_deref().unwrap_or(""),
            {
                if (*score - 1.0).abs() > f64::EPSILON {
                    format!("  (score: {:.2})", score)
                } else {
                    "".to_string()
                }
            }
        );
        if let Some(s) = &e.summary {
            println!("    {}", s);
        }
        if let Some(a) = &e.authors {
            println!("    authors: {}", a);
        }
        if let Some(u) = &e.url {
            println!("    url: {}", u);
        }
        println!();
    }

    Ok(())
}

/// Search the remote catalog and allow interactive selection of results
pub(crate) fn search_remote_interactive(query: &str, base_url: &str) -> Result<Option<String>, KamError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| KamError::FetchFailed(format!("Failed to build HTTP client: {}", e)))?;

    // Use cached index if available & fresh; otherwise fetch and cache.
    let entries = fetch_index_cached(&client, base_url)?;

    let q = query.to_lowercase().trim().to_string();
    if q.is_empty() {
        Utils::warn("Empty query");
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
        if score >= 0.30 { // Lower threshold to show more results for selection
            scored.push((score, e));
        }
    }

    if scored.is_empty() {
        Utils::warn(&format!("No results found for '{}'.", query));
        return Ok(None);
    }

    // sort by score descending
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Print numbered results
    println!("There are {} similar packages to '{}':", scored.len(), query);
    println!();
    for (i, (score, e)) in scored.iter().take(20).enumerate() {  // Show up to 20 results
        println!(
            "{} - {} - {}{}",
            (i + 1),
            e.name,
            e.description.as_deref().unwrap_or(""),
            {
                if (*score - 1.0).abs() > f64::EPSILON {
                    format!("  (score: {:.2})", score)
                } else {
                    "".to_string()
                }
            }
        );
        if let Some(s) = &e.summary {
            println!("    {}", s);
        }
        if let Some(a) = &e.authors {
            println!("    authors: {}", a);
        }
        if let Some(u) = &e.url {
            println!("    url: {}", u);
        }
        println!();
    }

    // Ask user to select
    print!("Enter the number of the package to install (or press Enter to skip): ");
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
            Utils::warn("Invalid selection. Number out of range.");
            Ok(None)
        }
        Err(_) => {
            Utils::warn("Invalid input. Please enter a number.");
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
fn effective_base_url(override_url: Option<&str>) -> String {
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
    if let Some(home) = dirs::home_dir() {
        let cfg = home.join(".kam").join("config.toml");
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

/// Download the latest release ZIP for `module_id` and save to `dest_dir` if provided,
/// otherwise to the current working directory. Returns the saved file path.
pub fn download_module_latest(
    module_id: &str,
    dest_dir: Option<&Path>,
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
                    return download_asset(&client, a, dest_dir);
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

    let pb = match size {
        Some(s) => ProgressBar::new(s),
        None => ProgressBar::new_spinner(),
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
        assert!(md.module_id == "asl" || md.module_id == "asl"); // sanity check
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

        let entries = fetch_index_cached(&client, &base).unwrap();
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

    // ---- Added parsing tests for `kam repo sync` ----

    #[test]
    fn test_parsing_repo_sync_sets_subcommand() {
        let cli = crate::cli::Cli::parse_from(&["kam", "repo", "sync"]);
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
        let cli = crate::cli::Cli::parse_from(&["kam", "repo", "sync", "--force"]);
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
        let cli = crate::cli::Cli::parse_from(&["kam", "repo", "sync", "--modules-url", url]);
        match cli.command {
            Some(crate::cli::Commands::Repo(repo_args)) => match repo_args.command {
                Some(RepoCommand::Sync(sync_args)) => {
                    assert_eq!(sync_args.modules_url.as_deref(), Some(url));
                }
                _ => panic!("expected RepoCommand::Sync"),
            },
            _ => panic!("expected Commands::Repo"),
        }
    }
}
