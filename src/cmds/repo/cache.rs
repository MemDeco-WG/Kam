use super::{MODULE_JSON_PREFIX, SearchEntry};
use crate::errors::KamError;
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

pub(crate) fn index_cache_path(base_url: &str) -> Result<PathBuf, KamError> {
    let mut p = cache_root_dir()?;
    let fname = format!("index_{}.json", sanitize_filename(base_url));
    p.push(fname);
    Ok(p)
}

pub(crate) fn module_cache_path(module_id: &str) -> Result<PathBuf, KamError> {
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

pub(super) fn read_local_index(base_url: &str) -> Result<Vec<SearchEntry>, KamError> {
    let path = index_cache_path(base_url)?;
    if path.exists()
        && let Ok(buf) = std::fs::read_to_string(&path)
    {
        return serde_json::from_str::<Vec<SearchEntry>>(&buf)
            .map_err(|e| KamError::Json(format!("Failed to parse cached index JSON: {e}")));
    }

    let Some(parent) = path.parent() else {
        return Err(missing_index_error(base_url));
    };
    try_find_index_in_cache_dir(parent).ok_or_else(|| missing_index_error(base_url))
}

pub(super) fn find_entry_by_name(base_url: &str, module_id: &str) -> Result<SearchEntry, KamError> {
    let entries = read_local_index(base_url)?;
    entries
        .into_iter()
        .find(|entry| entry.name == module_id)
        .ok_or_else(|| {
            KamError::PackageNotFound(format!(
                "Package '{module_id}' was not found in the local module index. Run `kam -Sy` first."
            ))
        })
}

pub(super) fn missing_index_error(base_url: &str) -> KamError {
    KamError::PackageNotFound(format!(
        "No local module index for {base_url}. Run `kam -Sy` or `kam repo sync` first."
    ))
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
