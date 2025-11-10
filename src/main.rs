

use clap::{Parser, Subcommand};
use dotenvy::dotenv;

#[derive(Parser)]
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
        Commands::Init(args) => kam::cmds::init::run(args),
        Commands::Cache(args) => kam::cmds::cache::run(args),
        Commands::Sync(args) => kam::cmds::sync::run(args),
        Commands::Build(args) => kam::cmds::build::run(args),
    }
}
