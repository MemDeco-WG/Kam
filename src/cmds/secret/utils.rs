use crate::cmds::secret::file::read_secret_file;
use crate::cmds::secret::index::{load_index, save_index};
use crate::errors::KamError;
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
        // Prefer an externally-provided passphrase from the environment for non-interactive flows.
        // This allows headless CI to provide a passphrase via a secure secret (e.g., KAM_SIGN_PASSPHRASE).
        let pw = if let Ok(pass) = std::env::var("KAM_SIGN_PASSPHRASE") {
            pass
        } else if prompt_for_password {
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

use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::sign::{Signer, Verifier};

// 获取或刷新公钥
// 先从索引里加载，如果有缓存就验证签名，不行就解密私钥重新计算
pub fn get_or_refresh_public_key(
    name: &str,
    verbose: bool,
) -> Result<PKey<openssl::pkey::Public>, KamError> {
    // 1. 先从索引里加载
    let idx = load_index()
        .map_err(|e| KamError::CommandFailed(format!("Failed to load secret index: {}", e)))?;

    // Check if we have both pem and signature
    let has_cache = if let Some(meta) = idx.entries.get(name) {
        meta.pub_key_pem.is_some() && meta.pub_key_signature.is_some()
    } else {
        return Err(KamError::CommandFailed(format!(
            "Secret '{}' not found in index",
            name
        )));
    };

    if has_cache {
        let meta = idx.entries.get(name).unwrap();
        let pem_str = meta.pub_key_pem.as_ref().unwrap();
        let sig_b64 = meta.pub_key_signature.as_ref().unwrap();

        // Attempt to parse and verify
        let parse_attempt =
            (|| -> Result<PKey<openssl::pkey::Public>, Box<dyn std::error::Error>> {
                let pkey = PKey::public_key_from_pem(pem_str.as_bytes())?;
                let sig_bytes = BASE64_ENGINE.decode(sig_b64)?;

                let pkey_clone = pkey.clone();
                let mut verifier = Verifier::new(MessageDigest::sha256(), &pkey_clone)?;
                verifier.update(pem_str.as_bytes())?;
                let valid = verifier.verify(&sig_bytes)?;
                if valid {
                    Ok(pkey)
                } else {
                    Err("Signature verification failed".into())
                }
            })();

        if let Ok(pkey) = parse_attempt {
            if verbose {
                println!("Using verified cached public key for '{}'", name);
            }
            return Ok(pkey);
        } else {
            if verbose {
                println!("Cache verification failed (tampered?), repairing...");
            }
        }
    }

    // 2. 回退方案：修复缓存
    // 解密私钥（如果需要密码会提示）
    let secret_bytes = read_secret_plaintext(name, true)?;

    // 解析私钥（尝试无密码，不行再试有密码的）
    let priv_key = if let Ok(pk) = PKey::private_key_from_pem(&secret_bytes) {
        pk
    } else if let Ok(pass) = std::env::var("KAM_SIGN_PASSPHRASE") {
        // 有密码，用密码解析
        PKey::private_key_from_pem_passphrase(&secret_bytes, pass.as_bytes()).map_err(|e| {
            KamError::CommandFailed(format!(
                "Failed to parse private key with passphrase: {}",
                e
            ))
        })?
    } else {
        return Err(KamError::CommandFailed(
            "Failed to parse private key from secret (passphrase needed?)".to_string(),
        ));
    };

    // 从私钥推导公钥并签名
    // 这样下次就不用再解密私钥了（直接用缓存的公钥）
    let pub_der = priv_key
        .public_key_to_der()
        .map_err(|e| KamError::CommandFailed(format!("Derive err: {}", e)))?;
    let pub_key = PKey::public_key_from_der(&pub_der)
        .map_err(|e| KamError::CommandFailed(format!("Pub parse err: {}", e)))?;
    let pem_bytes = pub_key
        .public_key_to_pem()
        .map_err(|e| KamError::CommandFailed(format!("PEM err: {}", e)))?;
    let pem_str = String::from_utf8(pem_bytes)
        .map_err(|e| KamError::CommandFailed(format!("UTF8 err: {}", e)))?;

    // 用私钥签名公钥（用于验证公钥的完整性）
    let mut signer = Signer::new(MessageDigest::sha256(), &priv_key)
        .map_err(|e| KamError::CommandFailed(format!("Sign init err: {}", e)))?;
    signer
        .update(pem_str.as_bytes())
        .map_err(|e| KamError::CommandFailed(format!("Sign update err: {}", e)))?;
    let sig_bytes = signer
        .sign_to_vec()
        .map_err(|e| KamError::CommandFailed(format!("Sign final err: {}", e)))?;
    let sig_b64 = BASE64_ENGINE.encode(&sig_bytes);

    // 更新索引（保存公钥和签名）
    // 重新加载索引，避免并发问题
    let mut idx = load_index()
        .map_err(|e| KamError::CommandFailed(format!("Failed to reload index: {}", e)))?;
    if let Some(meta) = idx.entries.get_mut(name) {
        meta.pub_key_pem = Some(pem_str);
        meta.pub_key_signature = Some(sig_b64);
        save_index(&idx)
            .map_err(|e| KamError::CommandFailed(format!("Failed to save index: {}", e)))?;
        if verbose {
            println!("Cache repaired/updated for '{}'", name);
        }
    }

    Ok(pub_key)
    // 缓存修复完成，下次就能直接用公钥了（不用再解密私钥）
}
