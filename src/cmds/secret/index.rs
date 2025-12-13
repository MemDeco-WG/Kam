use crate::errors::KamError;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct SecretMeta {
    pub encrypted: bool,
    pub created_at: i64,
    // 存储后端："keyring"或"file"（保留字段用于向后兼容）
    pub storage: String,
    // 最后探测时间戳（毫秒，UTC）
    pub last_probe: i64,
    // 存储的blob大小（字节）
    pub size: u64,
    // 缓存的公钥PEM（如果有的话），用于免密码操作
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pub_key_pem: Option<String>,
    // 公钥PEM的base64编码签名（用私钥签名）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pub_key_signature: Option<String>,
}

impl Default for SecretMeta {
    fn default() -> Self {
        SecretMeta {
            encrypted: false,
            created_at: 0,
            storage: "file".to_string(),
            last_probe: 0,
            size: 0,
            pub_key_pem: None,
            pub_key_signature: None,
        }
    }
}

// 密钥索引，存储所有密钥的元数据
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct SecretIndex {
    // 名字到元数据的映射
    pub entries: HashMap<String, SecretMeta>,
}

// 获取索引文件路径（~/.kam/secrets.json）
fn index_path() -> Result<PathBuf, KamError> {
    let home = dirs::home_dir().ok_or_else(|| {
        KamError::InvalidDirectory("Could not determine home directory".to_string())
    })?;
    let dir = home.join(".kam");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(KamError::Io)?;
    }
    Ok(dir.join("secrets.json"))
}

// 加载密钥索引
// 支持新格式（map）和旧格式（names数组），向后兼容
pub fn load_index() -> Result<SecretIndex, KamError> {
    let p = index_path()?;
    if !p.exists() {
        return Ok(SecretIndex::default());  // 文件不存在就返回空的
    }
    let s = fs::read_to_string(&p).map_err(KamError::Io)?;
    // 支持新索引格式（map）和旧遗留格式（names: [...]）
    // 这样旧版本的索引文件也能正常加载
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
                let last_probe = val.get("last_probe").and_then(|x| x.as_i64()).unwrap_or(0);
                let size = val.get("size").and_then(|x| x.as_u64()).unwrap_or(0);
                let pub_key_pem = val
                    .get("pub_key_pem")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                let pub_key_signature = val
                    .get("pub_key_signature")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                new.entries.insert(
                    k.clone(),
                    SecretMeta {
                        encrypted,
                        created_at,
                        storage,
                        last_probe,
                        size,
                        pub_key_pem,
                        pub_key_signature,
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
                    pub_key_pem: None,
                    pub_key_signature: None,
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
                let last_probe = val.get("last_probe").and_then(|x| x.as_i64()).unwrap_or(0);
                let size = val.get("size").and_then(|x| x.as_u64()).unwrap_or(0);
                let pub_key_pem = val
                    .get("pub_key_pem")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                let pub_key_signature = val
                    .get("pub_key_signature")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                new.entries.insert(
                    k.clone(),
                    SecretMeta {
                        encrypted,
                        created_at,
                        storage,
                        last_probe,
                        size,
                        pub_key_pem,
                        pub_key_signature,
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
                    pub_key_pem: None,
                    pub_key_signature: None,
                },
            );
        }
        new
    } else {
        return Err(KamError::JsonError(
            "Unsupported secret index format".to_string(),
        ));
    };

    // Normalize missing storage: default to 'file'
    let mut changed = false;
    for (_name, meta) in idx.entries.iter_mut() {
        if meta.storage.is_empty() {
            meta.storage = "file".to_string();
            changed = true;
        }
    }

    // Additional normalization: if entry claims keyring but keyring get fails and file exists, switch to file
    for (name, meta) in idx.entries.iter_mut() {
        if meta.storage == "keyring" {
            // Check fallback file
            let p = match super::file::secret_file_path(name) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if p.exists() {
                if let Ok(metadata) = fs::metadata(&p) {
                    meta.storage = "file".to_string();
                    meta.last_probe = Utc::now().timestamp_millis();
                    meta.size = metadata.len();
                    changed = true;
                }
            } else {
                // mark as unknown to avoid repeated probing
                meta.storage = "unknown".to_string();
                changed = true;
            }
        }
    }
    if changed {
        // update index on disk
        save_index(&idx)?;
    }
    Ok(idx)
}

pub fn save_index(idx: &SecretIndex) -> Result<(), KamError> {
    let p = index_path()?;
    let s = serde_json::to_string_pretty(idx)
        .map_err(|e| KamError::JsonError(format!("Failed to serialize secret index: {}", e)))?;
    fs::write(&p, s).map_err(KamError::Io)?;
    Ok(())
}
