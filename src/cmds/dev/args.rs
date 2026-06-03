use clap::{Args, Subcommand};

#[derive(Args, Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct DevArgs {
    #[command(subcommand)]
    pub command: Option<DevCommand>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long, global = true)]
    pub device: Option<String>,

    /// Watch source files and repeat dev build/sync when they change.
    #[arg(long)]
    pub watch: bool,

    /// Hot-update allowlisted files without a full module install.
    #[arg(long)]
    pub hot: bool,

    /// Build/sync WebUI assets and forward the declared WebUI port when configured.
    #[arg(long)]
    pub webui: bool,

    /// Skip dev-build hooks and only synchronize allowed files to the device.
    #[arg(long)]
    pub sync_only: bool,

    /// Build and install a full ZIP for first install or structural changes.
    #[arg(long)]
    pub install: bool,

    /// Tail declared module logs and module-related logcat output.
    #[arg(long)]
    pub logs: bool,

    /// Enable the standard MCP runtime contract during the dev session.
    #[arg(long)]
    pub mcp: bool,

    /// Forward named endpoints. Accepts mcp, webui, or mcp:webui.
    #[arg(long, value_delimiter = ':')]
    pub forward: Vec<String>,

    /// Print planned local and device writes without executing them.
    #[arg(long, global = true)]
    pub dry_run: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DevCommand {
    /// Diagnose adb, root, module path, hooks, logs, and MCP contract.
    Doctor,
}
