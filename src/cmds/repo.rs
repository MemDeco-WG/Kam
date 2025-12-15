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
use clap::Args;
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
    /// Pacman-style sync (download) flag (equivalent to pacman -S)
    #[arg(short = 'S', long = "sync")]
    pub sync: bool,

    /// Pacman-style search modifier (use with -S as '-Ss' to search or '-s' alone to search)
    #[arg(short = 's', long = "search")]
    pub search: bool,

    /// Skip interactive confirmation prompts (use -y or --yes)
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Positional targets: module IDs or search terms (used with -S / -s)
    #[arg(value_name = "TARGETS", num_args = 0.., last = true)]
    pub targets: Vec<String>,
}

/// Entrypoint for `kam repo` subcommand.
pub fn run(args: RepoArgs) -> Result<(), KamError> {
    handle_pacman_style(args.sync, args.search, args.targets)
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
) -> Result<(), KamError> {
    // Respect explicit --yes/-y flag passed from CLI (as param) or environment arg fallback
    let assume_yes = yes || std::env::args().any(|a| a == "-y" || a == "--yes");

    // Search mode (supports fuzzy search)
    if search {
        let q = targets.join(" ");
        if q.is_empty() {
            return Err(KamError::CommandFailed(
                "Search requires a query e.g. `-Ss <term>`".into(),
            ));
        }
        return search_remote(&q);
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
            let md = fetch_module_detail(&client, module_id)?;
            // Select an asset (first zip-like asset in releases)
            let mut chosen_asset: Option<(&Asset, &str)> = None; // (asset, release_name)
            if let Some(rels) = md.releases.as_ref() {
                for r in rels.iter() {
                    if let Some(assets) = r.release_assets.as_ref() {
                        if let Some(a) = assets.iter().find(|x| {
                            x.content_type
                                .as_deref()
                                .map(|ct| ct.to_lowercase().contains("zip"))
                                .unwrap_or(false)
                                || x.name.to_lowercase().ends_with(".zip")
                        }) {
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
            }

            let (asset, release_label) = match chosen_asset {
                Some((a, rname)) => (a, rname),
                None => {
                    Utils::warn(&format!(
                        "No downloadable zip asset found for module {}",
                        module_id
                    ));
                    continue;
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
                    "Confirm download '{}' [{}] ? Type 'yes' or any non-empty input to confirm, press Enter to cancel: ",
                    module_id, asset.name
                );
                stdout().flush().map_err(KamError::Io)?;
                let mut input = String::new();
                stdin().read_line(&mut input).map_err(KamError::Io)?;
                let ok = !input.trim().is_empty();
                if !ok {
                    Utils::warn(&format!("Skipped download: {}", module_id));
                }
                ok
            };

            if !confirmed {
                continue;
            }

            // Proceed to download
            match download_asset(&client, asset, None) {
                Ok(path) => {
                    Utils::success(&format!("Saved: {}", path.display()));
                }
                Err(e) => {
                    Utils::error(&format!("Failed to download {}: {}", module_id, e));
                }
            }
        }

        return Ok(());
    }

    // Nothing to do
    Ok(())
}

/// Search the remote catalog (search-index.json) for the provided query
/// (case-insensitive substring match across name/description/summary/authors).
pub fn search_remote(query: &str) -> Result<(), KamError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| KamError::FetchFailed(format!("Failed to build HTTP client: {}", e)))?;

    let url = format!("{}{}", BASE_URL, SEARCH_INDEX_PATH);
    let resp = client
        .get(&url)
        .header(USER_AGENT, "kam/repo-search")
        .send()
        .map_err(|e| KamError::FetchFailed(format!("GET {} failed: {}", url, e)))?;

    if !resp.status().is_success() {
        return Err(KamError::FetchFailed(format!(
            "{} returned status {}",
            url,
            resp.status()
        )));
    }

    let entries: Vec<SearchEntry> = resp
        .json()
        .map_err(|e| KamError::Json(format!("Failed to parse {} JSON: {}", url, e)))?;

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
                if (*score - 1.0).abs() > std::f64::EPSILON {
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
fn fetch_module_detail(client: &Client, module_id: &str) -> Result<ModuleDetail, KamError> {
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
    let md: ModuleDetail = resp
        .json()
        .map_err(|e| KamError::Json(format!("Failed to parse {} JSON: {}", url, e)))?;
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
    let size = asset
        .size
        .or_else(|| resp.content_length().map(|s| s as u64));

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
    use reqwest::blocking::Client;

    #[test]
    fn test_search_asl() {
        // Best-effort network test: ensure search completes and doesn't error.
        let res = search_remote("asl");
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
}
