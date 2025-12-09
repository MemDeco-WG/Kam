use colored::Colorize;
// PathBuf only used for argument types; keep import to prevent future lint

use crate::errors::KamError;
use crate::types::kam_toml::KamToml;

use super::args::{ExportArgs, ExportFormat};
use super::builders::*;

pub fn run(args: ExportArgs) -> Result<(), KamError> {
    let cwd = std::env::current_dir().map_err(KamError::Io)?;
    let kt = KamToml::load_from_dir(&cwd)?;

    // Determine format
    let format = if let Some(f) = args.format.clone() {
        f
    } else if let Some(output) = &args.output {
        let fname = output.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let ext = output.extension().and_then(|s| s.to_str()).unwrap_or("");
        if fname.eq_ignore_ascii_case("repo.json") {
            ExportFormat::Repo
        } else if fname.eq_ignore_ascii_case("module.json") {
            ExportFormat::Json
        } else if fname.eq_ignore_ascii_case("config.json") {
            ExportFormat::Config
        } else if fname.eq_ignore_ascii_case("update.json") {
            ExportFormat::Update
        } else if ext.eq_ignore_ascii_case("prop") {
            ExportFormat::Prop
        } else if ext.eq_ignore_ascii_case("json") {
            ExportFormat::Json
        } else {
            return Err(KamError::UnsupportedFormat(format!(
                "Cannot infer format from output: {}",
                output.display()
            )));
        }
    } else {
        return Err(KamError::CommandFailed(
            "Please provide --format or an output path to infer format".to_string(),
        ));
    };

    match format {
        ExportFormat::Prop => {
            let content = build_prop(&kt);
            if let Some(path) = &args.output {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(KamError::Io)?;
                }
                std::fs::write(path, content).map_err(KamError::Io)?;
                println!("{} Exported module.prop to {}", "✓".green(), path.display());
            } else {
                println!("{}", content);
            }
        }
        ExportFormat::Json => {
            let json_val = build_module_json(&kt);
            let pretty = serde_json::to_string_pretty(&json_val).map_err(KamError::Json)?;
            if let Some(path) = &args.output {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(KamError::Io)?;
                }
                std::fs::write(path, pretty).map_err(KamError::Io)?;
                println!("{} Exported module.json to {}", "✓".green(), path.display());
            } else {
                println!("{}", pretty);
            }
        }
        ExportFormat::Repo => {
            let json_val = build_repo_json(&kt);
            let pretty = serde_json::to_string_pretty(&json_val).map_err(KamError::Json)?;
            if let Some(path) = &args.output {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(KamError::Io)?;
                }
                std::fs::write(path, pretty).map_err(KamError::Io)?;
                println!("{} Exported repo.json to {}", "✓".green(), path.display());
            } else {
                println!("{}", pretty);
            }
        }
        ExportFormat::Track => {
            let json_val = build_track_json(&kt);
            let pretty = serde_json::to_string_pretty(&json_val).map_err(KamError::Json)?;
            if let Some(path) = &args.output {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(KamError::Io)?;
                }
                std::fs::write(path, pretty).map_err(KamError::Io)?;
                println!("{} Exported track.json to {}", "✓".green(), path.display());
            } else {
                println!("{}", pretty);
            }
        }
        ExportFormat::Config => {
            let json_val = build_config_json(&kt);
            let pretty = serde_json::to_string_pretty(&json_val).map_err(KamError::Json)?;
            if let Some(path) = &args.output {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(KamError::Io)?;
                }
                std::fs::write(path, pretty).map_err(KamError::Io)?;
                println!("{} Exported config.json to {}", "✓".green(), path.display());
            } else {
                println!("{}", pretty);
            }
        }
        ExportFormat::Update => {
            let json_val = build_update_json(&kt);
            let pretty = serde_json::to_string_pretty(&json_val).map_err(KamError::Json)?;
            if let Some(path) = &args.output {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(KamError::Io)?;
                }
                std::fs::write(path, pretty).map_err(KamError::Io)?;
                println!("{} Exported update.json to {}", "✓".green(), path.display());
            } else {
                println!("{}", pretty);
            }
        }
    }

    Ok(())
}
