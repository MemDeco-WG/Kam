use std::collections::HashSet;

use crate::cmds::repo;
use crate::errors::KamError;

use super::metadata::{InstalledModule, query_installed_modules};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OriginFilter {
    Native,
    Foreign,
}

pub fn handle_origin_filter(
    filter: OriginFilter,
    device: Option<String>,
    modules_url: Option<&str>,
    quiet: bool,
) -> Result<(), KamError> {
    let installed = query_installed_modules(device.as_deref())?;
    let modules = classify_installed_modules(&installed, filter, modules_url)?;
    for module in modules {
        if quiet {
            println!("{}", module.id);
        } else {
            println!(
                "{id} {version} {name}",
                id = module.id,
                version = display_or_dash(&module.version),
                name = display_or_dash(&module.name)
            );
        }
    }
    Ok(())
}

pub fn classify_installed_modules(
    installed: &[InstalledModule],
    filter: OriginFilter,
    modules_url: Option<&str>,
) -> Result<Vec<InstalledModule>, KamError> {
    let indexed = indexed_module_ids(modules_url)?;
    let mut modules = installed
        .iter()
        .filter(|module| {
            let is_native = indexed.contains(&module.id.to_ascii_lowercase());
            matches!(
                (filter, is_native),
                (OriginFilter::Native, true) | (OriginFilter::Foreign, false)
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    modules.sort_by_key(|module| module.id.to_ascii_lowercase());
    Ok(modules)
}

fn indexed_module_ids(modules_url: Option<&str>) -> Result<HashSet<String>, KamError> {
    let entries = repo::read_cached_index(modules_url)?;
    Ok(entries
        .into_iter()
        .map(|entry| entry.name.to_ascii_lowercase())
        .collect())
}

fn display_or_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
}
