use clap::CommandFactory;
use clap::Parser;
use kam::cli::{Cli, Commands};

#[test]
fn help_subcommand_present_in_command_definition() {
    let cmd = Cli::command();
    assert!(
        cmd.get_subcommands().any(|s| s.get_name() == "help"),
        "expected 'help' subcommand to be present"
    );
}

#[test]
fn parsing_help_as_subcommand_sets_variant() {
    let cli = Cli::parse_from(["kam", "help"]);
    match cli.command {
        Some(Commands::Help(args)) => assert!(
            args.subcommand.is_empty(),
            "expected no subcommand args for plain `help`"
        ),
        _other => panic!("expected Commands::Help, got different subcommand"),
    }
}

#[test]
fn parsing_help_with_subcommand_path_populates_args() {
    let cli = Cli::parse_from(["kam", "help", "tmpl", "import"]);
    match cli.command {
        Some(Commands::Help(args)) => {
            assert_eq!(
                args.subcommand,
                vec!["tmpl".to_string(), "import".to_string()]
            );
        }
        _other => panic!("expected Commands::Help, got different subcommand"),
    }
}
