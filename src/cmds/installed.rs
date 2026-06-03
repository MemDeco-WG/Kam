use clap::{Args, Subcommand};
use std::collections::BTreeMap;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;

use crate::errors::KamError;
use crate::utils::Utils;

#[derive(Args, Debug, Clone)]
pub struct InstalledArgs {
    /// Subcommands for installed module queries.
    #[command(subcommand)]
    pub command: Option<InstalledCommand>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum InstalledCommand {
    /// List installed modules from /data/adb/modules.
    List(InstalledListArgs),
    /// Search installed module metadata.
    Search(InstalledSearchArgs),
    /// Show installed module metadata.
    Info(InstalledInfoArgs),
}

#[derive(Args, Debug, Clone)]
pub struct InstalledListArgs {
    /// Optional query to filter module id, name, author, or description.
    #[arg(value_name = "QUERY", num_args = 0..)]
    pub query: Vec<String>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct InstalledSearchArgs {
    /// Search terms.
    #[arg(value_name = "QUERY", required = true, num_args = 1..)]
    pub query: Vec<String>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct InstalledInfoArgs {
    /// Installed module ids or names.
    #[arg(value_name = "MODULE", required = true, num_args = 1..)]
    pub modules: Vec<String>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,
}

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

pub fn run(args: &InstalledArgs) -> Result<(), KamError> {
    match &args.command {
        Some(InstalledCommand::List(list)) => {
            let device = list.device.as_ref().or(args.device.as_ref());
            handle_list(&list.query.join(" "), device.cloned(), false)
        }
        Some(InstalledCommand::Search(search)) => {
            let device = search.device.as_ref().or(args.device.as_ref());
            handle_search(&search.query.join(" "), device.cloned(), false)
        }
        Some(InstalledCommand::Info(info)) => {
            let device = info.device.as_ref().or(args.device.as_ref());
            handle_info(&info.modules, device.cloned())
        }
        None => handle_list("", args.device.clone(), false),
    }
}

pub fn handle_pacman_style(
    search: bool,
    info: bool,
    targets: &[String],
    device: Option<String>,
    quiet: bool,
) -> Result<(), KamError> {
    if info {
        return handle_info(targets, device);
    }
    if search {
        if targets.is_empty() {
            return Err(KamError::CommandFailed(
                "Search requires a query, e.g. `kam -Qs <term>`".to_string(),
            ));
        }
        return handle_search(&targets.join(" "), device, quiet);
    }
    handle_list(&targets.join(" "), device, quiet)
}

fn handle_list(query: &str, device: Option<String>, quiet: bool) -> Result<(), KamError> {
    let mut modules = query_installed_modules(device.as_deref())?;
    modules.sort_by_key(|module| module.id.to_ascii_lowercase());
    let query = query.trim();
    for module in modules {
        if !query.is_empty() && !matches_query(&module, query) {
            continue;
        }
        if quiet {
            println!("{}", module.id);
        } else {
            println!(
                "{id} {version} [{state}] {name}",
                id = module.id,
                version = display_or_dash(&module.version),
                state = module.state.as_str(),
                name = display_or_dash(&module.name)
            );
        }
    }
    Ok(())
}

fn handle_search(query: &str, device: Option<String>, quiet: bool) -> Result<(), KamError> {
    handle_list(query, device, quiet)
}

fn handle_info(modules: &[String], device: Option<String>) -> Result<(), KamError> {
    if modules.is_empty() {
        return Err(KamError::CommandFailed(
            "Info requires a module id, e.g. `kam -Qi <moduleId>`".to_string(),
        ));
    }
    let installed = query_installed_modules(device.as_deref())?;
    for requested in modules {
        let Some(module) = installed
            .iter()
            .find(|module| matches_module(module, requested))
        else {
            return Err(KamError::PackageNotFound(format!(
                "Installed module not found: {requested}"
            )));
        };
        print_module_info(module);
    }
    Ok(())
}

fn query_installed_modules(device: Option<&str>) -> Result<Vec<InstalledModule>, KamError> {
    ensure_adb()?;
    let output = adb_root_output(device, installed_modules_script())?;
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

fn print_module_info(module: &InstalledModule) {
    Utils::section(&module.id);
    println!("Name           : {}", display_or_dash(&module.name));
    println!("Version        : {}", display_or_dash(&module.version));
    println!("Version Code   : {}", display_or_dash(&module.version_code));
    println!("Author         : {}", display_or_dash(&module.author));
    println!("Description    : {}", display_or_dash(&module.description));
    println!("State          : {}", module.state.as_str());
    println!("Path           : {}", display_or_dash(&module.path));
}

fn matches_module(module: &InstalledModule, requested: &str) -> bool {
    module.id.eq_ignore_ascii_case(requested) || module.name.eq_ignore_ascii_case(requested)
}

fn matches_query(module: &InstalledModule, query: &str) -> bool {
    let haystack = format!(
        "{}\n{}\n{}\n{}\n{}",
        module.id, module.name, module.version, module.author, module.description
    )
    .to_ascii_lowercase();
    query
        .split_whitespace()
        .all(|term| haystack.contains(&term.to_ascii_lowercase()))
}

fn display_or_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
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

    fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::RemovePending => "remove-pending",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ModuleState, matches_query, parse_installed_modules};

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

    #[test]
    fn query_matches_multiple_metadata_fields() {
        let modules = parse_installed_modules(
            "__kam_module_begin__\n\
             path=/data/adb/modules/demo\n\
             state=enabled\n\
             id=demo\n\
             name=Demo Module\n\
             author=Alice\n\
             description=KernelSU helper\n\
             __kam_module_end__\n",
        );

        assert!(matches_query(&modules[0], "demo kernelsu"));
        assert!(matches_query(&modules[0], "alice"));
        assert!(!matches_query(&modules[0], "magisk only"));
    }
}
