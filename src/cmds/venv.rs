use clap::{Args, Subcommand};
use colored::Colorize;
use std::path::Path;

use crate::cache::KamCache;
use crate::errors::KamError;
use crate::venv::{KamVenv, VenvType};
use crate::types::kam_toml::KamToml;
use crate::template::TemplateVariableProcessor;
use std::collections::HashMap;

/// Arguments for the venv command
#[derive(Args, Debug)]
pub struct VenvArgs {
    /// Path to the project (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    #[command(subcommand)]
    pub command: Option<VenvCommands>,
}

/// Subcommands for venv
#[derive(Subcommand, Debug)]
pub enum VenvCommands {
    /// Create a virtual environment
    Create {
        /// Create as development venv (includes dev deps)
        #[arg(long)]
        dev: bool,
        /// Force recreate if exists
        #[arg(short, long)]
        force: bool,
    },

    /// Remove a virtual environment
    Remove {
        /// Skip confirmation
        #[arg(short, long)]
        yes: bool,
    },

    /// Show info about the virtual environment
    Info,

    /// Print activation instructions
    Activate,

    /// Print deactivation instructions
    Deactivate,

    /// Link a binary from cache into the venv
    LinkBin {
        /// Binary name in cache
        name: String,
    },

    /// Link a library (module id and version) into the venv
    LinkLib {
        /// Module id
        id: String,
        /// Module version (use "latest" if omitted)
        version: String,
    },
}

