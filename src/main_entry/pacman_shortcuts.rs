fn handle_top_level_repo_flags(cli: &Cli) -> bool {
    let repo_refresh = cli.update_index || pacman_sync_refresh_requested();
    let repo_clean = cli.clean_flag || pacman_sync_clean_requested();
    let repo_assume_yes = repo_assume_yes(cli.assume_yes);

    if repo_clean {
        match kam::cmds::cache::handler::clean_module_cache() {
            Ok(()) => return true,
            Err(e) => {
                print_error_chain(&e);
                std::process::exit(1);
            }
        }
    }

    if repo_refresh && !cli.sync_flag && !cli.search_flag && !cli.info_flag && !cli.list_flag {
        let base = kam::cmds::repo::effective_base_url(cli.modules_url.as_deref());
        match kam::cmds::repo::repo_sync_with_jobs(&base, true, None::<usize>, cli.quiet) {
            Ok(()) => return true,
            Err(e) => {
                print_error_chain(&e);
                std::process::exit(1);
            }
        }
    }

    if cli.sync_flag || cli.search_flag || cli.info_flag || cli.list_flag {
        let effective_targets = repo_targets(&cli.targets);
        let effective_quiet = cli.quiet || repo_quiet_requested();
        let repo_info = cli.info_flag || pacman_sync_info_requested();
        let repo_list = cli.list_flag || pacman_sync_list_requested();

        if repo_refresh {
            let base = kam::cmds::repo::effective_base_url(cli.modules_url.as_deref());
            match kam::cmds::repo::repo_sync_with_jobs(&base, true, None::<usize>, effective_quiet) {
                Ok(()) => {}
                Err(e) => {
                    print_error_chain(&e);
                    std::process::exit(1);
                }
            }
            if cli.sync_flag && !cli.search_flag && effective_targets.is_empty() {
                return true;
            }
        }

        match kam::cmds::repo::handle_pacman_style(
            cli.sync_flag,
            cli.search_flag,
            repo_info,
            repo_list,
            &effective_targets,
            repo_assume_yes,
            cli.modules_url.as_deref(),
            effective_quiet,
        ) {
            Ok(()) => return true,
            Err(e) => {
                print_error_chain(&e);
                std::process::exit(1);
            }
        }
    }

    false
}

fn handle_top_level_installed_flags(cli: &Cli) -> bool {
    if !cli.query_flag {
        return false;
    }
    let effective_targets = repo_targets(&cli.targets);
    let effective_quiet = cli.quiet || query_quiet_requested();
    let query_mode = pacman_query_mode(cli);
    let device = global_device(cli).or_else(|| target_option_value(&cli.targets, "--device"));
    let request = kam::cmds::installed::PacmanQueryRequest {
        mode: query_mode,
        targets: effective_targets,
        device: device.filter(|value| !value.eq_ignore_ascii_case("auto")),
        modules_url: cli.modules_url.clone(),
        quiet: effective_quiet,
    };
    match kam::cmds::installed::handle_pacman_style(&request) {
        Ok(()) => true,
        Err(e) => {
            print_error_chain(&e);
            std::process::exit(1);
        }
    }
}

fn handle_top_level_local_install_flags(cli: &Cli) -> bool {
    if !cli.local_install_flag {
        return false;
    }
    let targets = repo_targets(&cli.targets);
    if targets.is_empty() {
        print_error_chain(&KamError::CommandFailed(
            "Local package install requires a zip path, e.g. `kam -U module.zip`".to_string(),
        ));
        std::process::exit(1);
    }
    if targets.len() > 1 {
        print_error_chain(&KamError::CommandFailed(
            "Local package install accepts one zip path at a time.".to_string(),
        ));
        std::process::exit(1);
    }
    let args = kam::cmds::install::InstallArgs {
        path: Some(std::path::PathBuf::from(&targets[0])),
        manager: target_option_value(&cli.targets, "--manager").or_else(|| cli.manager.clone()),
        dry_run: cli.dry_run || target_flag_present(&cli.targets, "--dry-run"),
        adb: cli.adb || target_flag_present(&cli.targets, "--adb"),
        verbose: has_local_install_short('v'),
        quiet: cli.quiet || has_local_install_short('q'),
        assume_yes: cli.assume_yes || has_local_install_short('y'),
    };
    match kam::cmds::install::run(&args) {
        Ok(()) => true,
        Err(e) => {
            print_error_chain(&e);
            std::process::exit(1);
        }
    }
}

fn handle_top_level_remove_flags(cli: &Cli) -> bool {
    if !cli.remove_flag {
        return false;
    }
    let targets = repo_targets(&cli.targets);
    let device = global_device(cli).or_else(|| target_option_value(&cli.targets, "--device"));
    let request = kam::cmds::installed::RemoveRequest {
        modules: targets,
        device: device.filter(|value| !value.eq_ignore_ascii_case("auto")),
        dry_run: cli.dry_run || target_flag_present(&cli.targets, "--dry-run"),
        assume_yes: cli.assume_yes || has_remove_short('y'),
        quiet: cli.quiet || has_remove_short('q'),
    };
    match kam::cmds::installed::handle_remove(&request) {
        Ok(()) => true,
        Err(e) => {
            print_error_chain(&e);
            std::process::exit(1);
        }
    }
}

