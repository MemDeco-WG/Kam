use super::{MODULE_JSON_PREFIX, SEARCH_INDEX_PATH, SearchEntry};
use crate::errors::KamError;
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use std::path::{Path, PathBuf};

pub(crate) fn cache_root_dir() -> Result<PathBuf, KamError> {
    if let Ok(dir) = std::env::var("KAM_CACHE_DIR") {
        let p = PathBuf::from(dir);
        std::fs::create_dir_all(&p)?;
        return Ok(p);
    }

    let base = crate::utils::kam_home_dir()?;
    std::fs::create_dir_all(&base)?;
    Ok(base)
}

pub(super) fn index_cache_path(base_url: &str) -> Result<PathBuf, KamError> {
    let mut p = cache_root_dir()?;
    let fname = format!("index_{}.json", sanitize_filename(base_url));
    p.push(fname);
    Ok(p)
}

pub(super) fn module_cache_path(module_id: &str) -> Result<PathBuf, KamError> {
    let mut p = cache_root_dir()?;
    p.push("modules");
    std::fs::create_dir_all(&p)?;
    p.push(format!("{module_id}.json"));
    Ok(p)
}

pub(super) fn write_atomic(path: &Path, contents: &str) -> Result<(), KamError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub(super) fn fetch_index_cached(
    client: &Client,
    base_url: &str,
) -> Result<Vec<SearchEntry>, KamError> {
    let path = index_cache_path(base_url)?;
    let force_refresh = std::env::var("KAM_FORCE_INDEX_REFRESH").is_ok();

    if path.exists()
        && !force_refresh
        && let Ok(buf) = std::fs::read_to_string(&path)
        && let Ok(entries) = serde_json::from_str::<Vec<SearchEntry>>(&buf)
    {
        return Ok(entries);
    }

    if let Some(parent) = path.parent()
        && let Some(entries) = try_find_index_in_cache_dir(parent)
    {
        return Ok(entries);
    }

    let url = format!("{base_url}{SEARCH_INDEX_PATH}");
    match client
        .get(&url)
        .header(USER_AGENT, "kam/repo-search")
        .send()
    {
        Ok(resp) => {
            if !resp.status().is_success() {
                if let Some(parent) = path.parent()
                    && let Some(entries) = try_find_index_in_cache_dir(parent)
                {
                    return Ok(entries);
                }
                return Err(KamError::FetchFailed(format!(
                    "{url} returned status {}",
                    resp.status()
                )));
            }
            let body = resp
                .text()
                .map_err(|e| KamError::FetchFailed(format!("Failed to read {url} body: {e}")))?;
            let entries: Vec<SearchEntry> = serde_json::from_str(&body)
                .map_err(|e| KamError::Json(format!("Failed to parse {url} JSON: {e}")))?;
            let _ = write_atomic(&path, &body);
            Ok(entries)
        }
        Err(e) => {
            if let Some(parent) = path.parent()
                && let Some(entries) = try_find_index_in_cache_dir(parent)
            {
                return Ok(entries);
            }
            Err(KamError::FetchFailed(format!("GET {url} failed: {e}")))
        }
    }
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn try_find_index_in_cache_dir(dir: &Path) -> Option<Vec<SearchEntry>> {
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file()
                && let Some(name) = p.file_name().and_then(|n| n.to_str())
                && name.starts_with("index_")
                && std::path::Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                && let Ok(meta) = p.metadata()
                && let Ok(m) = meta.modified()
            {
                candidates.push((m, p));
            }
        }
    }
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.0));
    for (_mtime, p) in candidates {
        if let Ok(buf) = std::fs::read_to_string(&p)
            && let Ok(entries) = serde_json::from_str::<Vec<SearchEntry>>(&buf)
        {
            return Some(entries);
        }
    }
    None
}

pub(super) fn module_url(base_url: &str, module_id: &str) -> String {
    format!("{base_url}{MODULE_JSON_PREFIX}{module_id}.json")
}
