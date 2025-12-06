//
// 👀
//
use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use kam::errors::KamError;
use std::error::Error;

#[derive(Parser)]
#[command(
    name = "kam",
    about = "Kam — Super fast module manager",
    long_about = "Kam is a lightweight module management tool providing dependency resolution, build, and module management.",
    version,
    // custom help template inspired by `uv` to provide grouped sections
    help_template = "{bin} — {about}\n\nUsage: {usage}\n\nCommands:\n{subcommands}\n\nOptions:\n{options}\n"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Kam project
    Init(kam::cmds::init::InitArgs),

    /// Build the module
    Build(kam::cmds::build::BuildArgs),

    /// Manage module version
    Version(kam::cmds::version::VersionArgs),

    /// Manage local cache
    Cache(kam::cmds::cache::CacheArgs),

    /// Manage templates (import/export)
    Tmpl(kam::cmds::tmpl::TmplArgs),

    /// Validate kam.toml configuration
    Validate(kam::cmds::validate::ValidateArgs),
}

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
