use crate::errors::KamError;
use crate::utils::Utils;

mod args;
mod check;
mod files;
mod metadata;
mod origin;
mod owner;
mod package_info;
mod remove;
mod upgrades;

pub use args::{
    InstalledArgs, InstalledCheckArgs, InstalledCommand, InstalledFilesArgs, InstalledInfoArgs,
    InstalledListArgs, InstalledOriginArgs, InstalledOwnerArgs, InstalledPackageInfoArgs,
    InstalledRemoveArgs, InstalledSearchArgs, InstalledUpgradesArgs, PacmanQueryMode,
    PacmanQueryRequest,
};
pub use check::{CheckRequest, handle_check};
pub use files::{FilesRequest, handle_files};
use metadata::query_installed_modules;
pub use metadata::{InstalledModule, ModuleState, parse_installed_modules};
pub use origin::{OriginFilter, handle_origin_filter};
pub use owner::{OwnerRequest, handle_owner};
pub use package_info::{PackageInfoRequest, handle_package_files, handle_package_info};
pub use remove::{RemoveRequest, handle_remove};
pub use upgrades::handle_upgrades;

/// Run explicit installed-module subcommands.
///
/// # Errors
///
/// Returns an error when adb/root queries fail, requested modules are missing,
/// or a subcommand-specific validation fails.
pub fn run(args: &InstalledArgs) -> Result<(), KamError> {
    match &args.command {
        Some(InstalledCommand::List(list)) => {
            let device = list.device.as_ref().or(args.device.as_ref());
            handle_list(&list.query.join(" "), device.map(String::as_str), false)
        }
        Some(InstalledCommand::Search(search)) => {
            let device = search.device.as_ref().or(args.device.as_ref());
            handle_search(&search.query.join(" "), device.map(String::as_str), false)
        }
        Some(InstalledCommand::Info(info)) => {
            let device = info.device.as_ref().or(args.device.as_ref());
            handle_info(&info.modules, device.map(String::as_str))
        }
        Some(InstalledCommand::Upgrades(upgrades)) => {
            let device = upgrades.device.as_ref().or(args.device.as_ref());
            handle_upgrades(device.map(String::as_str), None, upgrades.quiet)
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
        Some(InstalledCommand::Foreign(origin)) => {
            let device = origin.device.as_ref().or(args.device.as_ref());
            handle_origin_filter(
                OriginFilter::Foreign,
                device.map(String::as_str),
                None,
                origin.quiet,
            )
        }
        Some(InstalledCommand::Native(origin)) => {
            let device = origin.device.as_ref().or(args.device.as_ref());
            handle_origin_filter(
                OriginFilter::Native,
                device.map(String::as_str),
                None,
                origin.quiet,
            )
        }
        Some(InstalledCommand::Check(check)) => {
            let device = check.device.as_ref().or(args.device.as_ref()).cloned();
            handle_check(&CheckRequest {
                modules: check.modules.clone(),
                device,
                quiet: check.quiet,
            })
        }
        Some(InstalledCommand::Owner(owner)) => {
            let device = owner.device.as_ref().or(args.device.as_ref()).cloned();
            handle_owner(&OwnerRequest {
                paths: owner.paths.clone(),
                device,
                quiet: owner.quiet,
            })
        }
        Some(InstalledCommand::Files(files)) => {
            let device = files.device.as_ref().or(args.device.as_ref()).cloned();
            handle_files(&FilesRequest {
                modules: files.modules.clone(),
                device,
                quiet: files.quiet,
            })
        }
        Some(InstalledCommand::PackageInfo(package)) => handle_package_info(&PackageInfoRequest {
            packages: package.packages.clone(),
            quiet: package.quiet,
        }),
        Some(InstalledCommand::PackageFiles(package)) => {
            handle_package_files(&PackageInfoRequest {
                packages: package.packages.clone(),
                quiet: package.quiet,
            })
        }
        None => handle_list("", args.device.as_deref(), false),
    }
}

/// Run pacman-style `kam -Q...` installed-module queries.
///
/// # Errors
///
/// Returns an error when adb/root queries fail, cached repository metadata is
/// unavailable for origin/upgrade checks, or requested modules are missing.
pub fn handle_pacman_style(request: &PacmanQueryRequest) -> Result<(), KamError> {
    match request.mode {
        PacmanQueryMode::Upgrades => handle_upgrades(
            request.device.as_deref(),
            request.modules_url.as_deref(),
            request.quiet,
        ),
        PacmanQueryMode::Foreign => handle_origin_filter(
            OriginFilter::Foreign,
            request.device.as_deref(),
            request.modules_url.as_deref(),
            request.quiet,
        ),
        PacmanQueryMode::Native => handle_origin_filter(
            OriginFilter::Native,
            request.device.as_deref(),
            request.modules_url.as_deref(),
            request.quiet,
        ),
        PacmanQueryMode::Check => handle_check(&CheckRequest {
            modules: request.targets.clone(),
            device: request.device.clone(),
            quiet: request.quiet,
        }),
        PacmanQueryMode::Owner => handle_owner(&OwnerRequest {
            paths: request.targets.clone(),
            device: request.device.clone(),
            quiet: request.quiet,
        }),
        PacmanQueryMode::Files => handle_files(&FilesRequest {
            modules: request.targets.clone(),
            device: request.device.clone(),
            quiet: request.quiet,
        }),
        PacmanQueryMode::Package => {
            let packages = request
                .targets
                .iter()
                .map(std::path::PathBuf::from)
                .collect();
            handle_package_info(&PackageInfoRequest {
                packages,
                quiet: request.quiet,
            })
        }
        PacmanQueryMode::PackageFiles => {
            let packages = request
                .targets
                .iter()
                .map(std::path::PathBuf::from)
                .collect();
            handle_package_files(&PackageInfoRequest {
                packages,
                quiet: request.quiet,
            })
        }
        PacmanQueryMode::Info => handle_info(&request.targets, request.device.as_deref()),
        PacmanQueryMode::Search => {
            if request.targets.is_empty() {
                return Err(KamError::CommandFailed(
                    "Search requires a query, e.g. `kam -Qs <term>`".to_string(),
                ));
            }
            handle_search(
                &request.targets.join(" "),
                request.device.as_deref(),
                request.quiet,
            )
        }
        PacmanQueryMode::List => handle_list(
            &request.targets.join(" "),
            request.device.as_deref(),
            request.quiet,
        ),
    }
}

fn handle_list(query: &str, device: Option<&str>, quiet: bool) -> Result<(), KamError> {
    let mut modules = query_installed_modules(device)?;
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

fn handle_search(query: &str, device: Option<&str>, quiet: bool) -> Result<(), KamError> {
    handle_list(query, device, quiet)
}

fn handle_info(modules: &[String], device: Option<&str>) -> Result<(), KamError> {
    if modules.is_empty() {
        return Err(KamError::CommandFailed(
            "Info requires a module id, e.g. `kam -Qi <moduleId>`".to_string(),
        ));
    }
    let installed = query_installed_modules(device)?;
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
