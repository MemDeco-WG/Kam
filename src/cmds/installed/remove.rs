use crate::errors::KamError;
use crate::utils::Utils;

use super::metadata::{InstalledModule, query_installed_modules, run_root_script};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveRequest {
    pub modules: Vec<String>,
    pub device: Option<String>,
    pub dry_run: bool,
    pub assume_yes: bool,
    pub quiet: bool,
}

/// # Errors
/// Returns `KamError` when module discovery fails or a requested module cannot be removed.
pub fn handle_remove(request: &RemoveRequest) -> Result<(), KamError> {
    if request.modules.is_empty() {
        return Err(KamError::CommandFailed(
            "Remove requires an installed module id, e.g. `kam -R moduleId`".to_string(),
        ));
    }
    let installed = query_installed_modules(request.device.as_deref())?;
    let mut targets = Vec::new();
    for requested in &request.modules {
        let Some(module) = installed
            .iter()
            .find(|module| matches_module(module, requested))
        else {
            return Err(KamError::PackageNotFound(format!(
                "Installed module not found: {requested}"
            )));
        };
        targets.push(module.clone());
    }

    if request.dry_run {
        for module in &targets {
            println!("touch {}/remove", module.path);
        }
        return Ok(());
    }

    if !request.assume_yes && !confirm_remove(&targets)? {
        Utils::warn("Remove cancelled.");
        return Ok(());
    }

    for module in targets {
        mark_remove(&module, request.device.as_deref())?;
        if !request.quiet {
            Utils::success(format!(
                "Marked {} for removal. Reboot for the manager to apply it.",
                module.id
            ));
        }
    }
    Ok(())
}

fn mark_remove(module: &InstalledModule, device: Option<&str>) -> Result<(), KamError> {
    let script = format!(
        "set -eu\n[ -d {path} ]\ntouch {path}/remove\n",
        path = shell_quote(&module.path)
    );
    let output = run_root_script(device, &script)?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(KamError::CommandFailed(format!(
            "Failed to mark {} for removal: {}",
            module.id,
            stderr.trim()
        )))
    }
}

fn confirm_remove(targets: &[InstalledModule]) -> Result<bool, KamError> {
    let names = targets
        .iter()
        .map(|module| module.id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    dialoguer::Confirm::new()
        .with_prompt(format!("Mark installed module(s) for removal: {names}?"))
        .default(false)
        .interact()
        .map_err(|err| KamError::CommandFailed(format!("Prompt failed: {err}")))
}

fn matches_module(module: &InstalledModule, requested: &str) -> bool {
    module.id.eq_ignore_ascii_case(requested) || module.name.eq_ignore_ascii_case(requested)
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

#[cfg(test)]
mod tests {
    use super::shell_quote;

    #[test]
    fn shell_quote_wraps_spaces() {
        assert_eq!(
            shell_quote("/data/adb/modules/demo"),
            "/data/adb/modules/demo"
        );
        assert_eq!(
            shell_quote("/data/adb/modules/demo module"),
            "'/data/adb/modules/demo module'"
        );
    }
}
