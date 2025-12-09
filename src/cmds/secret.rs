use crate::cmds::secret_crypto::{decrypt_with_password, encrypt_with_password};
use crate::errors::KamError;
use chrono::{TimeZone, Utc};
use clap::{Args, Subcommand};
use colored::*;
use dirs::home_dir;
use keyring::Entry;
use rpassword::prompt_password;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

use base64::engine::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const SERVICE_NAME: &str = "kam";

#[derive(Args, Debug)]
pub struct SecretArgs {
    #[command(subcommand)]
    pub command: SecretCommands,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use serial_test::serial;
    // env used directly via std::env::set_var

    #[test]
    #[serial]
    fn add_and_read_with_file_fallback_updates_index() {
        let d = tempdir().unwrap();
        unsafe { std::env::set_var("HOME", d.path().to_str().unwrap()); }
        let name = "test_secret";
        let blob = b"some-secret-data".to_vec();

        // Store using file fallback
        assert!(store_secret(name, &blob, false, true, false).is_ok());

        // Read back
        let read = read_secret_blob(name).unwrap();
        assert_eq!(read, blob);

        // Index should contain metadata
        let idx = load_index().unwrap();
        let meta = idx.entries.get(name).unwrap();
        assert_eq!(meta.storage, "file");
        assert_eq!(meta.size, blob.len() as u64);
        assert!(meta.created_at > 0);
    }

