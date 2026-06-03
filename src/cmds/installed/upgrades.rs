use crate::cmds::repo;
use crate::errors::KamError;

use super::metadata::{InstalledModule, query_installed_modules};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeCandidate {
    pub id: String,
    pub installed_version: String,
    pub available_version: String,
    pub installed_version_code: String,
    pub module_name: String,
}

pub fn handle_upgrades(
    device: Option<String>,
    modules_url: Option<&str>,
    quiet: bool,
) -> Result<(), KamError> {
    let installed = query_installed_modules(device.as_deref())?;
    let upgrades = find_upgrade_candidates(&installed, modules_url)?;
    for upgrade in upgrades {
        if quiet {
            println!("{}", upgrade.id);
        } else {
            println!(
                "{id} {installed} -> {available} {name}",
                id = upgrade.id,
                installed = display_or_dash(&upgrade.installed_version),
                available = display_or_dash(&upgrade.available_version),
                name = display_or_dash(&upgrade.module_name)
            );
        }
    }
    Ok(())
}

pub fn find_upgrade_candidates(
    installed: &[InstalledModule],
    modules_url: Option<&str>,
) -> Result<Vec<UpgradeCandidate>, KamError> {
    let base = repo::effective_base_url(modules_url);
    let mut candidates = Vec::new();
    for module in installed {
        let Ok(Some((module_name, available_version))) =
            repo::cached_module_update_metadata(&module.id)
        else {
            continue;
        };
        if !is_available_newer(&module.version, &available_version) {
            continue;
        }
        if repo::cached_entry_exists(&base, &module.id).is_err() {
            continue;
        }
        candidates.push(UpgradeCandidate {
            id: module.id.clone(),
            installed_version: module.version.clone(),
            available_version,
            installed_version_code: module.version_code.clone(),
            module_name,
        });
    }
    candidates.sort_by_key(|candidate| candidate.id.to_ascii_lowercase());
    Ok(candidates)
}

fn is_available_newer(installed: &str, available: &str) -> bool {
    let installed_parts = version_numbers(installed);
    let available_parts = version_numbers(available);
    if installed_parts.is_empty() || available_parts.is_empty() {
        return false;
    }
    for idx in 0..installed_parts.len().max(available_parts.len()) {
        let installed_part = installed_parts.get(idx).copied().unwrap_or(0);
        let available_part = available_parts.get(idx).copied().unwrap_or(0);
        if available_part > installed_part {
            return true;
        }
        if available_part < installed_part {
            return false;
        }
    }
    false
}

fn version_numbers(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .take(3)
        .collect()
}

fn display_or_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
}

#[cfg(test)]
mod tests {
    use super::{is_available_newer, version_numbers};

    #[test]
    fn version_compare_requires_available_to_be_newer() {
        assert!(!is_available_newer("v1.2.3", "1.2.3"));
        assert!(is_available_newer("V1.2.3", "v1.2.4"));
        assert!(is_available_newer("1.2.3", "1.3.0"));
        assert!(!is_available_newer("1.2.3", "1.2.2"));
        assert!(!is_available_newer("v1.1.0", "v1.0.34"));
        assert_eq!(
            version_numbers("1.3.4 (746-d1b76b3-release)"),
            vec![1, 3, 4]
        );
    }
}
