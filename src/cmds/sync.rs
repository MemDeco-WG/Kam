use clap::{Args, Subcommand};
use std::path::PathBuf;

use crate::cmds::export::{ExportArgs, ExportFormat};
use crate::cmds::tmpl::{TmplArgs, TmplCommands};
use crate::cmds::workflow::{WorkflowArgs, WorkflowCommand};
use crate::errors::KamError;
use crate::utils::Utils;

#[derive(Args, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct SyncArgs {
    #[command(subcommand)]
    pub command: Option<SyncCommand>,

    /// Print planned changes without writing files
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Check whether files can be synchronized without writing
    #[arg(long, global = true)]
    pub check: bool,

    /// Allow network-backed sync targets such as templates
    #[arg(long, global = true)]
    pub remote: bool,

    /// Force overwrite where supported
    #[arg(short, long, global = true)]
    pub force: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SyncCommand {
    /// Sync generated metadata files from kam.toml
    Metadata,

    /// Sync GitHub Actions workflows
    Workflow {
        /// Module source repository address. Defaults to current git origin when available.
        #[arg(long)]
        source_repo: Option<String>,
    },

    /// Sync official template cache. Requires --remote.
    Templates,

    /// Sync all safe targets. Remote targets require --remote.
    All {
        /// Module source repository address for workflow sync.
        #[arg(long)]
        source_repo: Option<String>,
    },
}

/// Run the sync command.
///
/// # Errors
/// Returns `KamError` if metadata export fails, workflow repository detection
/// fails, workflow files cannot be written, or a remote template sync is
/// requested without allowing remote operations.
pub fn run(args: &SyncArgs) -> Result<(), KamError> {
    let command = args.command.clone().unwrap_or(SyncCommand::Metadata);
    match command {
        SyncCommand::Metadata => sync_metadata(args),
        SyncCommand::Workflow { source_repo } => sync_workflow(args, source_repo),
        SyncCommand::Templates => sync_templates(args),
        SyncCommand::All { source_repo } => {
            sync_metadata(args)?;
            sync_workflow(args, source_repo)?;
            if args.remote {
                sync_templates(args)?;
            } else {
                Utils::info("Skipping templates sync; pass --remote to update template cache.");
            }
            Ok(())
        }
    }
}

fn sync_metadata(args: &SyncArgs) -> Result<(), KamError> {
    if args.force {
        Utils::info("Forcing metadata sync; generated files may be overwritten.");
    }

    let formats = [
        ExportFormat::Prop,
        ExportFormat::Update,
        ExportFormat::Json,
        ExportFormat::Repo,
        ExportFormat::Track,
        ExportFormat::Config,
    ];

    if args.dry_run || args.check {
        Utils::info("Would sync metadata files from kam.toml.");
        for format in &formats {
            Utils::info(format!("Would export {}", default_export_name(format)));
        }
        return Ok(());
    }

    for format in formats {
        crate::cmds::export::run(&ExportArgs {
            format: Some(format),
            output: None::<PathBuf>,
        })?;
    }
    Utils::success("Synchronized metadata files from kam.toml.");
    Ok(())
}

fn sync_workflow(args: &SyncArgs, source_repo: Option<String>) -> Result<(), KamError> {
    let source_repo = source_repo.unwrap_or_else(|| ".".to_string());
    if args.force {
        Utils::info("Forcing workflow sync; generated workflow files may be overwritten.");
    }

    if args.dry_run || args.check {
        Utils::info(format!(
            "Would sync GitHub Actions workflows for source repository: {source_repo}"
        ));
        return Ok(());
    }

    crate::cmds::workflow::run(&WorkflowArgs {
        command: WorkflowCommand::Install { source_repo },
    })?;
    Utils::success("Synchronized GitHub Actions workflows.");
    Ok(())
}

fn sync_templates(args: &SyncArgs) -> Result<(), KamError> {
    if !args.remote {
        return Err(KamError::CommandFailed(
            "Template sync requires --remote because it downloads templates.".to_string(),
        ));
    }
    if args.force {
        Utils::info("Forcing template cache sync.");
    }

    if args.dry_run || args.check {
        Utils::info("Would download and import official templates.");
        return Ok(());
    }

    crate::cmds::tmpl::run(TmplArgs {
        command: TmplCommands::Pull {
            url: None,
            global: true,
            quiet: false,
        },
    })?;
    Utils::success("Synchronized template cache.");
    Ok(())
}

fn default_export_name(format: &ExportFormat) -> &'static str {
    match format {
        ExportFormat::Prop => "module.prop",
        ExportFormat::Json => "module.json",
        ExportFormat::Repo => "repo.json",
        ExportFormat::Track => "track.json",
        ExportFormat::Config => "config.json",
        ExportFormat::Update => "update.json",
    }
}
