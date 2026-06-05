use clap::{Args, Subcommand};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::cmds::export::{ExportArgs, ExportFormat};
use crate::cmds::tmpl::{TmplArgs, TmplCommands};
use crate::cmds::workflow::{WorkflowArgs, WorkflowCommand};
use crate::cmds::{base_manifest, base_manifest::BaseSyncOptions};
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

const HOOKS_BASE_BLOCK: &str = r#"
[[base]]
name = "hooks"
path = ".kam/bases/hooks"
url = "https://github.com/MemDeco-WG/KamHooks.git"
branch = "main"
kind = "submodule"
include = [
  "pre-build/0500.CHECK_NDK.sh",
  "pre-build/1000.SYNC_MODULE_FILES.sh",
  "pre-build/2000.BUILD_WEBUI.sh",
  "pre-build/3000.BUILD_CRATES.sh",
  "pre-build/4000.XTASK.sh",
  "post-build/7000.SANITIZE_MODULE_ZIP.sh",
  "post-build/8000.SIGN_IF_ENABLE.sh",
  "post-build/8900.UPDATE_CHANGE_LOG.sh",
  "post-build/9000.UPLOAD_IF_ENABLED.sh",
  "post-build/9800.INTERACTIVE_INSTALL.sh",
  "post-build/9900.CLEAN_UP.sh",
]
"#;

const WORKFLOWS_BASE_BLOCK: &str = r#"
[[base]]
name = "workflows"
path = ".kam/bases/workflows"
url = "https://github.com/MemDeco-WG/KamModuleX.git"
branch = "main"
kind = "submodule"
subdir = ".github/workflows"
overlay = ".github/workflows"
include = ["README-CN.md", "README.md", "exec.yml", "init.yml"]
"#;

const OFFICIAL_HOOK_FILES: &[&str] = &[
    "hooks/pre-build/0000.EXAMPLE.sh",
    "hooks/pre-build/0100.INIT.sh",
    "hooks/pre-build/0500.CHECK_NDK.sh",
    "hooks/pre-build/1000.SYNC_MODULE_FILES.sh",
    "hooks/pre-build/2000.BUILD_WEBUI.sh",
    "hooks/pre-build/3000.BUILD_CRATES.sh",
    "hooks/pre-build/4000.XTASK.sh",
    "hooks/post-build/0000.EXAMPLE.sh",
    "hooks/post-build/7000.SANITIZE_MODULE_ZIP.sh",
    "hooks/post-build/8000.SIGN_IF_ENABLE.sh",
    "hooks/post-build/8900.UPDATE_CHANGE_LOG.sh",
    "hooks/post-build/9000.UPLOAD_IF_ENABLED.sh",
    "hooks/post-build/9800.INTERACTIVE_INSTALL.sh",
    "hooks/post-build/9900.CLEAN_UP.sh",
];

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

    /// Restore or update external bases recorded in .kam/bases.toml
    Bases,

    /// Migrate an older project to the .kam base + overlay architecture
    Migrate,

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
        SyncCommand::Bases => sync_bases(args),
        SyncCommand::Migrate => sync_migrate(args),
        SyncCommand::Workflow { source_repo } => sync_workflow(args, source_repo),
        SyncCommand::Templates => sync_templates(args),
        SyncCommand::All { source_repo } => {
            sync_bases(args)?;
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

    if materialize_workflow_base(args)? {
        return Ok(());
    }

    if args.dry_run || args.check {
        Utils::info(format!(
            "Would install generated GitHub Actions workflows for source repository: {source_repo}"
        ));
        return Ok(());
    }

    crate::cmds::workflow::run(&WorkflowArgs {
        command: WorkflowCommand::Install { source_repo },
    })?;
    Utils::success("Synchronized GitHub Actions workflows.");
    Ok(())
}

fn sync_bases(args: &SyncArgs) -> Result<(), KamError> {
    base_manifest::sync_project_bases(
        Path::new("."),
        BaseSyncOptions {
            dry_run: args.dry_run,
            check: args.check,
            update_remote: args.remote,
        },
    )?;
    if args.dry_run || args.check {
        return Ok(());
    }
    Utils::success("Synchronized .kam bases.");
    Ok(())
}

fn materialize_workflow_base(args: &SyncArgs) -> Result<bool, KamError> {
    if !base_manifest::materialize_workflow_bases(Path::new("."), args.dry_run || args.check)? {
        return Ok(false);
    }

    if !(args.dry_run || args.check) {
        Utils::success("Synchronized GitHub Actions workflows from .kam base.");
    }
    Ok(true)
}

fn sync_migrate(args: &SyncArgs) -> Result<(), KamError> {
    let project_root = Path::new(".");
    let kt = KamToml::load_from_dir(project_root)?;
    migrate_bases_manifest(project_root, &kt, args)?;
    migrate_gitignore(project_root, args)?;
    if args.dry_run || args.check {
        migrate_cleanup(project_root, args)?;
        return Ok(());
    }

    sync_bases(args)?;
    materialize_workflow_base(args)?;
    migrate_cleanup(project_root, args)?;
    Utils::success("Migrated project to .kam base + overlay architecture.");
    Ok(())
}

