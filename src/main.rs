//
// 👀
//
use clap::Parser;
use dotenvy::dotenv;
use kam::errors::KamError;
use std::error::Error;

use kam::cli::{Cli, Commands};

fn main() {
    dotenv().ok();
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
    };

    if let Err(e) = res {
        eprintln!("Error: {}", e);

        let mut source = e.source();
        while let Some(s) = source {
            eprintln!("  Caused by: {}", s);
            source = s.source();
        }

        std::process::exit(1);
    }
}