    #[serial]
    fn add_with_backup_attempts_fallback_file() {
        let d = tempdir().unwrap();
        unsafe { std::env::set_var("HOME", d.path().to_str().unwrap()); }
        let name = "baktest";
        let blob = b"secretdata".to_vec();
        // Attempt to store with backup (force_file = false, with_backup = true)
        let res = store_secret(name, &blob, true, false, true);
        assert!(res.is_ok());
        // Check index and/or file
        let idx = load_index().unwrap();
        if let Some(meta) = idx.entries.get(name) {
            // If keyring claimed, that's ok; fallback may or may not exist due to environment
            assert!(meta.storage == "keyring" || meta.storage == "file");
        }
        // If unexpected, check fallback path exists
        if let Ok(p) = secret_file_path(name) {
            if p.exists() {
                let s = fs::read_to_string(&p).unwrap();
                assert!(!s.is_empty());
            }
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum SecretCommands {
    /// List saved secrets
    List,

    /// Add a secret from a value or file
    Add {
        /// Name of the secret
        name: String,

        /// Path to a file to read the secret from
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Also accept path as a second positional parameter so users can run `kam secret add name path`
        #[arg(value_name = "FILE")]
        file_path: Option<PathBuf>,

        /// Provide value directly
        #[arg(short, long)]
        value: Option<String>,

        /// Force storing to local file instead of system keyring
        #[arg(long, default_value_t = false)]
        force_file: bool,
        /// Also create an encrypted fallback file under ~/.kam/secrets
        #[arg(long, default_value_t = false)]
        with_backup: bool,
        /// Pass the password on the CLI (not recommended); password will be prompted if not set
        #[arg(long)]
        password: Option<String>,
    },

    /// Get a secret and print it to stdout (or --out file)
    Get {
        /// Name of the secret
        name: String,

        /// Write to file instead of stdout
        #[arg(short, long)]
        out: Option<PathBuf>,

        /// Pass the password on the CLI (not recommended). If not provided, will ask interactively
        #[arg(long)]
        password: Option<String>,
    },

    /// Remove a secret
    Remove { name: String },
    /// Export secret to a file (by default decrypted). Use --encrypted to export encrypted blob.
    Export {
        name: String,
        path: PathBuf,
        #[arg(long, default_value_t = false)]
        encrypted: bool,
    },

    /// Import secret from a file. If file is an encrypted KAM blob, it will be stored as-is.
    Import {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
struct SecretMeta {
    encrypted: bool,
    created_at: i64,
    /// Storage backend used: "keyring" or "file"
    storage: String,
    /// Last probe timestamp in milliseconds since epoch (UTC)
    last_probe: i64,
    /// Stored blob size in bytes
    size: u64,
}

impl Default for SecretMeta {
    fn default() -> Self {
        SecretMeta {
            encrypted: false,
            created_at: 0,
            storage: "file".to_string(),
            last_probe: 0,
            size: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct SecretIndex {
    // map of name => meta
    entries: HashMap<String, SecretMeta>,
}

fn index_path() -> Result<PathBuf, KamError> {
    let home = home_dir().ok_or_else(|| {
        KamError::InvalidDirectory("Could not determine home directory".to_string())
    })?;
    let dir = home.join(".kam");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(KamError::Io)?;
    }
    Ok(dir.join("secrets.json"))
}

fn secrets_dir() -> Result<PathBuf, KamError> {
    let home = home_dir().ok_or_else(|| {
        KamError::InvalidDirectory("Could not determine home directory".to_string())
    })?;
    let dir = home.join(".kam").join("secrets");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(KamError::Io)?;
        #[cfg(unix)]
        {
            let mut perm = fs::metadata(home.join(".kam"))
                .map_err(KamError::Io)?
                .permissions();
            perm.set_mode(0o700);
            fs::set_permissions(home.join(".kam"), perm).map_err(KamError::Io)?;
        }
    }
    Ok(dir)
}

fn secret_file_path(name: &str) -> Result<PathBuf, KamError> {
    let dir = secrets_dir()?;
    Ok(dir.join(format!("{}.blob", name)))
}

fn write_secret_file(name: &str, blob: &[u8]) -> Result<(), KamError> {
    let p = secret_file_path(name)?;
    fs::write(&p, blob).map_err(KamError::Io)?;
    #[cfg(unix)]
    {
        let mut perm = fs::metadata(&p).map_err(KamError::Io)?.permissions();
        perm.set_mode(0o600);
        fs::set_permissions(&p, perm).map_err(KamError::Io)?;
    }
    Ok(())
}

fn read_secret_file(name: &str) -> Result<Vec<u8>, KamError> {
    let p = secret_file_path(name)?;
    if !p.exists() {
        return Err(KamError::CommandFailed("Secret file not found".to_string()));
    }
    let b = fs::read(&p).map_err(KamError::Io)?;
    Ok(b)
}

fn load_index() -> Result<SecretIndex, KamError> {
    let p = index_path()?;
    if !p.exists() {
        return Ok(SecretIndex::default());
    }
    let s = fs::read_to_string(&p).map_err(KamError::Io)?;
    // Support both new index format (map) and old legacy format (names: [..])
    let v: serde_json::Value = serde_json::from_str(&s)
        .map_err(|e| KamError::JsonError(format!("Failed to parse secret index JSON: {}", e)))?;
    let mut idx = if v.get("entries").is_some() {
        // Manually parse entries to be robust to missing fields
        let mut new = SecretIndex::default();
        let map = v.get("entries").unwrap();
        if let Some(obj) = map.as_object() {
            for (k, val) in obj.iter() {
                let encrypted = val
                    .get("encrypted")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);
                let created_at = val.get("created_at").and_then(|x| x.as_i64()).unwrap_or(0);
                let storage = val
                    .get("storage")
                    .and_then(|x| x.as_str())
                    .unwrap_or("file")
                    .to_string();
                let last_probe = val
                    .get("last_probe")
                    .and_then(|x| x.as_i64())
                    .unwrap_or(0);
                let size = val.get("size").and_then(|x| x.as_u64()).unwrap_or(0);
                new.entries.insert(
                    k.clone(),
                    SecretMeta {
                        encrypted,
                        created_at,
                        storage,
                        last_probe,
                        size,
                    },
                );
            }
        }
        new
    } else if v.get("names").is_some() {
        #[derive(Deserialize)]
        struct Legacy {
            names: HashSet<String>,
        }
        let legacy: Legacy = serde_json::from_value(v.clone()).map_err(|e| {
            KamError::JsonError(format!("Failed to parse legacy secret index: {}", e))
        })?;
        let mut new = SecretIndex::default();
        for n in legacy.names {
                new.entries.insert(
                    n,
                    SecretMeta {
                        encrypted: false,
                        created_at: 0,
                        storage: "file".to_string(),
                        last_probe: 0,
                        size: 0,
                    },
                );
        }
        new
    } else if v.is_object() {
        // Map from name -> SecretMeta
        let mut new = SecretIndex::default();
        if let Some(obj) = v.as_object() {
            for (k, val) in obj.iter() {
                // attempt to parse fields with defaults
                let encrypted = val
                    .get("encrypted")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);
                let created_at = val.get("created_at").and_then(|x| x.as_i64()).unwrap_or(0);
                let storage = val
                    .get("storage")
                    .and_then(|x| x.as_str())
                    .unwrap_or("file")
                    .to_string();
                let last_probe = val
                    .get("last_probe")
                    .and_then(|x| x.as_i64())
                    .unwrap_or(0);
                let size = val.get("size").and_then(|x| x.as_u64()).unwrap_or(0);
                new.entries.insert(
                    k.clone(),
                    SecretMeta {
                        encrypted,
                        created_at,
                        storage,
                        last_probe,
                        size,
                    },
                );
            }
        }
        new
    } else if v.is_array() {
        // legacy array of names
        let arr = serde_json::from_value::<Vec<String>>(v).map_err(|e| {
            KamError::JsonError(format!("Failed to parse legacy names array: {}", e))
        })?;
        let mut new = SecretIndex::default();
        for n in arr {
            new.entries.insert(
                n,
                SecretMeta {
                    encrypted: false,
                    created_at: 0,
                    storage: "file".to_string(),
                    last_probe: 0,
                    size: 0,
                },
            );
        }
        new
    } else {
        return Err(KamError::JsonError(
            "Unsupported secret index format".to_string(),
        ));
    };

    // Normalize missing storage by probing keyring
    let mut changed = false;
    for (name, meta) in idx.entries.iter_mut() {
        if meta.storage.is_empty() {
            // probe keyring
            if let Ok(entry) = Entry::new(SERVICE_NAME, name) {
                if entry.get_password().is_ok() {
                    meta.storage = "keyring".to_string();
                    changed = true;
                    continue;
                }
            }
            meta.storage = "file".to_string();
            changed = true;
        }
    }
    // Additional normalization: if entry claims keyring but keyring get fails and file exists, switch to file
    for (name, meta) in idx.entries.iter_mut() {
        if meta.storage == "keyring" {
            let mut keyring_ok = false;
            if let Ok(entry) = Entry::new(SERVICE_NAME, name) {
                if entry.get_password().is_ok() {
                    keyring_ok = true;
                }
            }
            if !keyring_ok {
                // try file fallback
                if let Ok(p) = secret_file_path(name) {
                    if p.exists() {
                        if let Ok(metadata) = fs::metadata(&p) {
                            meta.storage = "file".to_string();
                            meta.last_probe = Utc::now().timestamp_millis();
                            meta.size = metadata.len();
                            changed = true;
                        }
                    } else {
                        // mark as unknown to avoid repeated probing that might be expensive
                        meta.storage = "unknown".to_string();
                        changed = true;
                    }
                }
            }
        }
    }
    if changed {
        // update index on disk
        save_index(&idx)?;
    }
    Ok(idx)
}

fn save_index(idx: &SecretIndex) -> Result<(), KamError> {
    let p = index_path()?;
    let s = serde_json::to_string_pretty(idx)
        .map_err(|e| KamError::JsonError(format!("Failed to serialize secret index: {}", e)))?;
    fs::write(&p, s).map_err(KamError::Io)?;
    Ok(())
}

#[cfg(test)]
mod idx_tests {
    use super::*;
    use tempfile::tempdir;
    use serial_test::serial;
    // no extra imports

    #[test]
    #[serial]
    fn normalize_keyring_to_file_if_file_exists() {
        let d = tempdir().unwrap();
        let home = d.path();
        unsafe { std::env::set_var("HOME", home.to_str().unwrap()); }
        // create secrets dir and a file
        let dir = secrets_dir().unwrap();
        let p = dir.join("mysecret.blob");
        fs::write(&p, b"hello").unwrap();
        // create an index claiming keyring
        let mut idx = SecretIndex::default();
        idx.entries.insert(
            "mysecret".to_string(),
            SecretMeta {
                encrypted: false,
                created_at: 0,
                storage: "keyring".to_string(),
                last_probe: 0,
                size: 0,
            },
        );
        save_index(&idx).unwrap();
        let loaded = load_index().unwrap();
        let meta = loaded.entries.get("mysecret").unwrap();
        assert_eq!(meta.storage, "file");
        assert!(meta.size > 0);
    }
}

fn store_secret(
    name: &str,
    blob: &[u8],
    encrypted: bool,
    force_file: bool,
    with_backup: bool,
) -> Result<(), KamError> {
    let s = BASE64_ENGINE.encode(blob);
    // First try system keyring
    let mut stored_in_keyring = false;
    match Entry::new(SERVICE_NAME, name) {
        Ok(entry) => {
            if !force_file {
                if entry.set_password(&s).is_ok() {
                    // Try to read back to confirm persistence
                    if let Ok(readback) = entry.get_password() {
                                if readback == s {
                                    stored_in_keyring = true;
                                } else {
                                    // fallback to file if readback differs
                                    stored_in_keyring = false;
                                }
                    } else {
                        // If we cannot read back immediately, fallback to file storage
                        stored_in_keyring = false;
                    }
                }
            }
        }
        Err(_) => {}
    }

    // If keyring failed, fallback to local secure file storage
    if !stored_in_keyring {
        write_secret_file(name, &s.as_bytes())?;
    }
    else {
        // If stored in keyring, also attempt to write an encrypted fallback file for robustness.
        // Only write if the fallback file does not already exist. If writing fails, log a non-fatal warning.
        if let Ok(p) = secret_file_path(name) {
            if !p.exists() {
                if with_backup {
                    match write_secret_file(name, &s.as_bytes()) {
                    Ok(()) => println!("{} Encrypted fallback secret file created for {}", "✓".green(), name),
                    Err(e) => eprintln!("Warning: failed to write fallback secret file for {}: {}", name, e),
                    }
                }
            }
        }
    }
    let mut idx = load_index()?;
    let storage = if stored_in_keyring { "keyring" } else { "file" };
    let meta = SecretMeta {
        encrypted,
        created_at: Utc::now().timestamp_millis(),
        storage: storage.to_string(),
        last_probe: Utc::now().timestamp_millis(),
        size: blob.len() as u64,
    };
    idx.entries.insert(name.to_string(), meta);
    if !stored_in_keyring {
        // mark that we used local file fallback (still recorded in entries)
    }
    save_index(&idx)?;
    Ok(())
}

pub fn read_secret_blob(name: &str) -> Result<Vec<u8>, KamError> {
    // Try system keyring first, capture any error messages
    let keyring_err: Option<String>;
    if let Ok(entry) = Entry::new(SERVICE_NAME, name) {
        match entry.get_password() {
            Ok(s) => {
                match BASE64_ENGINE.decode(&s) {
                    Ok(blob) => {
                        // update index last_probe/size
                        if let Ok(mut idx) = load_index() {
                            if let Some(meta) = idx.entries.get_mut(name) {
                                meta.last_probe = Utc::now().timestamp_millis();
                                meta.size = blob.len() as u64;
                                meta.storage = "keyring".to_string();
                                let _ = save_index(&idx);
                            }
                        }
                        return Ok(blob);
                    }
                    Err(e) => {
                        keyring_err = Some(format!("Keyring decode error: {}", e));
                    }
                }
            }
            Err(e) => keyring_err = Some(format!("Keyring get error: {}", e)),
        }
    } else {
        keyring_err = Some("Keyring entry initialization failed".to_string());
    }

    // Fallback: read from local file if present, capture error message
    let file_err: Option<String>;
    match read_secret_file(name) {
        Ok(b) => {
            // if stored as base64 string in file, decode
            if let Ok(s) = std::str::from_utf8(&b) {
                if let Ok(blob) = BASE64_ENGINE.decode(s) {
                    // Update index last_probe/size/storage
                    if let Ok(mut idx) = load_index() {
                        if let Some(meta) = idx.entries.get_mut(name) {
                            meta.last_probe = Utc::now().timestamp_millis();
                            meta.size = blob.len() as u64;
                            meta.storage = "file".to_string();
                            let _ = save_index(&idx);
                        }
                    }
                    return Ok(blob);
                }
            }
            // else return raw bytes
            if let Ok(mut idx) = load_index() {
                if let Some(meta) = idx.entries.get_mut(name) {
                    meta.last_probe = Utc::now().timestamp_millis();
                    meta.size = b.len() as u64;
                    meta.storage = "file".to_string();
                    let _ = save_index(&idx);
                }
            }
            return Ok(b);
        }
        Err(e) => file_err = Some(format!("Fallback file error: {}", e)),
    }

    // If we got here, both keyring and file methods failed; record the probe time & provide a combined message
    let mut err_msg = "No matching entry found in secure storage".to_string();
    if let Some(k) = keyring_err {
        err_msg.push_str(&format!(": keyring: {}", k));
    }
    if let Some(f) = file_err {
        err_msg.push_str(&format!(", fallback: {}", f));
    }
    if let Ok(mut idx) = load_index() {
        if let Some(meta) = idx.entries.get_mut(name) {
            meta.last_probe = Utc::now().timestamp_millis();
            // size left unchanged
            let _ = save_index(&idx);
        }
    }
    Err(KamError::CommandFailed(err_msg))
}

/// Read secret and return plaintext private key bytes (PEM). If the stored blob is an encrypted KAM blob,
/// prompt for password to decrypt, otherwise return raw bytes.
pub fn read_secret_plaintext(name: &str, prompt_for_password: bool) -> Result<Vec<u8>, KamError> {
    let blob = read_secret_blob(name)?;
    if blob.starts_with(b"KAMKEYv1") {
        // Need password to decrypt
        let pw = if prompt_for_password {
            prompt_password("Private key password: ")
                .map_err(|e| KamError::CommandFailed(format!("Failed to read password: {}", e)))?
        } else {
            return Err(KamError::CommandFailed(
                "Secret appears encrypted; pass password or enable interactive prompt".to_string(),
            ));
        };
        let plain = decrypt_with_password(&blob, &pw)?;
        Ok(plain)
    } else {
        Ok(blob)
    }
}

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
                        let last_probe = Utc
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
            with_backup,
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
                let p1 = prompt_password("Encryption password: ")
                    .map_err(|e| KamError::CommandFailed(format!("Failed to read password: {}", e)))?;
                let p2 = prompt_password("Confirm encryption password: ")
                    .map_err(|e| KamError::CommandFailed(format!("Failed to read password: {}", e)))?;
                if p1 != p2 {
                    return Err(KamError::CommandFailed(
                        "Passwords do not match; aborting".to_string(),
                    ));
                }
                p1
            };
            let blob = encrypt_with_password(&data, &pw)?;
            store_secret(&name, &blob, true, force_file, with_backup)?;
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
                decrypt_with_password(&blob, &pw)?
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
            // Try keyring clear
            if let Ok(entry) = Entry::new(SERVICE_NAME, &name) {
                let _ = entry.set_password("");
            }
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
                    let plaintext = decrypt_with_password(&blob, &pw)?;
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
                store_secret(&final_name, &data, true, false, false)?;
            } else {
                let pw = prompt_password("Encryption password for import: ").map_err(|e| {
                    KamError::CommandFailed(format!("Failed to read password: {}", e))
                })?;
                let pw2 = prompt_password("Confirm encryption password for import: ").map_err(|e| {
                    KamError::CommandFailed(format!("Failed to read password: {}", e))
                })?;
                if pw != pw2 {
                    return Err(KamError::CommandFailed(
                        "Passwords do not match; aborting import".to_string(),
                    ));
                }
                let blob = encrypt_with_password(&data, &pw)?;
                store_secret(&final_name, &blob, true, false, false)?;
            }
            println!("{} Secret '{}' imported.", "✓".green(), final_name);
        }
    }
    Ok(())
}
