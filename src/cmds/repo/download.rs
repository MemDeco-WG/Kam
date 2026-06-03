use super::cache::{module_cache_path, module_url};
use super::{Asset, BASE_URL, ModuleDetail, Release};
use crate::errors::KamError;
use crate::utils::Utils;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use std::fs::File;
use std::io::IsTerminal;
use std::io::{Read, Write, stdin, stdout};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(super) fn process_module_download(
    md: &ModuleDetail,
    module_id: &str,
    client: &Client,
    assume_yes: bool,
    quiet: bool,
) -> Result<(), KamError> {
    process_module_download_to_dir(md, module_id, client, assume_yes, quiet, None).map(|_| ())
}

pub(super) fn process_module_download_to_dir(
    md: &ModuleDetail,
    module_id: &str,
    client: &Client,
    assume_yes: bool,
    quiet: bool,
    dest_dir: Option<&Path>,
) -> Result<Option<PathBuf>, KamError> {
    let Some((asset, release_label)) = select_zip_asset(md) else {
        Utils::warn(trf!("repo.no_downloadable_zip_asset", module_id));
        return Ok(None);
    };

    print_module_download_detail(md, asset, release_label);
    let confirmed = prompt_confirm_download(module_id, &asset.name, assume_yes)?;
    if !confirmed {
        return Ok(None);
    }

    match download_asset(client, asset, dest_dir, quiet) {
        Ok(path) => {
            if !quiet {
                Utils::success(trf!("repo.saved", path.display().to_string()));
            }
            Ok(Some(path))
        }
        Err(e) => {
            let err_str = e.to_string();
            let args: Vec<&dyn std::fmt::Display> = vec![&module_id, &err_str];
            Utils::error(crate::i18n::tr_fmt("repo.failed_to_download", &args));
            Ok(None)
        }
    }
}

pub(super) fn read_module_detail_from_cache(module_id: &str) -> Result<ModuleDetail, KamError> {
    let path = module_cache_path(module_id)?;
    read_cached_module(module_id, &path, false).ok_or_else(|| {
        KamError::PackageNotFound(format!(
            "No cached package metadata for '{module_id}'. Run `kam -Sy` first."
        ))
    })
}

pub(super) fn try_read_module_detail_from_cache(
    module_id: &str,
    quiet: bool,
) -> Result<Option<ModuleDetail>, KamError> {
    let path = module_cache_path(module_id)?;
    Ok(read_cached_module(module_id, &path, quiet))
}

/// # Errors
/// Returns `KamError` on network, I/O, or JSON parsing failures.
pub fn download_module_latest(
    module_id: &str,
    dest_dir: Option<&Path>,
    quiet: bool,
) -> Result<PathBuf, KamError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| KamError::FetchFailed(format!("Failed to build HTTP client: {e}")))?;

    let url = module_url(BASE_URL, module_id);
    let resp = client
        .get(&url)
        .header(USER_AGENT, "kam/repo-module")
        .send()
        .map_err(|e| KamError::FetchFailed(format!("GET {url} failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(KamError::FetchFailed(format!(
            "{} returned status {}",
            url,
            resp.status()
        )));
    }

    let md: ModuleDetail = resp
        .json()
        .map_err(|e| KamError::Json(format!("Failed to parse {url} JSON: {e}")))?;

    find_zip_asset(md.releases.as_ref())
        .map(|asset| download_asset(&client, asset, dest_dir, quiet))
        .transpose()?
        .ok_or_else(|| {
            KamError::PackageNotFound(format!(
                "No downloadable zip asset found for module {module_id}"
            ))
        })
}

fn read_cached_module(module_id: &str, path: &Path, quiet: bool) -> Option<ModuleDetail> {
    if let Ok(mut f) = File::open(path) {
        let mut buf = String::new();
        if f.read_to_string(&mut buf).is_ok() {
            if let Ok(md) = serde_json::from_str::<ModuleDetail>(&buf) {
                return Some(md);
            }
            if !quiet {
                Utils::warn(format!(
                    "Cached module JSON for {module_id} could not be parsed"
                ));
            }
        } else if !quiet {
            Utils::warn(format!("Failed to read cached module JSON for {module_id}"));
        }
    } else if !quiet {
        Utils::warn(format!("Failed to open cached module JSON for {module_id}"));
    }
    None
}

pub(super) fn print_module_info(md: &ModuleDetail) {
    print_module_metadata(md);
    if let Some((asset, release_label)) = select_zip_asset(md) {
        println!("{}", trf!("repo.module_detail.release", release_label));
        println!(
            "{}",
            trf!(
                "repo.module_detail.asset",
                asset.name,
                asset
                    .size
                    .map(|s| format!(" ({})", format_size(s)))
                    .unwrap_or_default()
            )
        );
    }
}

pub(super) fn selected_zip_asset_url(md: &ModuleDetail) -> Option<&str> {
    select_zip_asset(md).map(|(asset, _)| asset.download_url.as_str())
}

