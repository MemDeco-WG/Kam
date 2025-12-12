use crate::cmds::secret::index::{SecretMeta, load_index, save_index};
use crate::errors::KamError;
use base64::engine::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use chrono::Utc;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn secrets_dir() -> Result<PathBuf, KamError> {
    let home = dirs::home_dir().ok_or_else(|| {
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

pub fn secret_file_path(name: &str) -> Result<PathBuf, KamError> {
    let dir = secrets_dir()?;
    Ok(dir.join(format!("{}.blob", name)))
}

pub fn write_secret_file(name: &str, blob: &[u8]) -> Result<(), KamError> {
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

pub fn read_secret_file(name: &str) -> Result<Vec<u8>, KamError> {
    let p = secret_file_path(name)?;
    if !p.exists() {
        return Err(KamError::CommandFailed("Secret file not found".to_string()));
    }
    let b = fs::read(&p).map_err(KamError::Io)?;
    Ok(b)
}

pub fn store_secret(
    name: &str,
    blob: &[u8],
    encrypted: bool,
    _force_file: bool,
    pub_key_pem: Option<String>,
    pub_key_signature: Option<String>,
) -> Result<(), KamError> {
    let s = BASE64_ENGINE.encode(blob);
    // Store only to local secure file storage
    write_secret_file(name, &s.as_bytes())?;

    // Local file storage
    let mut idx = load_index()?;
    let storage = "file";
    let meta = SecretMeta {
        encrypted,
        created_at: Utc::now().timestamp_millis(),
        storage: storage.to_string(),
        last_probe: Utc::now().timestamp_millis(),
        size: blob.len() as u64,
        pub_key_pem,
        pub_key_signature,
    };
    idx.entries.insert(name.to_string(), meta);
    save_index(&idx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    #[serial]
    fn add_and_read_with_file_fallback_updates_index() {
        let d = tempdir().unwrap();
        unsafe {
            std::env::set_var("HOME", d.path().to_str().unwrap());
        }
        let name = "test_secret";
        let blob = b"some-secret-data".to_vec();

        // Store using file fallback
        assert!(store_secret(name, &blob, false, true, None, None).is_ok());

        // Read back
        let read = read_secret_file(name).unwrap();
        // stored data is base64 encoded in file
        let decoded = BASE64_ENGINE
            .decode(std::str::from_utf8(&read).unwrap())
            .unwrap();
        assert_eq!(decoded, blob);

        // Index should contain metadata
        let idx = crate::cmds::secret::index::load_index().unwrap();
        let meta = idx.entries.get(name).unwrap();
        assert_eq!(meta.storage, "file");
        assert_eq!(meta.size, blob.len() as u64);
        assert!(meta.created_at > 0);
    }

    #[serial]
    fn add_with_backup_attempts_fallback_file() {
        let d = tempdir().unwrap();
        unsafe {
            std::env::set_var("HOME", d.path().to_str().unwrap());
        }
        let name = "baktest";
        let blob = b"secretdata".to_vec();
        // Attempt to store with backup (force_file = false, with_backup = true)
        let res = store_secret(name, &blob, true, false, None, None);
        assert!(res.is_ok());
        // Check index and/or file
        let idx = crate::cmds::secret::index::load_index().unwrap();
        if let Some(meta) = idx.entries.get(name) {
            assert_eq!(meta.storage, "file");
        }
        // Check fallback path exists
        let p = secret_file_path(name).unwrap();
        assert!(p.exists());
        let s = fs::read_to_string(&p).unwrap();
        assert!(!s.is_empty());
    }
}
