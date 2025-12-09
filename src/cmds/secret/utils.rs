use crate::cmds::secret::file::read_secret_file;
use crate::errors::KamError;
use crate::cmds::secret::index::{load_index, save_index};
use base64::engine::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use chrono::Utc;
use rpassword::prompt_password;

pub fn global_with_backup_default() -> bool {
    if let Some(home) = dirs::home_dir() {
        let cfg = home.join(".kam").join("config.toml");
        if cfg.exists() {
            if let Ok(s) = std::fs::read_to_string(&cfg) {
                if let Ok(v) = toml::from_str::<toml::Value>(&s) {
                    if let Some(sec) = v.get("secret") {
                        if let Some(b) = sec.get("with_backup").and_then(|x| x.as_bool()) {
                            return b;
                        }
                    }
                }
            }
        }
    }
    false
}

pub fn read_secret_blob(name: &str) -> Result<Vec<u8>, KamError> {
    // Fallback: read from local file; since we no longer use keyring we don't try it
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
        Err(e) => {
            // Update index probe time if we have the entry
            if let Ok(mut idx) = load_index() {
                if let Some(meta) = idx.entries.get_mut(name) {
                    meta.last_probe = Utc::now().timestamp_millis();
                    let _ = save_index(&idx);
                }
            }
            return Err(e);
        }
    }
}

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
        let plain = crate::cmds::secret_crypto::decrypt_with_password(&blob, &pw)?;
        Ok(plain)
    } else {
        Ok(blob)
    }
}
