

use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use kam::errors::KamError;

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

fn main() -> Result<(), KamError> {
    dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
        Commands::Init(args) => kam::cmds::init::run(args).map_err(|e| KamError::Other(format!("{}", e))),
        Commands::Cache(args) => kam::cmds::cache::run(args).map_err(|e| KamError::Other(format!("{}", e))),
        Commands::Sync(args) => kam::cmds::sync::run(args).map_err(|e| KamError::Other(format!("{}", e))),
        Commands::Build(args) => kam::cmds::build::run(args).map_err(|e| KamError::Other(format!("{}", e))),
    }
}
