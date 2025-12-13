use crate::cmds::config;
use crate::cmds::config::{ConfigArgs, ConfigCommand};
use crate::cmds::tmpl::import;
use crate::errors::KamError;
use chrono::Utc;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header;
use reqwest::redirect::Policy;
use std::fs;
use std::io::{Read, Write};
use std::time::Duration;
use tempfile::Builder as TempFileBuilder;

const DEFAULT_TEMPLATES_URL: &str =
    "https://github.com/MemDeco-WG/Kam/releases/latest/download/templates.zip";

fn get_project_or_global_config_path(global: bool) -> Result<std::path::PathBuf, KamError> {
    if global {
        let home = dirs::home_dir().ok_or_else(|| {
            KamError::CommandFailed("Cannot determine home directory for global config".to_string())
        })?;
        let dir = home.join(".kam");
        return Ok(dir.join("config.toml"));
    }

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
    let v: toml::Value = toml::from_str(&s)
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse config file: {}", e)))?;
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
    let args = ConfigArgs {
        global,
        local: false,
        command: ConfigCommand::Set {
            key: key.to_string(),
            value: value.to_string(),
        },
    };
    config::run(args)
}

pub fn run_pull(url: Option<String>, _global: bool) -> Result<(), KamError> {
    let download_url = url.as_deref().unwrap_or(DEFAULT_TEMPLATES_URL);
    use crate::utils::Utils;
    Utils::executing(&format!("Downloading templates from: {}", download_url));

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(Policy::default())
        .build()
        .map_err(|e| KamError::CommandFailed(format!("Failed to build HTTP client: {}", e)))?;

    let mut resp = client
        .get(download_url)
        .send()
        .map_err(|e| KamError::CommandFailed(format!("Failed to download template: {}", e)))?;

    if !resp.status().is_success() {
        return Err(KamError::CommandFailed(format!(
            "Download failed: HTTP {}",
            resp.status()
        )));
    }

    let file_size = resp
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let pb = if let Some(size) = file_size {
        let pb = ProgressBar::new(size);
        pb.set_style(ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA: {eta})",
        ).unwrap().progress_chars("#>-"));
        Some(pb)
    } else {
        Utils::warn("Could not determine file size. Progress bar will be disabled.");
        None
    };

    let mut tmpf = TempFileBuilder::new()
        .suffix(".zip")
        .tempfile()
        .map_err(KamError::Io)?;
    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 8192];

    loop {
        match resp.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                tmpf.write_all(&buf[..n]).map_err(KamError::Io)?;
                downloaded += n as u64;
                if let Some(pb) = pb.as_ref() {
                    pb.set_position(downloaded);
                }
            }
            Err(e) => {
                if let Some(pb) = pb.as_ref() {
                    pb.finish_with_message("download failed".red().to_string());
                }
                return Err(KamError::CommandFailed(format!(
                    "Failed to read response: {}",
                    e
                )));
            }
        }
    }

    if let Some(pb) = pb {
        pb.finish_with_message("download complete".green().to_string());
    }

    let tmp_path = tmpf.path().to_path_buf();
    Utils::executing("Importing downloaded templates...");
    import::import_template(&tmp_path, None, true)?;

    set_config_value(true, "tmpl.pull.url", download_url)?;
    let now = Utc::now().to_rfc3339();
    set_config_value(true, "tmpl.pull.last_download", &now)?;

    Utils::success("Templates downloaded and imported successfully");
    Ok(())
}

pub fn run_update(_global: bool) -> Result<(), KamError> {
    let url = read_config_value(true, "tmpl.pull.url")?;
    if let Some(v) = url {
        run_pull(Some(v.clone()), true)
    } else {
        Err(KamError::CommandFailed(
            "No recorded tmpl.pull.url found in global config. Run `kam tmpl pull <url>` first."
                .to_string(),
        ))
    }
}
