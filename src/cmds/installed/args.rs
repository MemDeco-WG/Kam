use clap::{Args, Subcommand};

#[derive(Args, Debug, Clone)]
pub struct InstalledArgs {
    /// Subcommands for installed module queries.
    #[command(subcommand)]
    pub command: Option<InstalledCommand>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum InstalledCommand {
    /// List installed modules from /data/adb/modules.
    List(InstalledListArgs),
    /// Search installed module metadata.
    Search(InstalledSearchArgs),
    /// Show installed module metadata.
    Info(InstalledInfoArgs),
    /// List installed modules with a newer cached repository release.
    Upgrades(InstalledUpgradesArgs),
    /// Mark installed modules for removal.
    Remove(InstalledRemoveArgs),
    /// List installed modules not present in the cached repository index.
    Foreign(InstalledOriginArgs),
    /// List installed modules present in the cached repository index.
    Native(InstalledOriginArgs),
    /// Check installed module directory and module.prop integrity.
    Check(InstalledCheckArgs),
    /// Find which installed module owns a device path.
    Owner(InstalledOwnerArgs),
    /// List files owned by installed modules.
    Files(InstalledFilesArgs),
    /// Show metadata from local module ZIP packages.
    PackageInfo(InstalledPackageInfoArgs),
    /// List files inside local module ZIP packages.
    PackageFiles(InstalledPackageInfoArgs),
}

#[derive(Args, Debug, Clone)]
pub struct InstalledListArgs {
    /// Optional query to filter module id, name, author, or description.
    #[arg(value_name = "QUERY", num_args = 0..)]
    pub query: Vec<String>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct InstalledSearchArgs {
    /// Search terms.
    #[arg(value_name = "QUERY", required = true, num_args = 1..)]
    pub query: Vec<String>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct InstalledInfoArgs {
    /// Installed module ids or names.
    #[arg(value_name = "MODULE", required = true, num_args = 1..)]
    pub modules: Vec<String>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct InstalledUpgradesArgs {
    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,

    /// Suppress details and print only module ids.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

#[derive(Args, Debug, Clone)]
pub struct InstalledRemoveArgs {
    /// Installed module ids or names to mark for removal.
    #[arg(value_name = "MODULE", required = true, num_args = 1..)]
    pub modules: Vec<String>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,

    /// Print planned removal marker writes without changing the device.
    #[arg(long)]
    pub dry_run: bool,

    /// Assume yes to confirmation prompts.
    #[arg(short = 'y', long = "yes")]
    pub assume_yes: bool,

    /// Suppress non-essential output.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

#[derive(Args, Debug, Clone)]
pub struct InstalledOriginArgs {
    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,

    /// Suppress details and print only module ids.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

#[derive(Args, Debug, Clone)]
pub struct InstalledCheckArgs {
    /// Optional installed module ids or names to check.
    #[arg(value_name = "MODULE", num_args = 0..)]
    pub modules: Vec<String>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,

    /// Suppress successful checks and print only problems.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

#[derive(Args, Debug, Clone)]
pub struct InstalledOwnerArgs {
    /// Device paths to resolve to installed modules.
    #[arg(value_name = "PATH", required = true, num_args = 1..)]
    pub paths: Vec<String>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,

    /// Suppress details and print only module ids.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

#[derive(Args, Debug, Clone)]
pub struct InstalledFilesArgs {
    /// Installed module ids or names.
    #[arg(value_name = "MODULE", required = true, num_args = 1..)]
    pub modules: Vec<String>,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long)]
    pub device: Option<String>,

    /// Suppress module id prefixes and print paths only.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

#[derive(Args, Debug, Clone)]
pub struct InstalledPackageInfoArgs {
    /// Local module ZIP packages to inspect.
    #[arg(value_name = "PACKAGE", required = true, num_args = 1..)]
    pub packages: Vec<std::path::PathBuf>,

    /// Suppress details and print only package ids.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacmanQueryRequest {
    pub mode: PacmanQueryMode,
    pub targets: Vec<String>,
    pub device: Option<String>,
    pub modules_url: Option<String>,
    pub quiet: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacmanQueryMode {
    List,
    Search,
    Info,
    Upgrades,
    Foreign,
    Native,
    Check,
    Owner,
    Files,
    Package,
    PackageFiles,
}
