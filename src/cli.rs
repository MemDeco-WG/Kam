use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "kam",
    about = "Kam — A CLI toolkit for scaffolding, building, and distributing ksu/APU/Magisk/AnyTemplate modules",
    long_about = "Kam is a CLI toolkit for scaffolding, building, packaging, and distributing Android modules and templates (ksu/APU/Magisk/AnyTemplate). It supports module initialization, packaging, template management, and repo metadata exports.",
    version,
    help_template = "{bin} — {about}\n\nUsage: {usage}\n\nCommands:\n{subcommands}\n\nOptions:\n{options}\n"
)]
pub struct Cli {
    /// Pacman-style sync (download) flag (equivalent to pacman -S)
    #[arg(short = 'S', long = "sync", action = clap::ArgAction::SetTrue)]
    pub sync: bool,

    /// Pacman-style search modifier (use with -S as '-Ss' to search or '-s' alone to search)
    #[arg(short = 's', long = "search", action = clap::ArgAction::SetTrue)]
    pub search: bool,

    /// Positional targets: module IDs or search terms (used with -S / -s)
    #[arg(value_name = "TARGETS", num_args = 0..)]
    pub targets: Vec<String>,

    /// Assume "yes" to all confirmation prompts (equivalent to -y). Use `-y` or `--yes` to skip confirmation.
    #[arg(short = 'y', long = "yes", action = clap::ArgAction::SetTrue, global = true)]
    pub assume_yes: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Kam project from templates (supports meta and kernel templates)
    Init(crate::cmds::init::InitArgs),

    /// Build and package a module into a deployable ZIP artifact
    Build(crate::cmds::build::BuildArgs),

    /// Manage module versions and bump policies
    Version(crate::cmds::version::VersionArgs),

    /// Manage module versions and bump policies
    Cache(crate::cmds::cache::CacheArgs),

    /// Manage templates: import, export, package, and list
    Tmpl(crate::cmds::tmpl::TmplArgs),

    /// Validate `kam.toml` configuration and templates
    Validate(crate::cmds::validate::ValidateArgs),

    /// Generate shell completion scripts for common shells
    Completions(crate::cmds::completion::CompletionArgs),

    /// Secret keyring management (used by sign/verify tasks)
    Secret(crate::cmds::secret::SecretArgs),

    /// Sign an artifact using a key from the keyring or a PEM file
    Sign(crate::cmds::sign::SignArgs),
    /// Verify an artifact signature (.sig) or a sigstore bundle (DSSE)
    Verify(crate::cmds::verify::VerifyArgs),

    /// Check project JSON/YAML/Markdown files (lint/format/parse)
    Check(crate::cmds::check::CheckArgs),

    /// Export `kam.toml` to `module.prop`, `module.json`, `repo.json`, `track.json`, `config.json`, `update.json`
    Export(crate::cmds::export::ExportArgs),

    /// Inspect and edit `kam.toml` using dot-path keys (get/set/unset/list)
    Toml(crate::cmds::toml::TomlArgs),

    /// Manage per-project or global kam configuration (similar to git config)
    Config(crate::cmds::config::ConfigArgs),

    /// Install a module package to a connected device (using configured root manager)
    Install(crate::cmds::install::InstallArgs),

    /// Interact with module repository (search/download)
    Repo(crate::cmds::repo::RepoArgs),

    /// Display about information for Kam and credits
    About(crate::cmds::about::AboutArgs),
}
