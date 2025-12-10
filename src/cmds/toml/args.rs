use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct TomlArgs {
    /// Operate on the project's kam.toml (default), or specify file using --file
    #[arg(long)]
    pub file: Option<String>,

    #[command(subcommand)]
    pub command: TomlCommand,
}

#[derive(Subcommand, Debug)]
pub enum TomlCommand {
    /// Get a value by dot-separated key path
    Get { key: String },
    /// Set a value by key (usage: kam toml set prop.name=value | kam toml set prop.name value)
    Set { key: String, value: Option<String> },
    /// Unset/remove a key
    Unset { key: String },
    /// Dump the full toml
    List,
}