fn print_module_metadata(md: &ModuleDetail) {
    println!(
        "{}",
        trf!(
            "repo.module_detail.title",
            md.module_id.as_str(),
            md.module_name.as_deref().unwrap_or("")
        )
    );
    print_optional_detail("repo.module_detail.url", md.url.as_deref());
    print_optional_detail("repo.module_detail.homepage", md.homepage_url.as_deref());
    print_optional_detail("repo.module_detail.summary", md.summary.as_deref());
    print_authors(md);
}

fn print_module_download_detail(md: &ModuleDetail, asset: &Asset, release_label: &str) {
    let size_str = asset
        .size
        .map(|s| format!(" ({})", format_size(s)))
        .unwrap_or_default();
    print_module_metadata(md);
    println!("{}", trf!("repo.module_detail.release", release_label));
    println!("{}", trf!("repo.module_detail.asset", asset.name, size_str));
    println!(
        "{}",
        trf!("repo.module_detail.download_url", asset.download_url)
    );
}

fn print_optional_detail(key: &str, value: Option<&str>) {
    if let Some(value) = value
        && !value.trim().is_empty()
    {
        println!("{}", trf!(key, value));
    }
}

fn print_authors(md: &ModuleDetail) {
    if let Some(auths) = md.authors.as_ref()
        && !auths.is_empty()
    {
        let authors_str = auths
            .iter()
            .map(|a| {
                if let Some(link) = a.link.as_deref()
                    && !link.trim().is_empty()
                {
                    return format!("{name} ({link})", name = a.name, link = link);
                }
                a.name.clone()
            })
            .collect::<Vec<_>>()
            .join(", ");
        println!("{}: {}", trf!("repo.authors"), authors_str);
    }
}

fn prompt_confirm_download(
    module_id: &str,
    asset_name: &str,
    assume_yes: bool,
) -> Result<bool, KamError> {
    if assume_yes {
        return Ok(true);
    }
    print!("{}", trf!("repo.confirm_download", module_id, asset_name));
    stdout().flush().map_err(KamError::Io)?;
    let mut input = String::new();
    stdin().read_line(&mut input).map_err(KamError::Io)?;
    let ok = parse_confirm_input(input.trim(), false);
    if !ok {
        Utils::warn(trf!("repo.skipped_download", module_id));
    }
    Ok(ok)
}

fn parse_confirm_input(input: &str, default_yes: bool) -> bool {
    let s = input.trim();
    if s.is_empty() {
        default_yes
    } else {
        let s = s.to_ascii_lowercase();
        s == "y" || s == "yes"
    }
}

fn select_zip_asset(md: &ModuleDetail) -> Option<(&Asset, &str)> {
    md.releases.as_ref().and_then(|rels| {
        rels.iter().find_map(|release| {
            let asset = release
                .assets
                .as_ref()?
                .iter()
                .find(|asset| is_zip_asset(asset))?;
            let label = release
                .name
                .as_deref()
                .or(release.version.as_deref())
                .unwrap_or("latest");
            Some((asset, label))
        })
    })
}

fn find_zip_asset(releases: Option<&Vec<Release>>) -> Option<&Asset> {
    releases?.iter().find_map(|release| {
        release
            .assets
            .as_ref()?
            .iter()
            .find(|asset| is_zip_asset(asset))
    })
}

fn is_zip_asset(asset: &Asset) -> bool {
    asset
        .content_type
        .as_deref()
        .is_some_and(|ct| ct.to_lowercase().contains("zip"))
        || asset.name.to_lowercase().ends_with(".zip")
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        let whole = bytes / GB;
        let frac = (bytes % GB) * 100 / GB;
        format!("{whole}.{frac:02} GiB")
    } else if bytes >= MB {
        let whole = bytes / MB;
        let frac = (bytes % MB) * 100 / MB;
        format!("{whole}.{frac:02} MiB")
    } else if bytes >= KB {
        let whole = bytes / KB;
        let frac = (bytes % KB) * 100 / KB;
        format!("{whole}.{frac:02} KiB")
    } else {
        format!("{bytes} B")
    }
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
        .map_err(|e| KamError::FetchFailed(format!("GET {url} failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(KamError::FetchFailed(format!(
            "{url} returned status {status}",
            status = resp.status()
        )));
    }

    let dest = dest_dir.map_or_else(|| PathBuf::from(&asset.name), |d| d.join(&asset.name));
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(KamError::Io)?;
    }
    let size = asset.size.or_else(|| resp.content_length());
    let pb = download_progress(size, quiet);
    let mut out = File::create(&dest).map_err(KamError::Io)?;
    let mut buf = [0u8; 8 * 1024];
    let mut written = 0u64;

    loop {
        let n = resp
            .read(&mut buf)
            .map_err(|e| KamError::FetchFailed(format!("Failed to read response body: {e}")))?;
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

fn download_progress(size: Option<u64>, quiet: bool) -> ProgressBar {
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
    pb
}
