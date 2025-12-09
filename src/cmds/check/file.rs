use crate::errors::KamError;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Serialize, Debug)]
pub struct FileResult {
    pub path: String,
    pub kind: String,
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub fixed: bool,
}

pub fn check_file(path: &Path, kind: &str, do_fix: bool) -> Result<FileResult, KamError> {
    let s = fs::read_to_string(path)?;
    let mut fr = FileResult {
        path: path.to_string_lossy().to_string(),
        kind: kind.to_string(),
        valid: true,
        errors: Vec::new(),
        warnings: Vec::new(),
        fixed: false,
    };

    match kind {
        "json" => {
            match serde_json::from_str::<serde_json::Value>(&s) {
                Ok(v) => {
                    // If fix, reformat
                    if do_fix {
                        let pretty = serde_json::to_string_pretty(&v).unwrap_or_default();
                        if pretty != s {
                            fs::OpenOptions::new()
                                .write(true)
                                .truncate(true)
                                .open(path)?
                                .write_all(pretty.as_bytes())?;
                            fr.fixed = true;
                        }
                    }
                }
                Err(e) => {
                    fr.valid = false;
                    fr.errors.push(format!("JSON parse error: {}", e));
                }
            }
        }
        "yaml" => match serde_yaml::from_str::<serde_yaml::Value>(&s) {
            Ok(v) => {
                if do_fix {
                    let pretty = serde_yaml::to_string(&v).unwrap_or_default();
                    if pretty != s {
                        fs::OpenOptions::new()
                            .write(true)
                            .truncate(true)
                            .open(path)?
                            .write_all(pretty.as_bytes())?;
                        fr.fixed = true;
                    }
                }
            }
            Err(e) => {
                fr.valid = false;
                fr.errors.push(format!("YAML parse error: {}", e));
            }
        },
        "toml" => {
            match toml::from_str::<toml::Value>(&s) {
                Ok(v) => {
                    if do_fix {
                        let pretty = toml::to_string_pretty(&v).unwrap_or_default();
                        if pretty != s {
                            fs::OpenOptions::new()
                                .write(true)
                                .truncate(true)
                                .open(path)?
                                .write_all(pretty.as_bytes())?;
                            fr.fixed = true;
                        }
                    }
                }
                Err(e) => {
                    fr.valid = false;
                    fr.errors.push(format!("TOML parse error: {}", e));
                }
            }
        },
        "sh" => {
            // Delegated to check_sh in sh.rs
            match super::sh::check_sh(path, do_fix) {
                Ok(p) => return Ok(p),
                Err(e) => {
                    fr.warnings.push(format!("sh check failed: {}", e));
                }
            }
        }
        _ => {}
    }

    Ok(fr)
}
