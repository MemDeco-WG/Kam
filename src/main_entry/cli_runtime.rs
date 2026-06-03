fn parse_cli_matches(cmd: &clap::Command) -> clap::ArgMatches {
    if debug_i18n_enabled() {
        dump_command_debug(cmd);
    }

    let args_os: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let mut parse_cmd = cmd.clone();
    let args = kam::cli::inject_double_dash_for_targets(args_os, &mut parse_cmd);
    let toks: Vec<String> = args
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    if let Some(help_pos) = toks.iter().position(|t| t == "--help" || t == "-h") {
        let subcommand_path = subcommand_path_before_help(&toks, help_pos);
        if let Some(cur) = find_subcommand(cmd, &subcommand_path) {
            print_localized_help(cur, &subcommand_path);
        } else {
            print_localized_help(cmd.clone(), &[]);
        }
        std::process::exit(0);
    }

    match cmd.clone().try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            let cleaned = dedupe_help(&e.to_string());
            match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    print!("{cleaned}");
                    if !cleaned.ends_with('\n') {
                        println!();
                    }
                    std::process::exit(0);
                }
                _ => {
                    eprintln!("{cleaned}");
                    std::process::exit(2);
                }
            }
        }
    }
}

fn cli_from_matches(matches: &clap::ArgMatches) -> Cli {
    let mut cli = match Cli::from_arg_matches(matches) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error parsing arguments: {e}");
            std::process::exit(2);
        }
    };
    hydrate_global_subcommand_args(matches, &mut cli);
    hydrate_dev_args_from_argv(&mut cli);
    cli
}

fn global_device(cli: &Cli) -> Option<String> {
    cli.device
        .as_ref()
        .filter(|value| !value.eq_ignore_ascii_case("auto"))
        .cloned()
}

fn hydrate_global_subcommand_args(matches: &clap::ArgMatches, cli: &mut Cli) {
    let Some(("dev", dev_matches)) = matches.subcommand() else {
        return;
    };
    let Some(Commands::Dev(dev)) = &mut cli.command else {
        return;
    };
    if dev.device.is_none()
        && let Some(device) = dev_matches.get_one::<String>("device")
    {
        dev.device = Some(device.clone());
    }
    if !dev.dry_run && dev_matches.get_flag("dry_run") {
        dev.dry_run = true;
    }
}

fn hydrate_dev_args_from_argv(cli: &mut Cli) {
    let Some(Commands::Dev(dev)) = &mut cli.command else {
        return;
    };
    let args: Vec<String> = std::env::args().collect();
    let Some(dev_pos) = args.iter().position(|arg| arg == "dev") else {
        return;
    };
    let dev_args = &args[dev_pos..];
    if dev.device.is_none()
        && let Some(device) = find_option_value(dev_args, "--device")
    {
        dev.device = Some(device);
    }
    if !dev.dry_run && dev_args.iter().any(|arg| arg == "--dry-run") {
        dev.dry_run = true;
    }
}

fn find_option_value(args: &[String], name: &str) -> Option<String> {
    for (idx, arg) in args.iter().enumerate() {
        if arg == name {
            return args.get(idx + 1).filter(|value| !value.starts_with('-')).cloned();
        }
        if let Some(value) = arg.strip_prefix(&format!("{name}=")) {
            return Some(value.to_string());
        }
    }
    None
}