fn migrate_bases_manifest(
    project_root: &Path,
    kt: &KamToml,
    args: &SyncArgs,
) -> Result<(), KamError> {
    let manifest_path = project_root.join(".kam").join("bases.toml");
    let mut content = if manifest_path.exists() {
        fs::read_to_string(&manifest_path).map_err(KamError::Io)?
    } else {
        String::new()
    };

    let mut appended = Vec::new();
    if should_add_kamfw_base(project_root, kt, &content) {
        appended.push(kamfw_base_block(kt));
    }
    if should_add_anykernel_base(project_root, kt, &content) {
        appended.push(anykernel_base_block());
    }
    if !content.contains("name = \"hooks\"") {
        appended.push(HOOKS_BASE_BLOCK.trim().to_string());
    }
    if !content.contains("name = \"workflows\"") {
        appended.push(WORKFLOWS_BASE_BLOCK.trim().to_string());
    }

    if appended.is_empty() {
        return Ok(());
    }

    if args.dry_run || args.check {
        Utils::info(format!(
            "Would update {} with {} base block(s).",
            manifest_path.display(),
            appended.len()
        ));
        return Ok(());
    }

    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }
    if !content.trim().is_empty() {
        content.push_str("\n\n");
    }
    content.push_str(&appended.join("\n\n"));
    content.push('\n');
    fs::write(manifest_path, content).map_err(KamError::Io)
}

fn migrate_gitignore(project_root: &Path, args: &SyncArgs) -> Result<(), KamError> {
    let gitignore_path = project_root.join(".gitignore");
    let mut content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).map_err(KamError::Io)?
    } else {
        String::new()
    };
    let additions = [
        "!.kam/",
        ".kam/*",
        "!.kam/bases.toml",
        "!.kam/bases/",
        "!.kam/bases/**",
    ];
    let missing: Vec<_> = additions
        .iter()
        .filter(|line| !content.lines().any(|existing| existing.trim() == **line))
        .copied()
        .collect();
    if missing.is_empty() {
        return Ok(());
    }

    if args.dry_run || args.check {
        Utils::info(format!(
            "Would update {} with .kam base tracking exceptions.",
            gitignore_path.display()
        ));
        return Ok(());
    }

    if !content.ends_with('\n') && !content.is_empty() {
        content.push('\n');
    }
    content
        .push_str("\n# Kam managed bases are versioned; runtime state under .kam stays ignored.\n");
    content.push_str(&missing.join("\n"));
    content.push('\n');
    fs::write(gitignore_path, content).map_err(KamError::Io)
}

fn should_add_kamfw_base(project_root: &Path, kt: &KamToml, manifest: &str) -> bool {
    if manifest.contains("name = \"kamfw\"") {
        return false;
    }
    let source_dir = source_dir(kt);
    project_root.join(&source_dir).join("lib/kamfw").exists()
}

fn should_add_anykernel_base(project_root: &Path, kt: &KamToml, manifest: &str) -> bool {
    if manifest.contains("name = \"anykernel3\"") {
        return false;
    }
    let source_dir = source_dir(kt);
    source_dir == "src/AnyKernel3" || project_root.join(&source_dir).join("anykernel.sh").exists()
}

fn source_dir(kt: &KamToml) -> String {
    kt.kam
        .build
        .as_ref()
        .and_then(|build| build.source_dir.clone())
        .unwrap_or_else(|| format!("src/{}", kt.prop.id))
}

fn kamfw_base_block(kt: &KamToml) -> String {
    format!(
        r#"[[base]]
name = "kamfw"
path = "{}/lib/kamfw"
url = "https://github.com/MemDeco-WG/kamfw.git"
branch = "main"
kind = "submodule""#,
        source_dir(kt)
    )
}

fn anykernel_base_block() -> String {
    r#"[[base]]
name = "anykernel3"
path = "src/AnyKernel3"
url = "https://github.com/osm0sis/AnyKernel3.git"
branch = "master"
kind = "submodule""#
        .to_string()
}

fn migrate_cleanup(project_root: &Path, args: &SyncArgs) -> Result<(), KamError> {
    if !args.force {
        Utils::info(
            "Keeping legacy hook copies; pass --force to remove known official hook files.",
        );
        return Ok(());
    }

    for rel in OFFICIAL_HOOK_FILES {
        let path = project_root.join(rel);
        if args.dry_run || args.check {
            if path.exists() {
                Utils::info(format!("Would remove {}", path.display()));
            }
            continue;
        }
        if path.exists() {
            fs::remove_file(path).map_err(KamError::Io)?;
        }
    }

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
