use crate::errors::KamError;
use colored::Colorize;
use std::fs;
use std::io::Write;
use tempfile::Builder as TempFileBuilder;
use reqwest::blocking::Client;
use std::time::Duration;
use crate::cmds::tmpl::import;
use crate::cmds::config::{ConfigArgs, ConfigCommand};
use crate::cmds::config;
use chrono::Utc;
use reqwest::redirect::Policy;

const DEFAULT_TEMPLATES_URL: &str = "https://github.com/MemDeco-WG/Kam/releases/download/latest/templates.zip";

fn get_project_or_global_config_path(global: bool) -> Result<std::path::PathBuf, KamError> {
    // Reuse logic similar to config::get_config_paths but duplicated here for read/write
    if global {
        let home = dirs::home_dir().ok_or_else(|| KamError::CommandFailed("Cannot determine home directory for global config".to_string()))?;
        let dir = home.join(".kam");
        return Ok(dir.join("config.toml"));
    }

    // find kam.toml at cwd or upwards to locate project root; fallback to current dir
    let mut cwd = std::env::current_dir().map_err(KamError::Io)?;
    loop {
        if cwd.join("kam.toml").exists() {
            break;
        }
        if !cwd.pop() {
            break;
        }
    }
    if !cwd.join("kam.toml").exists() {
        cwd = std::env::current_dir().map_err(KamError::Io)?;
    }
    let dir = cwd.join(".kam");
    Ok(dir.join("config.toml"))
}

fn read_config_value(global: bool, key: &str) -> Result<Option<String>, KamError> {
    let path = get_project_or_global_config_path(global)?;
    if !path.exists() {
        return Ok(None);
    }
    let s = fs::read_to_string(&path).map_err(KamError::Io)?;
    let v: toml::Value = toml::from_str(&s).map_err(|e| KamError::CommandFailed(format!("Failed to parse config file: {}", e)))?;
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = &v;
    for (i, p) in parts.iter().enumerate() {
        if let Some(tbl) = current.as_table() {
            if let Some(next) = tbl.get(*p) {
                current = next;
                if i == parts.len() - 1 {
                    return Ok(Some(current.to_string().trim_matches('\"').to_string()));
                }
                continue;
            } else {
                return Ok(None);
            }
        } else {
            return Ok(None);
        }
    }
    Ok(None)
}

fn set_config_value(global: bool, key: &str, value: &str) -> Result<(), KamError> {
    // Use the existing config run helper to set the value for us, to avoid duplicating write logic
    let args = ConfigArgs {
        global,
        local: false,
        command: ConfigCommand::Set { key: key.to_string(), value: value.to_string() },
    };
    config::run(args)
}

pub fn run_pull(url: Option<String>, _global: bool) -> Result<(), KamError> {
    let download_url = url.as_deref().unwrap_or(DEFAULT_TEMPLATES_URL);

    println!("{} Downloading templates from: {}", "→".cyan(), download_url);

    // Create HTTP client with a reasonable timeout
    let client = Client::builder().timeout(Duration::from_secs(120)).redirect(Policy::default()).build().map_err(|e| KamError::CommandFailed(format!("Failed to build HTTP client: {}", e)))?;
    let resp = client.get(download_url).send().map_err(|e| KamError::CommandFailed(format!("Failed to download template: {}", e)))?;
    if !resp.status().is_success() {
        return Err(KamError::CommandFailed(format!("Download failed: HTTP {}", resp.status())));
    }

    let mut tmpf = TempFileBuilder::new().suffix(".zip").tempfile().map_err(KamError::Io)?;
    let bytes = resp.bytes().map_err(|e| KamError::CommandFailed(format!("Failed to read response body: {}", e)))?;
    tmpf.write_all(&bytes).map_err(KamError::Io)?;
    // Need to persist file path for import (named tempfile is kept until drop)
    let tmp_path = tmpf.path().to_path_buf();

    // Perform import using force=true (behaviour like -f)
    println!("{} Importing downloaded templates...", "→".cyan());
    import::import_template(&tmp_path, None, true)?;

    // On success, record URL and last download plan in global config
    set_config_value(true, "tmpl.pull.url", download_url)?;
    let now = Utc::now().to_rfc3339();
    set_config_value(true, "tmpl.pull.last_download", &now)?;

    println!("{} Templates downloaded and imported successfully", "✓".green());
    Ok(())
}

pub fn run_update(_global: bool) -> Result<(), KamError> {
    // Always read recorded URL from global config
    let url = read_config_value(true, "tmpl.pull.url")?;
    if let Some(v) = url {
        run_pull(Some(v.clone()), true)
    } else {
        Err(KamError::CommandFailed("No recorded tmpl.pull.url found in global config. Run `kam tmpl pull <url>` first.".to_string()))
    }
}