fn handle_top_level_repo_flags(cli: &Cli) -> bool {
    let repo_refresh = cli.update_index || pacman_sync_refresh_requested();
    let repo_assume_yes = repo_assume_yes(cli.assume_yes);

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
    let query_info = cli.info_flag || pacman_query_info_requested();
    let query_search = cli.search_flag || pacman_query_search_requested();
    let query_upgrades = pacman_query_upgrades_requested();
    let device = global_device(cli).or_else(|| target_option_value(&cli.targets, "--device"));
    match kam::cmds::installed::handle_pacman_style(
        query_search,
        query_info,
        query_upgrades,
        &effective_targets,
        device.filter(|value| !value.eq_ignore_ascii_case("auto")),
        cli.modules_url.as_deref(),
        effective_quiet,
    ) {
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

fn pacman_query_info_requested() -> bool {
    has_query_short('i')
}

fn pacman_query_search_requested() -> bool {
    has_query_short('s')
}

fn pacman_query_upgrades_requested() -> bool {
    has_query_short('u')
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
                || (arg.starts_with('-') && !arg.starts_with("--") && !arg.contains('S') && arg.contains('y'))
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

fn dispatch_command(cli: Cli, cmd: &clap::Command) -> Result<(), KamError> {
    let selected_device = global_device(&cli);
    match cli.command {
        Some(Commands::Init(args)) => kam::cmds::init::run(&args),
        Some(Commands::Add(args)) => kam::cmds::add::run(&args),
        Some(Commands::Dev(args)) => kam::cmds::dev::run(&args),
        Some(Commands::Diff(args)) => kam::cmds::diff::run(&args),
        Some(Commands::Build(args)) => kam::cmds::build::run(&args),
        Some(Commands::Version(args)) => kam::cmds::version::run(args),
        Some(Commands::Cache(args)) => kam::cmds::cache::run(args),
        Some(Commands::Tmpl(args)) => kam::cmds::tmpl::run(args),
        Some(Commands::Validate(args)) => kam::cmds::validate::run(&args),
        Some(Commands::Completions(args)) => kam::cmds::completion::run(&args),
        Some(Commands::Secret(args)) => kam::cmds::secret::run(args),
        Some(Commands::Sign(args)) => kam::cmds::sign::run(&args),
        Some(Commands::Sync(args)) => kam::cmds::sync::run(&args),
        Some(Commands::Verify(args)) => kam::cmds::verify::run(&args),
        Some(Commands::Check(args)) => kam::cmds::check::run(&args),
        Some(Commands::Export(args)) => kam::cmds::export::run(&args),
        Some(Commands::Config(args)) => kam::cmds::config::run(args),
        Some(Commands::Toml(args)) => kam::cmds::toml::run(args),
        Some(Commands::Install(args)) => kam::cmds::install::run(&args),
        Some(Commands::Mcp(args)) => kam::cmds::mcp::run(&args),
        Some(Commands::Publish(args)) => kam::cmds::publish::run(&args),
        Some(Commands::Workflow(args)) => kam::cmds::workflow::run(&args),
        Some(Commands::Repo(args)) => {
            kam::cmds::repo::run_with_modules_url(args, cli.modules_url.as_deref())
        }
        Some(Commands::Installed(mut args)) => {
            if args.device.is_none() {
                args.device = selected_device;
            }
            kam::cmds::installed::run(&args)
        }
        Some(Commands::Help(args)) => {
            handle_help_command(cmd, &args.subcommand);
            Ok(())
        }
        Some(Commands::Env(args)) => kam::cmds::env::run(&args),
        Some(Commands::About(args)) => kam::cmds::about::run(args),
        _ => {
            print_localized_help(cmd.clone(), &[]);
            Ok(())
        }
    }
}

fn handle_help_command(cmd: &clap::Command, subcommand_path: &[String]) {
    if let Some(command) = find_subcommand(cmd, subcommand_path) {
        print_localized_help(command, subcommand_path);
    } else {
        let missing = subcommand_path.join(" ");
        kam::utils::Utils::error(format!("Unknown subcommand: {missing}"));
        std::process::exit(2);
    }
}

fn main() {
    dotenv().ok();
    kam::i18n::init();

    let cmd = build_localized_command();
    let matches = parse_cli_matches(&cmd);
    let cli = cli_from_matches(&matches);
    if handle_top_level_local_install_flags(&cli) {
        return;
    }
    if handle_top_level_remove_flags(&cli) {
        return;
    }
    if handle_top_level_installed_flags(&cli) {
        return;
    }
    if handle_top_level_repo_flags(&cli) {
        return;
    }

    let res = dispatch_command(cli, &cmd);
    if let Err(e) = res {
        print_error_chain(&e);
        std::process::exit(1);
    }
}
