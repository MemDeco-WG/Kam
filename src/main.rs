//
// keep it simple and stupid.
// I try to keep it simple.
// 👾
// author/human : LIghtJUNction
//
use clap::Parser;
use dotenvy::dotenv;
use kam::errors::KamError;
use std::error::Error;
use colored::Colorize;

use kam::cli::{Cli, Commands};

fn print_error_chain(e: &KamError) {
    use kam::utils::Utils;
    Utils::error(&format!("{}", e));
    let mut source = e.source();
    while let Some(s) = source {
        eprintln!("  {} {}", "→".dimmed(), s.to_string().dimmed());
        source = s.source();
    }
}

fn main() {
    dotenv().ok();
    // Initialize i18n subsystem (language detection + config-based override).
    // This ensures that CLI messages use the correct language as early as possible.
    kam::i18n::init();

    let cli = Cli::parse();

    let res: Result<(), KamError> = match cli.command {
        Commands::Init(args) => kam::cmds::init::run(args),
        Commands::Build(args) => kam::cmds::build::run(args),
        Commands::Version(args) => kam::cmds::version::run(args),
        Commands::Cache(args) => kam::cmds::cache::run(args),
        Commands::Tmpl(args) => kam::cmds::tmpl::run(args),
        Commands::Validate(args) => kam::cmds::validate::run(args),
        Commands::Completions(args) => kam::cmds::completion::run(args),
        Commands::Secret(args) => kam::cmds::secret::run(args),
        Commands::Sign(args) => kam::cmds::sign::run(args),
        Commands::Verify(args) => kam::cmds::verify::run(args),
        Commands::Check(args) => kam::cmds::check::run(args),
        Commands::Export(args) => kam::cmds::export::run(args),
        Commands::Config(args) => kam::cmds::config::run(args),
        Commands::Toml(args) => kam::cmds::toml::run(args),
        Commands::About(args) => kam::cmds::about::run(args),
    };

    if let Err(e) = res {
        print_error_chain(&e);
        std::process::exit(1);
    }
}
