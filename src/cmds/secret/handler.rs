use colored::*;
use std::fs;
use std::io::{Read, Write};

use super::args::{SecretArgs, SecretCommands};
use super::file::secret_file_path;
use super::index::{load_index, save_index};
use super::utils::{global_with_backup_default, read_secret_blob};
use crate::errors::KamError;
use chrono::TimeZone;
use rpassword::prompt_password;

pub fn run(args: SecretArgs) -> Result<(), KamError> {
    match args.command {
        SecretCommands::List => {
            let idx = load_index()?;
            if idx.entries.is_empty() {
                println!("No secrets stored.");
            } else {
                println!("Stored secrets:");
                let mut names: Vec<_> = idx.entries.keys().cloned().collect();
                names.sort();
                for n in names {
                    if let Some(meta) = idx.entries.get(&n) {
                        let enc = if meta.encrypted {
                            "(encrypted)".yellow()
                        } else {
                            "(plain)".cyan()
                        };
                        let loc = match meta.storage.as_str() {
                            "keyring" => "[keyring]".dimmed(),
                            "file" => "[file]".dimmed(),
                            _ => "[unknown]".dimmed(),
                        };
                        // Format size and last_probe
                        let size_str = if meta.size > 0 {
                            format!("{}B", meta.size)
                        } else {
                            "-".to_string()
                        };
                        let last_probe = chrono::Utc
                            .timestamp_millis_opt(meta.last_probe)
                            .single()
                            .map(|t| t.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string());
                        println!(
                            "  {} {} {} {} size={} last_probe={} ",
                            "•".cyan(),
                            n,
                            enc,
                            loc,
                            size_str,
                            last_probe
                        );
                    } else {
                        println!("  {} {}", "•".cyan(), n);
                    }
                }
            }
        }
        SecretCommands::Add {
            name,
            file,
            file_path,
            value,
            force_file,
            password,
            with_backup: _,
        } => {
            let chosen_file = file.or(file_path);
            let data = if let Some(path) = chosen_file {
                fs::read(&path).map_err(KamError::Io)?
            } else if let Some(v) = value {
                v.into_bytes()
            } else {
                // Read from stdin until EOF
                let mut s = String::new();
                std::io::stdin()
                    .read_to_string(&mut s)
                    .map_err(|e| KamError::Io(e))?;
                s.into_bytes()
            };

            // Enforce encryption: always require password and store encrypted blob
            let pw = if let Some(pw) = password {
                pw
            } else {
                let p1 = prompt_password("Encryption password: ").map_err(|e| {
                    KamError::CommandFailed(format!("Failed to read password: {}", e))
                })?;
                let p2 = prompt_password("Confirm encryption password: ").map_err(|e| {
                    KamError::CommandFailed(format!("Failed to read password: {}", e))
                })?;
                if p1 != p2 {
                    return Err(KamError::CommandFailed(
                        "Passwords do not match; aborting".to_string(),
                    ));
                }
                p1
            };
            let blob = crate::cmds::secret_crypto::encrypt_with_password(&data, &pw)?;
            // Determine effective with_backup: CLI flag overrides global default
            let _default_with_backup = global_with_backup_default();
            // Always store to local file (no keyring)
            super::file::store_secret(&name, &blob, true, force_file)?;
            println!("{} Secret '{}' saved.", "✓".green(), name);
        }
        SecretCommands::Get {
            name,
            out,
            password,
        } => {
            let blob = read_secret_blob(&name)?;
            // Try to decrypt if it looks like an encrypted blob
            let plaintext = if blob.starts_with(b"KAMKEYv1") {
                let pw = if let Some(pw) = password {
                    pw
                } else {
                    prompt_password("Password: ").map_err(|e| {
                        KamError::CommandFailed(format!("Failed to read password: {}", e))
                    })?
                };
                crate::cmds::secret_crypto::decrypt_with_password(&blob, &pw)?
            } else {
                // We now require secrets to be encrypted. If the secret is plain, instruct the user to re-add.
                return Err(KamError::CommandFailed("Stored secret is not encrypted; please re-import or add using the new required password flow (kam secret add ...)".to_string()));
            };

            if let Some(path) = out {
                let mut f = fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&path)
                    .map_err(KamError::Io)?;
                f.write_all(&plaintext).map_err(KamError::Io)?;
                println!(
                    "{} Secret '{}' written to {}",
                    "✓".green(),
                    name,
                    path.display()
                );
            } else {
                // Write to stdout
                let s = String::from_utf8_lossy(&plaintext);
                println!("{}", s);
            }
        }
        SecretCommands::Remove { name } => {
            // Remove fallback file if any
            if let Ok(p) = secret_file_path(&name) {
                if p.exists() {
                    let _ = fs::remove_file(&p);
                }
            }
            let mut idx = load_index()?;
            idx.entries.remove(&name);
            save_index(&idx)?;
            println!("{} Secret '{}' removed.", "✓".green(), name);
        }
        SecretCommands::Export {
            name,
            path,
            encrypted,
        } => {
            let blob = read_secret_blob(&name)?;
            if encrypted {
                fs::write(&path, &blob).map_err(KamError::Io)?;
            } else {
                // Decrypt before exporting
                if blob.starts_with(b"KAMKEYv1") {
                    let pw = prompt_password("Password: ").map_err(|e| {
                        KamError::CommandFailed(format!("Failed to read password: {}", e))
                    })?;
                    let plaintext = crate::cmds::secret_crypto::decrypt_with_password(&blob, &pw)?;
                    fs::write(&path, &plaintext).map_err(KamError::Io)?;
                } else {
                    // Already plaintext
                    fs::write(&path, &blob).map_err(KamError::Io)?;
                }
            }
            println!(
                "{} Secret '{}' exported to {}",
                "✓".green(),
                name,
                path.display()
            );
        }
        SecretCommands::Import { path, name } => {
            let data = fs::read(&path).map_err(KamError::Io)?;
            let final_name = if let Some(n) = name {
                n
            } else {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("imported")
                    .to_string()
            };
            // If the file looks like encrypted blob (has magic header), store as-is; else encrypt before storing
            if data.starts_with(b"KAMKEYv1") {
                super::file::store_secret(&final_name, &data, true, false)?;
            } else {
                let pw = prompt_password("Encryption password for import: ").map_err(|e| {
                    KamError::CommandFailed(format!("Failed to read password: {}", e))
                })?;
                let pw2 =
                    prompt_password("Confirm encryption password for import: ").map_err(|e| {
                        KamError::CommandFailed(format!("Failed to read password: {}", e))
                    })?;
                if pw != pw2 {
                    return Err(KamError::CommandFailed(
                        "Passwords do not match; aborting import".to_string(),
                    ));
                }
                let blob = crate::cmds::secret_crypto::encrypt_with_password(&data, &pw)?;
                super::file::store_secret(&final_name, &blob, true, false)?;
            }
            println!("{} Secret '{}' imported.", "✓".green(), final_name);
        }
    }
    Ok(())
}