fn pacman_sync_refresh_requested() -> bool {
    has_sync_short('y')
}

fn pacman_sync_info_requested() -> bool {
    has_sync_short('i')
}

fn pacman_sync_list_requested() -> bool {
    has_sync_short('l')
}

fn pacman_sync_clean_requested() -> bool {
    has_sync_short('c')
}

fn pacman_query_info_requested() -> bool {
    has_query_short('i')
}

fn pacman_query_search_requested() -> bool {
    has_query_short('s')
}

fn pacman_query_upgrades_requested() -> bool {
    has_query_short('u')
}

fn pacman_query_foreign_requested() -> bool {
    has_query_short('m')
}

fn pacman_query_native_requested() -> bool {
    has_query_short('n')
}

fn pacman_query_check_requested() -> bool {
    has_query_short('k')
}

fn pacman_query_mode(cli: &Cli) -> kam::cmds::installed::PacmanQueryMode {
    use kam::cmds::installed::PacmanQueryMode;

    if pacman_query_upgrades_requested() {
        PacmanQueryMode::Upgrades
    } else if cli.foreign_flag || pacman_query_foreign_requested() {
        PacmanQueryMode::Foreign
    } else if cli.native_flag || pacman_query_native_requested() {
        PacmanQueryMode::Native
    } else if cli.check_flag || pacman_query_check_requested() {
        PacmanQueryMode::Check
    } else if cli.info_flag || pacman_query_info_requested() {
        PacmanQueryMode::Info
    } else if cli.search_flag || pacman_query_search_requested() {
        PacmanQueryMode::Search
    } else {
        PacmanQueryMode::List
    }
}

fn repo_quiet_requested() -> bool {
    has_sync_short('q') || std::env::args().any(|arg| arg == "-q" || arg == "--quiet")
}

fn query_quiet_requested() -> bool {
    has_query_short('q') || std::env::args().any(|arg| arg == "-q" || arg == "--quiet")
}

fn repo_assume_yes(parsed_assume_yes: bool) -> bool {
    parsed_assume_yes
        && std::env::args().any(|arg| {
            arg == "--yes"
                || arg == "-y"
                || (arg.starts_with('-')
                    && !arg.starts_with("--")
                    && !arg.contains('S')
                    && arg.contains('y'))
        })
}

fn has_sync_short(flag: char) -> bool {
    std::env::args().skip(1).any(|arg| {
        if arg.starts_with("--") || !arg.starts_with('-') {
            return false;
        }
        let chars: Vec<char> = arg.chars().skip(1).collect();
        chars.contains(&'S') && chars.contains(&flag)
    })
}

fn has_query_short(flag: char) -> bool {
    std::env::args().skip(1).any(|arg| {
        if arg.starts_with("--") || !arg.starts_with('-') {
            return false;
        }
        let chars: Vec<char> = arg.chars().skip(1).collect();
        chars.contains(&'Q') && chars.contains(&flag)
    })
}

fn has_local_install_short(flag: char) -> bool {
    std::env::args().skip(1).any(|arg| {
        if arg.starts_with("--") || !arg.starts_with('-') {
            return false;
        }
        let chars: Vec<char> = arg.chars().skip(1).collect();
        chars.contains(&'U') && chars.contains(&flag)
    })
}

fn has_remove_short(flag: char) -> bool {
    std::env::args().skip(1).any(|arg| {
        if arg.starts_with("--") || !arg.starts_with('-') {
            return false;
        }
        let chars: Vec<char> = arg.chars().skip(1).collect();
        chars.contains(&'R') && chars.contains(&flag)
    })
}

fn repo_targets(raw_targets: &[String]) -> Vec<String> {
    let mut targets = Vec::new();
    let mut skip_next = false;
    for target in raw_targets {
        if skip_next {
            skip_next = false;
            continue;
        }
        match target.as_str() {
            "-y" | "--yes" | "-q" | "--quiet" | "-u" | "--update" | "-v" | "--verbose"
            | "--adb" | "--dry-run" => {}
            "--modules-url" | "--device" | "--manager" => skip_next = true,
            _ if target.starts_with("--modules-url=") => {}
            _ if target.starts_with("--device=") => {}
            _ if target.starts_with("--manager=") => {}
            _ => targets.push(target.clone()),
        }
    }
    targets
}

fn target_flag_present(raw_targets: &[String], name: &str) -> bool {
    raw_targets.iter().any(|target| target == name)
}

fn target_option_value(raw_targets: &[String], name: &str) -> Option<String> {
    for (idx, target) in raw_targets.iter().enumerate() {
        if target == name {
            return raw_targets
                .get(idx + 1)
                .filter(|value| !value.starts_with('-'))
                .cloned();
        }
        if let Some(value) = target.strip_prefix(&format!("{name}=")) {
            return Some(value.to_string());
        }
    }
    None
}
