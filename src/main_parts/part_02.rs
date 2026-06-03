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
    match Cli::from_arg_matches(matches) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error parsing arguments: {e}");
            std::process::exit(2);
        }
    }
}

fn handle_top_level_repo_flags(cli: &Cli) -> bool {
    if cli.update_index && !cli.sync_flag && !cli.search_flag {
        let base = kam::cmds::repo::effective_base_url(cli.modules_url.as_deref());
        match kam::cmds::repo::repo_sync_with_jobs(&base, true, None::<usize>, cli.quiet) {
            Ok(()) => return true,
            Err(e) => {
                print_error_chain(&e);
                std::process::exit(1);
            }
        }
    }

    if cli.sync_flag || cli.search_flag {
        let effective_targets = cli.targets.clone();

        if cli.update_index {
            let base = kam::cmds::repo::effective_base_url(cli.modules_url.as_deref());
            match kam::cmds::repo::repo_sync_with_jobs(&base, true, None::<usize>, cli.quiet) {
                Ok(()) => {}
                Err(e) => {
                    print_error_chain(&e);
                    std::process::exit(1);
                }
            }
        }

        match kam::cmds::repo::handle_pacman_style(
            cli.sync_flag,
            cli.search_flag,
            &effective_targets,
            cli.assume_yes,
            cli.modules_url.as_deref(),
            cli.quiet,
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

fn dispatch_command(cli: Cli, cmd: &clap::Command) -> Result<(), KamError> {
    match cli.command {
        Some(Commands::Init(args)) => kam::cmds::init::run(&args),
        Some(Commands::Add(args)) => kam::cmds::add::run(&args),
        Some(Commands::Dev(args)) => kam::cmds::dev::run(&args),
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
    if handle_top_level_repo_flags(&cli) {
        return;
    }

    let res = dispatch_command(cli, &cmd);
    if let Err(e) = res {
        print_error_chain(&e);
        std::process::exit(1);
    }
}
