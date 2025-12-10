use crate::errors::KamError;
use colored::*;
use std::fs;
use std::path::PathBuf;

use super::args::TomlArgs;

fn find_kam_toml_path(file: &Option<String>) -> Result<PathBuf, KamError> {
    if let Some(path) = file {
        return Ok(PathBuf::from(path));
    }
    let mut cwd = std::env::current_dir().map_err(KamError::Io)?;
    loop {
        let candidate = cwd.join("kam.toml");
        if candidate.exists() {
            return Ok(candidate);
        }
        if !cwd.pop() {
            break;
        }
    }
    // fallback to cwd/kam.toml even if not found
    Ok(std::env::current_dir().map_err(KamError::Io)?.join("kam.toml"))
}

fn read_toml(path: &PathBuf) -> Result<toml::Value, KamError> {
    if !path.exists() {
        return Err(KamError::InvalidDirectory(format!(
            "Toml file not found: {}",
            path.display()
        )));
    }
    let s = fs::read_to_string(path).map_err(KamError::Io)?;
    let v: toml::Value = toml::from_str(&s)
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse toml: {}", e)))?;
    Ok(v)
}

fn write_toml(path: &PathBuf, v: &toml::Value) -> Result<(), KamError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }
    let s = toml::to_string_pretty(v)
        .map_err(|e| KamError::CommandFailed(format!("Failed to serialize toml: {}", e)))?;
    fs::write(path, s).map_err(KamError::Io)?;
    Ok(())
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

fn set_value_by_path(value: &mut toml::Value, path: &str, new_value: &str) {
    let v = value;
    if !v.is_table() {
        *v = toml::Value::Table(Default::default());
    }
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = v.as_table_mut().unwrap();
    for (i, &part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // heuristics: if string contains only digits -> integer
            if part == "versionCode" {
                if let Ok(num) = new_value.parse::<i64>() {
                    current.insert(part.to_string(), toml::Value::Integer(num));
                } else {
                    current.insert(part.to_string(), toml::Value::String(new_value.to_string()));
                }
            } else if let Ok(num) = new_value.parse::<i64>() {
                current.insert(part.to_string(), toml::Value::Integer(num));
            } else if new_value == "true" || new_value == "false" {
                current.insert(part.to_string(), toml::Value::Boolean(new_value == "true"));
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

pub fn run(args: TomlArgs) -> Result<(), KamError> {
    let path = find_kam_toml_path(&args.file)?;

    match args.command {
        crate::cmds::toml::args::TomlCommand::Get { key } => {
            let v = read_toml(&path)?;
            if let Some(val) = get_value_by_path(&v, &key) {
                println!("{}", val);
                Ok(())
            } else {
                Err(KamError::CommandFailed(format!(
                    "Key '{}' not found in {}",
                    key,
                    path.display()
                )))
            }
        }
        crate::cmds::toml::args::TomlCommand::Set { key, value } => {
            // support key=value single argument if value is None
            let (key, new_value) = if let Some(v) = value {
                (key, v)
            } else if key.contains('=') {
                let parts: Vec<&str> = key.splitn(2, '=').collect();
                (parts[0].to_string(), parts[1].to_string())
            } else {
                return Err(KamError::InvalidFilename("No value provided".to_string()));
            };
            let mut v = read_toml(&path).unwrap_or(toml::Value::Table(Default::default()));
            set_value_by_path(&mut v, &key, &new_value);
            write_toml(&path, &v)?;
            println!("{} Set {} = {} in {}", "✓".green(), key, new_value, path.display());
            Ok(())
        }
        crate::cmds::toml::args::TomlCommand::Unset { key } => {
            let mut v = read_toml(&path)?;
            let removed = unset_value_by_path(&mut v, &key);
            if removed {
                write_toml(&path, &v)?;
                println!("{} Unset {} in {}", "✓".green(), key, path.display());
                Ok(())
            } else {
                Err(KamError::CommandFailed(format!(
                    "Key '{}' not found in {}",
                    key,
                    path.display()
                )))
            }
        }
        crate::cmds::toml::args::TomlCommand::List => {
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
    fn toml_set_get_unset() {
        let d = tempdir().unwrap();
        let path = d.path().join("kam.toml");
        fs::write(&path, "[prop]\nid = \"test\"\nversion = \"0.1.0\"\n").unwrap();
        let mut v = read_toml(&path).unwrap();
        assert!(get_value_by_path(&v, "prop.id").is_some());

        set_value_by_path(&mut v, "prop.name", "Tester");
        write_toml(&path, &v).unwrap();
        let v2 = read_toml(&path).unwrap();
        assert!(get_value_by_path(&v2, "prop.name").is_some());

        let mut v3 = v2.clone();
        let removed = unset_value_by_path(&mut v3, "prop.name");
        assert!(removed);
    }
}