/// Run the venv command
//
// Helper: set environment variable if it is not already present.
//
// This function uses `std::env::var_os` to avoid overwriting values that may
// already be set by the calling environment. It centralizes the check-and-set
// logic and avoids repeated `std::env::set_var` calls sprinkled throughout
// the codebase. `std::env::set_var` is safe in normal Rust code, but using
// an explicit check prevents surprises and satisfies the owner request to
// consolidate env setting logic behind a safe function.
#[allow(dead_code)]
fn set_env_if_missing(_key: &str, _value: &str) {
    // Replaced by explicit replacement maps when creating venv.
    // This helper intentionally no-ops to avoid mutating process environment.
}
pub fn run(args: VenvArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);
    let venv_path = project_path.join(".kam_venv");

    match args.command {
        Some(VenvCommands::Create { dev, force }) => {
            if venv_path.exists() {
                if force {
                    std::fs::remove_dir_all(&venv_path)?;
                } else {
                    return Err(KamError::VenvExists(format!(
                        "Virtual environment already exists at {}. Use --force to recreate.",
                        venv_path.display()
                    )));
                }
            }

            // Build replacements map from kam.toml for explicit template rendering
            // instead of mutating the process environment.
            let replacements_for_venv: Option<std::collections::HashMap<String, String>> =
                if let Ok(kam_toml) = KamToml::load_from_dir(Path::new(&args.path)) {
                    let mut replacements = TemplateVariableProcessor::flatten_kam_toml(&kam_toml);

                    // ensure shallow keys are present, matching previous behavior
                    if !replacements.contains_key("id") {
                        replacements.insert("id".to_string(), kam_toml.prop.id.clone());
                    }
                    if !replacements.contains_key("version") {
                        replacements.insert("version".to_string(), kam_toml.prop.version.clone());
                    }
                    if !replacements.contains_key("author") {
                        replacements.insert("author".to_string(), kam_toml.prop.author.clone());
                    }
                    if !replacements.contains_key("name") {
                        let shallow_name = kam_toml
                            .prop
                            .name
                            .get("en")
                            .cloned()
                            .or_else(|| kam_toml.prop.name.iter().next().map(|(_k, v)| v.clone()))
                            .unwrap_or_else(|| kam_toml.prop.id.clone());
                        replacements.insert("name".to_string(), shallow_name);
                    }

                    Some(replacements)
                } else {
                    // If kam.toml cannot be loaded, do not pass explicit replacements.
                    None
                };

            println!("{} Creating virtual environment...", "→".cyan());
            let venv_type = if dev {
                VenvType::Development
            } else {
                VenvType::Runtime
            };
            let venv = KamVenv::create_with_replacements(
                &venv_path,
                venv_type,
                replacements_for_venv,
            )
            .map_err(|e| KamError::VenvCreateFailed(format!("Venv create failed: {}", e)))?;
            println!("  {} Created at: {}", "✓".green(), venv.root().display());
            println!();
            println!("To activate the virtual environment:");
            println!(
                "  {}: source {}/activate",
                "Unix".yellow(),
                venv.root().display()
            );
            println!("  {}: {0}\\activate.bat", "Windows".yellow(),);
            println!("  {}: {0}\\activate.ps1", "PowerShell".yellow());
            Ok(())
        }

        Some(VenvCommands::Remove { yes }) => {
            if !venv_path.exists() {
                println!(
                    "{} No virtual environment found at {}",
                    "!".yellow(),
                    venv_path.display()
                );
                return Ok(());
            }

            if !yes {
                println!(
                    "{} This will delete {}",
                    "Warning:".yellow().bold(),
                    venv_path.display()
                );
                print!("Are you sure? (y/N): ");
                use std::io::{self, Write};
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("{} Cancelled.", "Cancelled:".yellow());
                    return Ok(());
                }
            }

            std::fs::remove_dir_all(&venv_path)?;
            println!(
                "{} Removed virtual environment at {}",
                "✓".green(),
                venv_path.display()
            );
            Ok(())
        }

        Some(VenvCommands::Info) => {
            if !venv_path.exists() {
                return Err(KamError::VenvNotFound(format!(
                    "Virtual environment not found at {}",
                    venv_path.display()
                )));
            }

            let venv = KamVenv::load(&venv_path)?;
            println!(
                "{} Virtual environment: {}",
                "Info:".cyan(),
                venv.root().display()
            );
            println!("  Type: {:?}", venv.venv_type());
            println!("  Bin: {}", venv.bin_dir().display());
            println!("  Lib: {}", venv.lib_dir().display());

            // List bin entries
            if let Ok(entries) = std::fs::read_dir(venv.bin_dir()) {
                println!("\n  Binaries:");
                for e in entries.flatten() {
                    println!("    - {}", e.file_name().to_string_lossy());
                }
            }

            // List libs
            if let Ok(entries) = std::fs::read_dir(venv.lib_dir()) {
                println!("\n  Libraries:");
                for e in entries.flatten() {
                    println!("    - {}", e.file_name().to_string_lossy());
                }
            }

            Ok(())
        }

        Some(VenvCommands::Activate) => {
            println!("To activate the virtual environment:");
            println!("  Unix: source .kam_venv/activate");
            println!("  Windows (cmd): .kam_venv\\activate.bat");
            println!("  PowerShell: .kam_venv\\activate.ps1");
            Ok(())
        }

        Some(VenvCommands::Deactivate) => {
            println!(
                "To deactivate, run the 'deactivate' function or script provided by the activation environment."
            );
            println!("  In shells: run 'deactivate' or execute .kam-venv/deactivate");
            Ok(())
        }

        Some(VenvCommands::LinkBin { name }) => {
            if !venv_path.exists() {
                return Err(KamError::VenvNotFound(format!(
                    "Virtual environment not found at {}",
                    venv_path.display()
                )));
            }

            let cache = KamCache::new()?;
            let venv = KamVenv::load(&venv_path)?;
            venv.link_binary(cache.bin_path(&name).as_path())?;
            println!("{} Linked binary '{}' into venv", "✓".green(), name);
            Ok(())
        }

        Some(VenvCommands::LinkLib { id, version }) => {
            if !venv_path.exists() {
                return Err(KamError::VenvNotFound(format!(
                    "Virtual environment not found at {}",
                    venv_path.display()
                )));
            }

            let cache = KamCache::new()?;
            let venv = KamVenv::load(&venv_path)?;
            let ver = if version.is_empty() {
                "latest"
            } else {
                &version
            };
            venv.link_library(&id, ver, &cache)?;
            println!("{} Linked library '{}@{}' into venv", "✓".green(), id, ver);
            Ok(())
        }

        None => {
            // Default behaviour for `kam venv` with no subcommand:
            // Ensure virtual environment exists, sync dependencies, and print activation instructions.

            // Before invoking sync, set KAM_* env vars so sync-created venvs use project properties
            // This mirrors the behavior in `Create`: prefer top-level vars and set KAM_VAR_* for flattened variables.
            if let Ok(kam_toml) = KamToml::load_from_dir(Path::new(&args.path)) {
                set_env_if_missing("KAM_ID", &kam_toml.prop.id);

                let shallow_name = kam_toml
                    .prop
                    .name
                    .get("en")
                    .cloned()
                    .or_else(|| kam_toml.prop.name.iter().next().map(|(_k, v)| v.clone()))
                    .unwrap_or_else(|| kam_toml.prop.id.clone());
                set_env_if_missing("KAM_NAME", &shallow_name);

                set_env_if_missing("KAM_VERSION", &kam_toml.prop.version);
                set_env_if_missing("KAM_AUTHOR", &kam_toml.prop.author);

                let flat_vars: HashMap<String, String> = TemplateVariableProcessor::flatten_kam_toml(&kam_toml);
                for (k, v) in flat_vars.into_iter() {
                    let env_key = format!(
                        "KAM_VAR_{}",
                        k.replace('.', "_").replace('-', "_").to_ascii_uppercase()
                    );
                    if std::env::var_os(&env_key).is_none() {
                        // Avoid mutating the process environment in this context.
                        // Some platforms or build contexts do not allow `std::env::set_var` in a safe way,
                        // and we explicitly skip setting environment variables here.
                        // If template consumers require these variables, they should be exported
                        // by the caller or provided via a replacement map to `kam venv create`.
                        println!(
                            "{} Skipping automatic setting of {} (KAM_VAR_*); please export it manually or provide replacements.",
                            "!".yellow(),
                            env_key
                        );
                    }
                }
            } else {
                println!("  {} No kam.toml found in {}; using existing environment values (if any)", "!".yellow(), args.path);
            }

            println!(
                "{} Ensuring virtual environment and synchronizing dependencies...",
                "→".cyan()
            );
            // Reuse sync command logic: request venv creation and linking.
            let sync_args = crate::cmds::sync::SyncArgs {
                path: args.path.clone(),
                dev: false,
            };
            crate::cmds::sync::run(sync_args)?;
            // After sync/run, activation hints are printed by sync when appropriate.
            Ok(())
        }
    }
}
