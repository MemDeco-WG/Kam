//
// 👀
//
use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use kam::errors::KamError;

#[derive(Parser)]
#[command(
    name = "kam",
    about = "Kam — Super fast module manager",
    long_about = "Kam is a lightweight module management tool providing dependency resolution, build, and cache management.",
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

    /// Manage the global cache
    Cache(kam::cmds::cache::CacheArgs),

    /// Synchronize dependencies
    Sync(kam::cmds::sync::SyncArgs),

    /// Build the module
    Build(kam::cmds::build::BuildArgs),

    /// Publish the module to a repository
    Publish(kam::cmds::publish::PublishArgs),

    /// Manage virtual environment
    Venv(kam::cmds::venv::VenvArgs),
}

fn main() -> Result<(), KamError> {
    dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
        Commands::Init(args) => kam::cmds::init::run(args),
        Commands::Cache(args) => kam::cmds::cache::run(args),
        Commands::Sync(args) => kam::cmds::sync::run(args),
        Commands::Build(args) => kam::cmds::build::run(args),
        Commands::Publish(args) => kam::cmds::publish::run(args),
        Commands::Venv(args) => kam::cmds::venv::run(args),
    }
}
