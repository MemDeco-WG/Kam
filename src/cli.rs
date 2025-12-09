use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "kam",
    about = "Kam — Lightweight, offline module initializer & packager",
    long_about = "Kam is a small, network-free CLI focused on module initialization (scaffolding) and packaging (build). Lightweight and offline-first.",
    version,
    help_template = "{bin} — {about}\n\nUsage: {usage}\n\nCommands:\n{subcommands}\n\nOptions:\n{options}\n"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Kam project
    Init(crate::cmds::init::InitArgs),

    /// Build the module
    Build(crate::cmds::build::BuildArgs),

    /// Manage module version
    Version(crate::cmds::version::VersionArgs),

    /// Manage local cache
    Cache(crate::cmds::cache::CacheArgs),

    /// Manage templates (import/export)
    Tmpl(crate::cmds::tmpl::TmplArgs),

    /// Validate kam.toml configuration
    Validate(crate::cmds::validate::ValidateArgs),

    /// Generate shell completion scripts
    Completions(crate::cmds::completion::CompletionArgs),

    /// Secret keyring management for signing
    Secret(crate::cmds::secret::SecretArgs),

    /// Sign an artifact (zip) using developer private key from keyring
    Sign(crate::cmds::sign::SignArgs),
    /// Verify an artifact signature or a sigstore bundle
    Verify(crate::cmds::verify::VerifyArgs),

    /// Check project JSON/YAML/Markdown files
    Check(crate::cmds::check::CheckArgs),

    /// 导出 kam.toml 为 module.prop/module.json/update.json
    Export(crate::cmds::export::ExportArgs),
}
