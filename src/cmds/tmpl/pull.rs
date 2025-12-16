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
        // Respect KAM_HOME env var to override the Kam home directory (defaults to $HOME/.kam)
        let dir = crate::utils::kam_home_dir()?;
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

// 从URL拉取模板
// 下载ZIP文件然后导入
pub fn run_pull(url: Option<String>, _global: bool) -> Result<(), KamError> {
    // 如果没指定URL就用默认的GitHub releases地址
    let download_url = url.as_deref().unwrap_or(DEFAULT_TEMPLATES_URL);
    use crate::utils::Utils;
    Utils::executing(&format!("Downloading templates from: {}", download_url));

    // 创建HTTP客户端，设置30秒超时
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(Policy::default())
        .build()
        .map_err(|e| KamError::CommandFailed(format!("Failed to build HTTP client: {}", e)))?;

    let mut resp = client
        .get(download_url)
        .send()
        .map_err(|e| KamError::CommandFailed(format!("Failed to download template: {}", e)))?;

    // 检查HTTP状态码
    if !resp.status().is_success() {
        return Err(KamError::CommandFailed(format!(
            "Download failed: HTTP {}",
            resp.status()
        )));
    }

    // 尝试获取文件大小（用于进度条）
    let file_size = resp
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    // 如果有文件大小就显示进度条，没有就警告一下
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

    // 创建临时文件存储下载的内容
    let mut tmpf = TempFileBuilder::new()
        .suffix(".zip")
        .tempfile()
        .map_err(KamError::Io)?;
    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 8192]; // 8KB缓冲区，应该够用了

    // 分块读取并写入临时文件
    loop {
        match resp.read(&mut buf) {
            Ok(0) => break, // 读完了
            Ok(n) => {
                tmpf.write_all(&buf[..n]).map_err(KamError::Io)?;
                downloaded += n as u64;
                // 更新进度条
                if let Some(pb) = pb.as_ref() {
                    pb.set_position(downloaded);
                }
            }
            Err(e) => {
                // 下载失败
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

    // 下载完成，导入模板
    let tmp_path = tmpf.path().to_path_buf();
    Utils::executing("Importing downloaded templates...");
    import::import_template(&tmp_path, None, true)?;

    // 保存URL和下载时间到配置（方便下次update）
    set_config_value(true, "tmpl.pull.url", download_url)?;
    let now = Utc::now().to_rfc3339();
    set_config_value(true, "tmpl.pull.last_download", &now)?;

    Utils::success("Templates downloaded and imported successfully");
    Ok(())
}

// 更新模板（用上次记录的URL重新下载）
pub fn run_update(_global: bool) -> Result<(), KamError> {
    let url = read_config_value(true, "tmpl.pull.url")?;
    if let Some(v) = url {
        // 有记录的URL，就用它重新下载
        run_pull(Some(v.clone()), true)
    } else {
        // 没有记录的URL，提示用户先pull一次
        Err(KamError::CommandFailed(
            "No recorded tmpl.pull.url found in global config. Run `kam tmpl pull <url>` first."
                .to_string(),
        ))
    }
}
