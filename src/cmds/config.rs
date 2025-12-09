use clap::{Args, Subcommand};
use crate::errors::KamError;
use std::path::{Path, PathBuf};
use std::fs;

#[derive(Args, Debug)]
pub struct ConfigArgs {
    /// Use the global configuration file (~/.kam/config.toml)
    #[arg(long)]
    pub global: bool,

    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Get a configuration value by key (dot-separated path)
    Get { key: String },
    /// Set a configuration value by key
    Set { key: String, value: String },
    /// Unset (remove) a configuration value by key
    Unset { key: String },
    /// List all config values in the target file
    List,
}

fn get_config_paths(global: bool) -> Result<PathBuf, KamError> {
    if global {
        let home = dirs::home_dir().ok_or_else(|| {
            KamError::CommandFailed("Cannot determine home directory for global config".to_string())
        })?;
        let dir = home.join(".kam");
        Ok(dir.join("config.toml"))
    } else {
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
        // If no kam.toml found, use current dir
        if !cwd.join("kam.toml").exists() {
            cwd = std::env::current_dir().map_err(KamError::Io)?;
        }
        let dir = cwd.join(".kam");
        Ok(dir.join("config.toml"))
    }
}

fn read_toml(path: &Path) -> Result<toml::Value, KamError> {
    if !path.exists() {
        return Ok(toml::Value::Table(Default::default()));
    }
    let s = fs::read_to_string(path).map_err(KamError::Io)?;
    let v: toml::Value = toml::from_str(&s).map_err(|e| KamError::CommandFailed(format!("Failed to parse config file: {}", e)))?;
    Ok(v)
}

fn write_toml(path: &Path, v: &toml::Value) -> Result<(), KamError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }
    let s = toml::to_string_pretty(v).map_err(|e| KamError::CommandFailed(format!("Failed to serialize config: {}", e)))?;
    fs::write(path, s).map_err(KamError::Io)?;
    Ok(())
}

fn set_value_by_path(value: &mut toml::Value, path: &str, new_value: &str) {
    let v = value;
    if !v.is_table() {
        *v = toml::Value::Table(Default::default());
    }
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = v.as_table_mut().unwrap();
    for (i, &part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if part == "versionCode" {
                if let Ok(num) = new_value.parse::<i64>() {
                    current.insert(part.to_string(), toml::Value::Integer(num));
                } else {
                    current.insert(part.to_string(), toml::Value::String(new_value.to_string()));
                }
            } else {
                current.insert(part.to_string(), toml::Value::String(new_value.to_string()));
            }
            return;
        }
        if !current.contains_key(part) {
            current.insert(part.to_string(), toml::Value::Table(Default::default()));
        }
        current = current[part].as_table_mut().unwrap();
    }
}

fn get_value_by_path(value: &toml::Value, path: &str) -> Option<toml::Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value;
    for (i, &part) in parts.iter().enumerate() {
        if let Some(tbl) = current.as_table() {
            if let Some(next) = tbl.get(part) {
                current = next;
                if i == parts.len() - 1 {
                    return Some(current.clone());
                }
                continue;
            } else {
                return None;
            }
        } else {
            return None;
        }
    }
    None
}

fn unset_value_by_path(value: &mut toml::Value, path: &str) -> bool {
    let parts: Vec<&str> = path.split('.').collect();
    if !value.is_table() {
        return false;
    }
    let mut current_tbl = value.as_table_mut().unwrap();
    for (i, &part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            return current_tbl.remove(part).is_some();
        } else {
            if let Some(next) = current_tbl.get_mut(part) {
                if next.is_table() {
                    current_tbl = next.as_table_mut().unwrap();
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }
    }
    false
}

pub fn run(args: ConfigArgs) -> Result<(), KamError> {
    let path = get_config_paths(args.global)?;

    match args.command {
        ConfigCommand::Get { key } => {
            let v = read_toml(&path)?;
            if let Some(val) = get_value_by_path(&v, &key) {
                println!("{}", val);
                Ok(())
            } else {
                Err(KamError::CommandFailed(format!("Key '{}' not found in {}", key, path.display())))
            }
        }
        ConfigCommand::Set { key, value } => {
            let mut v = read_toml(&path)?;
            set_value_by_path(&mut v, &key, &value);
            write_toml(&path, &v)?;
            println!("Set {} = {} in {}", key, value, path.display());
            Ok(())
        }
        ConfigCommand::Unset { key } => {
            let mut v = read_toml(&path)?;
            let removed = unset_value_by_path(&mut v, &key);
            if removed {
                write_toml(&path, &v)?;
                println!("Unset {} in {}", key, path.display());
                Ok(())
            } else {
                Err(KamError::CommandFailed(format!("Key '{}' not found in {}", key, path.display())))
            }
        }
        ConfigCommand::List => {
            let v = read_toml(&path)?;
            println!("{}", toml::to_string_pretty(&v).unwrap_or_default());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_set_get_unset() {
        let d = tempdir().unwrap();
        let config_path = d.path().join(".kam/config.toml");
        // ensure path
        let _ = fs::create_dir_all(config_path.parent().unwrap());

        // set
        let mut v = read_toml(&config_path).unwrap();
        set_value_by_path(&mut v, "prop.name", "Test Module");
        write_toml(&config_path, &v).unwrap();

        let r = read_toml(&config_path).unwrap();
        assert!(get_value_by_path(&r, "prop.name").is_some());

        // unset
        let mut r2 = r.clone();
        let removed = unset_value_by_path(&mut r2, "prop.name");
        assert!(removed);
    }
}
