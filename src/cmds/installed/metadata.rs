use std::collections::BTreeMap;
use std::io::Write;
use std::process::{Command, Stdio};

use crate::errors::KamError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledModule {
    pub id: String,
    pub name: String,
    pub version: String,
    pub version_code: String,
    pub author: String,
    pub description: String,
    pub state: ModuleState,
    pub path: String,
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleState {
    Enabled,
    Disabled,
    RemovePending,
}

pub fn query_installed_modules(device: Option<&str>) -> Result<Vec<InstalledModule>, KamError> {
    ensure_adb()?;
    let output = run_root_script(device, installed_modules_script())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(KamError::CommandFailed(format!(
            "Failed to query installed modules: {}",
            stderr.trim()
        )));
    }
    Ok(parse_installed_modules(&stdout))
}

pub fn run_root_script(
    device: Option<&str>,
    script: &str,
) -> Result<std::process::Output, KamError> {
    ensure_adb()?;
    adb_root_output(device, script)
}

pub fn parse_installed_modules(input: &str) -> Vec<InstalledModule> {
    let mut modules = Vec::new();
    let mut current = BTreeMap::new();
    let mut in_module = false;
    for line in input.lines() {
        match line.trim() {
            "__kam_module_begin__" => {
                current.clear();
                in_module = true;
            }
            "__kam_module_end__" => {
                if in_module && let Some(module) = module_from_properties(&current) {
                    modules.push(module);
                }
                current = BTreeMap::new();
                in_module = false;
            }
            _ if in_module => {
                if let Some((key, value)) = line.split_once('=') {
                    current.insert(key.trim().to_string(), value.trim().to_string());
                }
            }
            _ => {}
        }
    }
    modules
}

fn module_from_properties(properties: &BTreeMap<String, String>) -> Option<InstalledModule> {
    let path = properties.get("path").cloned().unwrap_or_default();
    let id = properties
        .get("id")
        .cloned()
        .or_else(|| path.rsplit('/').next().map(str::to_string))?;
    Some(InstalledModule {
        id,
        name: properties.get("name").cloned().unwrap_or_default(),
        version: properties.get("version").cloned().unwrap_or_default(),
        version_code: properties.get("versionCode").cloned().unwrap_or_default(),
        author: properties.get("author").cloned().unwrap_or_default(),
        description: properties.get("description").cloned().unwrap_or_default(),
        state: properties
            .get("state")
            .map_or(ModuleState::Enabled, |value| ModuleState::from_str(value)),
        path,
        properties: properties.clone(),
    })
}

fn adb_root_output(device: Option<&str>, script: &str) -> Result<std::process::Output, KamError> {
    let mut cmd = adb(device);
    cmd.arg("shell")
        .arg("su")
        .arg("-c")
        .arg("sh")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(KamError::Io)?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(script.as_bytes()).map_err(KamError::Io)?;
        stdin.write_all(b"\n").map_err(KamError::Io)?;
    }
    child.wait_with_output().map_err(KamError::Io)
}

fn adb(device: Option<&str>) -> Command {
    let mut cmd = Command::new("adb");
    if let Some(device) = normalize_device(device) {
        cmd.arg("-s").arg(device);
    }
    cmd
}

fn installed_modules_script() -> &'static str {
    r#"for d in /data/adb/modules/*; do
  [ -f "$d/module.prop" ] || continue
  state=enabled
  [ -e "$d/disable" ] && state=disabled
  [ -e "$d/remove" ] && state=remove-pending
  printf '__kam_module_begin__\n'
  printf 'path=%s\n' "$d"
  printf 'state=%s\n' "$state"
  sed -n 's/\r$//;/^[[:space:]]*#/d;/^[[:space:]]*$/d;/=/p' "$d/module.prop"
  printf '__kam_module_end__\n'
done"#
}

fn normalize_device(device: Option<&str>) -> Option<&str> {
    device.filter(|value| !value.eq_ignore_ascii_case("auto"))
}

fn ensure_adb() -> Result<(), KamError> {
    if crate::utils::command_exists("adb") {
        Ok(())
    } else {
        Err(KamError::CommandFailed(
            "adb not found on PATH. Install Android platform-tools.".to_string(),
        ))
    }
}

impl ModuleState {
    fn from_str(value: &str) -> Self {
        match value {
            "disabled" => Self::Disabled,
            "remove-pending" => Self::RemovePending,
            _ => Self::Enabled,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::RemovePending => "remove-pending",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ModuleState, parse_installed_modules};

    #[test]
    fn parses_installed_module_blocks() {
        let modules = parse_installed_modules(
            "__kam_module_begin__\n\
             path=/data/adb/modules/MagicNet\n\
             state=disabled\n\
             id=MagicNet\n\
             name=MagicNet\n\
             version=1.2.3\n\
             versionCode=42\n\
             author=LIghtJUNction\n\
             description=Proxy module\n\
             __kam_module_end__\n",
        );

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].id, "MagicNet");
        assert_eq!(modules[0].state, ModuleState::Disabled);
        assert_eq!(modules[0].version_code, "42");
    }

    #[test]
    fn falls_back_to_directory_name_when_id_is_missing() {
        let modules = parse_installed_modules(
            "__kam_module_begin__\n\
             path=/data/adb/modules/demo\n\
             state=enabled\n\
             name=Demo Module\n\
             __kam_module_end__\n",
        );

        assert_eq!(modules[0].id, "demo");
    }
}
