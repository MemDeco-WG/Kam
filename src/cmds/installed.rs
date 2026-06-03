use clap::{Args, Subcommand};

use crate::errors::KamError;
use crate::utils::Utils;

mod metadata;
mod remove;
mod upgrades;

use metadata::query_installed_modules;
pub use metadata::{InstalledModule, ModuleState, parse_installed_modules};
pub use remove::{RemoveRequest, handle_remove};
pub use upgrades::handle_upgrades;

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
    /// List installed modules with a newer cached repository release.
    Upgrades(InstalledUpgradesArgs),
    /// Mark installed modules for removal.
    Remove(InstalledRemoveArgs),
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

#[derive(Args, Debug, Clone)]
pub struct InstalledUpgradesArgs {
    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,

    /// Suppress details and print only module ids.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

#[derive(Args, Debug, Clone)]
pub struct InstalledRemoveArgs {
    /// Installed module ids or names to mark for removal.
    #[arg(value_name = "MODULE", required = true, num_args = 1..)]
    pub modules: Vec<String>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,

    /// Print planned removal marker writes without changing the device.
    #[arg(long)]
    pub dry_run: bool,

    /// Assume yes to confirmation prompts.
    #[arg(short = 'y', long = "yes")]
    pub assume_yes: bool,

    /// Suppress non-essential output.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
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
        Some(InstalledCommand::Upgrades(upgrades)) => {
            let device = upgrades.device.as_ref().or(args.device.as_ref());
            handle_upgrades(device.cloned(), None, upgrades.quiet)
        }
        Some(InstalledCommand::Remove(remove)) => {
            let device = remove.device.as_ref().or(args.device.as_ref()).cloned();
            handle_remove(&RemoveRequest {
                modules: remove.modules.clone(),
                device,
                dry_run: remove.dry_run,
                assume_yes: remove.assume_yes,
                quiet: remove.quiet,
            })
        }
        None => handle_list("", args.device.clone(), false),
    }
}

pub fn handle_pacman_style(
    search: bool,
    info: bool,
    upgrades: bool,
    targets: &[String],
    device: Option<String>,
    modules_url: Option<&str>,
    quiet: bool,
) -> Result<(), KamError> {
    if upgrades {
        return handle_upgrades(device, modules_url, quiet);
    }
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

#[cfg(test)]
mod tests {
    use super::{matches_query, parse_installed_modules};

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
