use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use clap::Args;
use colored::*;

/// Arguments for the version command
#[derive(Args, Debug)]
pub struct VersionArgs {
    /// The new version (e.g. 1.0.1) or bump type (major, minor, patch)
    #[arg(value_name = "VERSION")]
    pub version: Option<String>,
}

pub fn run(args: VersionArgs) -> Result<(), KamError> {
    let current_dir = std::env::current_dir()?;
    let mut kam_toml = KamToml::load_from_dir(&current_dir)?;

    if let Some(v) = args.version {
        let new_version = match v.as_str() {
            "major" => bump_version(&kam_toml.prop.version, 0)?,
            "minor" => bump_version(&kam_toml.prop.version, 1)?,
            "patch" => bump_version(&kam_toml.prop.version, 2)?,
            _ => {
                // Validate custom version format
                validate_version(&v)?;
                v
            }
        };

        if new_version != kam_toml.prop.version {
            println!(
                "Bumped version: {} -> {}",
                kam_toml.prop.version.cyan(),
                new_version.green()
            );
            kam_toml.prop.version = new_version;
        } else {
            println!("Version unchanged: {}", kam_toml.prop.version.cyan());
        }

        // Always update versionCode when version is provided
        let old_code = kam_toml.prop.versionCode;
        let new_code = chrono::Utc::now().timestamp_millis();
        println!(
            "Updated versionCode: {} -> {}",
            old_code.to_string().cyan(),
            new_code.to_string().green()
        );
        kam_toml.prop.versionCode = new_code;

        kam_toml.write_to_dir(&current_dir)?;
    } else {
        println!("Current version: {}", kam_toml.prop.version.cyan());
        println!(
            "Current versionCode: {}",
            kam_toml.prop.versionCode.to_string().cyan()
        );
    }

    Ok(())
}

fn validate_version(version: &str) -> Result<(), KamError> {
    let parts: Vec<&str> = version.split('.').collect();

    if parts.is_empty() {
        return Err(KamError::InvalidConfig(
            "Version cannot be empty".to_string(),
        ));
    }

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            return Err(KamError::InvalidConfig(format!(
                "Invalid version '{}': part {} is empty",
                version,
                i + 1
            )));
        }

        // Each part should be a number (allow alphanumeric for pre-release versions like 1.0.0-beta1)
        if !part.chars().all(|c| c.is_alphanumeric() || c == '-') {
            return Err(KamError::InvalidConfig(format!(
                "Invalid version '{}': part '{}' contains invalid characters",
                version, part
            )));
        }
    }

    Ok(())
}

fn bump_version(current: &str, index: usize) -> Result<String, KamError> {
    let mut parts: Vec<u32> = current.split('.').map(|s| s.parse().unwrap_or(0)).collect();

    // Ensure we have at least 3 parts for semver
    while parts.len() < 3 {
        parts.push(0);
    }

    if index >= parts.len() {
        return Err(KamError::InvalidConfig(
            "Invalid version format for bumping".to_string(),
        ));
    }

    parts[index] += 1;
    for i in index + 1..parts.len() {
        parts[i] = 0;
    }

    Ok(parts
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join("."))
}
