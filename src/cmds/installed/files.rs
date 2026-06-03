use crate::errors::KamError;

use super::metadata::{InstalledModule, query_installed_modules, run_root_script};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesRequest {
    pub modules: Vec<String>,
    pub device: Option<String>,
    pub quiet: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleFileRecord {
    pub module_id: String,
    pub path: String,
}

/// List files inside installed module directories.
///
/// # Errors
///
/// Returns an error when adb/root queries fail, no module is supplied, a module
/// is not installed, or file listing fails on the device.
pub fn handle_files(request: &FilesRequest) -> Result<(), KamError> {
    if request.modules.is_empty() {
        return Err(KamError::CommandFailed(
            "Files query requires a module id, e.g. `kam -Ql MagicNet`".to_string(),
        ));
    }

    let installed = query_installed_modules(request.device.as_deref())?;
    let modules = select_modules(&installed, &request.modules)?;
    for module in modules {
        let records = query_module_files(&module, request.device.as_deref())?;
        for record in records {
            if request.quiet {
                println!("{}", record.path);
            } else {
                println!("{} {}", record.module_id, record.path);
            }
        }
    }
    Ok(())
}

fn select_modules(
    installed: &[InstalledModule],
    requested_modules: &[String],
) -> Result<Vec<InstalledModule>, KamError> {
    let mut selected = Vec::new();
    for requested in requested_modules {
        let Some(module) = installed
            .iter()
            .find(|module| matches_module(module, requested))
        else {
            return Err(KamError::PackageNotFound(format!(
                "Installed module not found: {requested}"
            )));
        };
        selected.push(module.clone());
    }
    Ok(selected)
}

fn query_module_files(
    module: &InstalledModule,
    device: Option<&str>,
) -> Result<Vec<ModuleFileRecord>, KamError> {
    let script = files_script(&module.id, &module.path);
    let output = run_root_script(device, &script)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(KamError::CommandFailed(format!(
            "Failed to list files for {}: {}",
            module.id,
            stderr.trim()
        )));
    }
    Ok(parse_file_records(&stdout))
}

#[must_use]
pub fn parse_file_records(input: &str) -> Vec<ModuleFileRecord> {
    input
        .lines()
        .filter_map(|line| {
            let (module_id, path) = line.split_once('\t')?;
            Some(ModuleFileRecord {
                module_id: module_id.to_string(),
                path: path.to_string(),
            })
        })
        .collect()
}

fn files_script(module_id: &str, module_path: &str) -> String {
    format!(
        r#"module_id={module_id}
module_path={module_path}
[ -d "$module_path" ] || exit 1
find "$module_path" -mindepth 1 -print 2>/dev/null | sort | while IFS= read -r path; do
  printf '%s\t%s\n' "$module_id" "$path"
done
"#,
        module_id = shell_quote(module_id),
        module_path = shell_quote(module_path)
    )
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
    use super::{parse_file_records, shell_quote};

    #[test]
    fn parses_module_file_records() {
        let records = parse_file_records(
            "MagicNet\t/data/adb/modules/MagicNet/module.prop\n\
             MagicNet\t/data/adb/modules/MagicNet/cli\n",
        );

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].module_id, "MagicNet");
        assert_eq!(records[1].path, "/data/adb/modules/MagicNet/cli");
    }

    #[test]
    fn shell_quote_wraps_spaces() {
        assert_eq!(shell_quote("MagicNet"), "MagicNet");
        assert_eq!(shell_quote("demo module"), "'demo module'");
    }
}
