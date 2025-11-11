

use clap::{Parser, Subcommand, CommandFactory, FromArgMatches, ColorChoice};
use dotenvy::dotenv;
use kam::errors::KamError;

#[derive(Parser)]
#[command(
    name = "kam",
    about = "Kam — 模块管理工具",
    long_about = "Kam 是一个轻量的模块管理工具，提供依赖解析、构建与缓存管理。",
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
    // Build the underlying clap Command to enable colored output and then
    // parse into our `Cli` struct. We enable `ColorChoice::Always` so the
    // help output includes ANSI color sequences similar to `uv`.
    let mut cmd = Cli::command();
    cmd = cmd.color(ColorChoice::Always);
    let matches = cmd.get_matches();

    let cli = Cli::from_arg_matches(&matches).map_err(|e| KamError::Other(format!("{}", e)))?;

    match cli.command {
        Commands::Init(args) => kam::cmds::init::run(args).map_err(|e| KamError::Other(format!("{}", e))),
        Commands::Cache(args) => kam::cmds::cache::run(args).map_err(|e| KamError::Other(format!("{}", e))),
        Commands::Sync(args) => kam::cmds::sync::run(args).map_err(|e| KamError::Other(format!("{}", e))),
        Commands::Build(args) => kam::cmds::build::run(args).map_err(|e| KamError::Other(format!("{}", e))),
        Commands::Publish(args) => kam::cmds::publish::run(args).map_err(|e| KamError::Other(format!("{}", e))),
        Commands::Venv(args) => kam::cmds::venv::run(args).map_err(|e| KamError::Other(format!("{}", e))),
    }
}
