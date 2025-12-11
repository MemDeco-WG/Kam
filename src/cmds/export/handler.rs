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
        // If output explicitly asks for stdout, default to Prop
        if output.to_str().map(|s| s == "-").unwrap_or(false) {
            ExportFormat::Prop
        } else {
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
        }
    } else {
        // Default to Prop (module.prop) if no format or output specified
        ExportFormat::Prop
    };

    // Determine output path. If args.output provided and equals "-", print to stdout.
    let output_path: Option<std::path::PathBuf> = if let Some(p) = &args.output {
        match p.to_str() {
            Some("-") => None,
            _ => Some(p.clone()),
        }
    } else {
        // Default filename based on format
        let filename = match format {
            ExportFormat::Prop => "module.prop",
            ExportFormat::Json => "module.json",
            ExportFormat::Repo => "repo.json",
            ExportFormat::Track => "track.json",
            ExportFormat::Config => "config.json",
            ExportFormat::Update => "update.json",
        };
        Some(cwd.join(filename))
    };

    match format {
        ExportFormat::Prop => {
            let content = build_prop(&kt);
            if let Some(path) = &output_path {
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
            if let Some(path) = &output_path {
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
            if let Some(path) = &output_path {
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
            if let Some(path) = &output_path {
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
            if let Some(path) = &output_path {
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
            if let Some(path) = &output_path {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use tempfile::tempdir;

    fn write_sample_kam_toml(dir: &std::path::Path) -> KamToml {
        let kt = KamToml::new_with_current_timestamp(
            "example.mod".to_string(),
            "Example".to_string(),
            "1.2.3".to_string(),
            "Tester".to_string(),
            "desc".to_string(),
            None,
            None,
        );
        kt.write_to_dir(dir).unwrap();
        kt
    }

    #[test]
    #[serial]
    fn default_export_writes_module_prop() {
        let tmp = tempdir().unwrap();
        let path = tmp.path();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(path).unwrap();
        let _kt = write_sample_kam_toml(path);

        let args = ExportArgs {
            format: None,
            output: None,
        };
        run(args).unwrap();
        std::env::set_current_dir(orig).unwrap();
        assert!(path.join("module.prop").exists());
        let contents = fs::read_to_string(path.join("module.prop")).unwrap();
        assert!(contents.contains("id=") || contents.contains("name="));
    }

    #[test]
    #[serial]
    fn json_export_writes_module_json_by_default() {
        let tmp = tempdir().unwrap();
        let path = tmp.path();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(path).unwrap();
        let _kt = write_sample_kam_toml(path);

        let args = ExportArgs {
            format: Some(ExportFormat::Json),
            output: None,
        };
        run(args).unwrap();
        std::env::set_current_dir(orig).unwrap();
        assert!(path.join("module.json").exists());
    }

    #[test]
    #[serial]
    fn stdout_dash_export_does_not_write_file() {
        let tmp = tempdir().unwrap();
        let path = tmp.path();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(path).unwrap();
        let _kt = write_sample_kam_toml(path);

        let args = ExportArgs {
            format: None,
            output: Some(std::path::PathBuf::from("-")),
        };
        run(args).unwrap();
        std::env::set_current_dir(orig).unwrap();
        assert!(!path.join("module.prop").exists());
    }
}
