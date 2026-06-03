use super::{ModuleDetail, cache, download, search};
use crate::errors::KamError;
use reqwest::blocking::Client;
use std::time::Duration;

pub(crate) fn handle_repo_urls(
    modules: &[String],
    modules_url: Option<&str>,
    quiet: bool,
) -> Result<(), KamError> {
    if modules.is_empty() {
        return Err(KamError::CommandFailed(
            "Package URL print requires a module id, e.g. `-Sp <moduleId>`".into(),
        ));
    }
    let base = super::effective_base_url(modules_url);
    for module_id in modules {
        cache::find_entry_by_name(&base, module_id)?;
        let md = download::read_module_detail_from_cache(module_id)?;
        let Some(url) = download::selected_zip_asset_url(&md) else {
            return Err(KamError::PackageNotFound(format!(
                "No downloadable zip asset found for module {module_id}"
            )));
        };
        if quiet || modules.len() == 1 {
            println!("{url}");
        } else {
            println!("{module_id} {url}");
        }
    }
    Ok(())
}

pub(crate) fn download_targets(
    targets: &[String],
    base_url: &str,
    assume_yes: bool,
    quiet: bool,
    fetch_only: bool,
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
            let action = if fetch_only { "Fetch" } else { "Download" };
            crate::utils::Utils::section(format!("{action}: {module_id}"));
        }
        match cache::find_entry_by_name(base_url, module_id)
            .and_then(|_| download::read_module_detail_from_cache(module_id))
        {
            Ok(md) => {
                download_cached_or_local(&md, module_id, &client, assume_yes, quiet, fetch_only)?;
            }
            Err(KamError::PackageNotFound(_)) => {
                handle_missing_module(module_id, base_url, &client, assume_yes, quiet, fetch_only)?;
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
    fetch_only: bool,
) -> Result<(), KamError> {
    crate::utils::Utils::warn(trf!("repo.module_not_found_showing_similar", module_id));
    if let Some(selected_module) = search::search_local_interactive(module_id, base_url)? {
        crate::utils::Utils::info(trf!("repo.selected_module", selected_module));
        let md = download::read_module_detail_from_cache(&selected_module)?;
        download_cached_or_local(&md, &selected_module, client, assume_yes, quiet, fetch_only)?;
    } else {
        crate::utils::Utils::info(crate::i18n::tr("repo.skipped_selection"));
    }
    Ok(())
}

fn download_cached_or_local(
    md: &ModuleDetail,
    module_id: &str,
    client: &Client,
    assume_yes: bool,
    quiet: bool,
    fetch_only: bool,
) -> Result<(), KamError> {
    if fetch_only {
        let package_dir = cache::package_cache_dir()?;
        download::process_module_download_to_dir(
            md,
            module_id,
            client,
            assume_yes,
            quiet,
            Some(&package_dir),
        )?;
    } else {
        download::process_module_download(md, module_id, client, assume_yes, quiet)?;
    }
    Ok(())
}
