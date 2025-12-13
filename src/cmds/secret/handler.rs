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
use openssl::sign::Signer;
use openssl::hash::MessageDigest;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use base64::engine::Engine as _;

/// Extract public key and signature from private key data.
/// Returns (pub_key_pem, pub_key_signature) if successful.
fn extract_public_key_and_signature(
    data: &[u8],
    password: &str,
) -> Result<(Option<String>, Option<String>), KamError> {
    // Try to parse private key, first without password, then with password
    let pkey = openssl::pkey::PKey::private_key_from_pem(data)
        .or_else(|_| openssl::pkey::PKey::private_key_from_pem_passphrase(data, password.as_bytes()))
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse private key: {}", e)))?;

    // Extract public key PEM
    let pem_bytes = pkey.public_key_to_pem()
        .map_err(|e| KamError::CommandFailed(format!("Failed to extract public key: {}", e)))?;
    let pem_s = String::from_utf8_lossy(&pem_bytes).to_string();

    // Sign the PEM string
    let mut signer = Signer::new(MessageDigest::sha256(), &pkey)
        .map_err(|e| KamError::CommandFailed(format!("Failed to create signer: {}", e)))?;
    signer.update(pem_s.as_bytes())
        .map_err(|e| KamError::CommandFailed(format!("Failed to update signer: {}", e)))?;
    let sig = signer.sign_to_vec()
        .map_err(|e| KamError::CommandFailed(format!("Failed to sign: {}", e)))?;
    let signature = BASE64_ENGINE.encode(&sig);

    Ok((Some(pem_s), Some(signature)))
}

pub fn run(args: SecretArgs) -> Result<(), KamError> {
    match args.command {
        SecretCommands::List => {
            let idx = load_index()?;
            if idx.entries.is_empty() {
                use crate::utils::Utils;
                Utils::info(&trf!("secret.no_secrets_stored"));
            } else {
                use crate::utils::Utils;
                Utils::section(crate::i18n::tr_key("secret.stored_secrets"));
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

            // 强制加密：总是需要密码并存储加密的blob
            // 虽然可能有点麻烦，但安全第一
            let pw = if let Some(pw) = password {
                pw
            } else {
                // 交互式输入密码，要确认两次
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

            // 尝试从私钥推导公钥并签名
            // 如果数据是私钥，就提取公钥并签名（用于验证）
            let mut pub_key_pem = None;
            let mut pub_key_signature = None;

            // 尝试解析私钥，先试试无密码的，不行再试有密码的
            // 虽然可能有点慢，但至少能兼容两种情况
            let pkey_res = openssl::pkey::PKey::private_key_from_pem(&data)
                .or_else(|_| openssl::pkey::PKey::private_key_from_pem_passphrase(&data, pw.as_bytes()));

            if let Ok(pkey) = pkey_res {
                 if let Ok(pem) = pkey.public_key_to_pem() {
                     let pem_s = String::from_utf8_lossy(&pem).to_string();
                     pub_key_pem = Some(pem_s.clone());

                     // Sign the PEM string
                     use openssl::sign::Signer;
                     use openssl::hash::MessageDigest;
                     use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
                     use base64::engine::Engine as _;

                     if let Ok(mut signer) = Signer::new(MessageDigest::sha256(), &pkey) {
                         if signer.update(pem_s.as_bytes()).is_ok() {
                             if let Ok(sig) = signer.sign_to_vec() {
                                 pub_key_signature = Some(BASE64_ENGINE.encode(&sig));
                             }
                         }
                     }
                 }
            }

            // Determine effective with_backup: CLI flag overrides global default
            let _default_with_backup = global_with_backup_default();
            // Always store to local file (no keyring)
            super::file::store_secret(&name, &blob, true, force_file, pub_key_pem, pub_key_signature)?;
            use crate::utils::Utils;
            Utils::success(&trf!("secret.saved", name));
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
                use crate::utils::Utils;
                Utils::success(&trf!("secret.written_to", name, path.display()));
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
            use crate::utils::Utils;
            Utils::success(&trf!("secret.removed", name));
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
            use crate::utils::Utils;
            Utils::success(&trf!("secret.exported", name, path.display()));
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
                // If importing an already encrypted blob, we can't easily derive the public key without the password.
                // We'll skip caching for now, or we could prompt for password to verify/cache (but user might not know it if just moving blobs).
                // Let's stick to storing as-is. Public key won't be cached until re-added or we add a 'refresh' command.
                super::file::store_secret(&final_name, &data, true, false, None, None)?;
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

                // Attempt to derive public key
                let (pub_key_pem, pub_key_signature) = extract_public_key_and_signature(&data, &pw)
                    .unwrap_or((None, None));

                super::file::store_secret(&final_name, &blob, true, false, pub_key_pem, pub_key_signature)?;
            }
            use crate::utils::Utils;
            Utils::success(&trf!("secret.imported", final_name));
        }
        SecretCommands::ExportPub { name, out } => {


            // Use helper to get/refresh public key (handles caching and fallback)
            let pkey = match crate::cmds::secret::utils::get_or_refresh_public_key(&name, true) {
                Ok(pk) => pk,
                Err(e) => {
                    return Err(KamError::CommandFailed(trf!("secret.failed_retrieve_public_key", name, e)));
                }
            };

            // 4. Derive Public Key PEM
            let pub_pem = pkey.public_key_to_pem().map_err(|e| KamError::CommandFailed(format!("Failed to derive public key: {}", e)))?;

            // 5. Output
            if let Some(path) = out {
                fs::write(&path, &pub_pem).map_err(KamError::Io)?;
                 use crate::utils::Utils;
                 Utils::success(&trf!("secret.public_key_exported", name, path.display()));
            } else {
                let s = String::from_utf8_lossy(&pub_pem);
                print!("{}", s);
            }
        }
        SecretCommands::ImportCert {
            repo,
            issue,
            cert_chain,
            name,
        } => {
            let chain_pem = if let Some(chain_path) = cert_chain {
                // Load from file
                fs::read_to_string(&chain_path).map_err(KamError::Io)?
            } else if let (Some(repo_str), Some(issue_num)) = (repo, issue) {
                // Fetch from GitHub
                let parts: Vec<&str> = repo_str.split('/').collect();
                if parts.len() != 2 {
                    return Err(KamError::CommandFailed(
                        "Repository must be in format 'owner/repo'".to_string(),
                    ));
                }
                let owner = parts[0];
                let repo_name = parts[1];

                use crate::utils::Utils;
                Utils::executing(&trf!("secret.fetching_cert_from_github", issue_num));
                super::github::fetch_cert_from_issue(owner, repo_name, issue_num)?
            } else {
                return Err(KamError::CommandFailed(
                    "Must provide either --cert-chain or both --repo and --issue".to_string(),
                ));
            };

            // Store the certificate chain
            super::cert::store_cert_chain(&name, &chain_pem)?;
            use crate::utils::Utils;
            Utils::success(&trf!("secret.cert_chain_imported", name));
        }
        SecretCommands::Trust {
            add_root,
            ca_name,
            list,
            remove,
        } => {
            if list {
                // List trusted CAs
                let cas = super::cert::list_trusted_cas()?;
                if cas.is_empty() {
                    use crate::utils::Utils;
                    Utils::info(crate::i18n::tr_key("No trusted Root CAs."));
                } else {
                    use crate::utils::Utils;
                    Utils::section(crate::i18n::tr_key("Trusted Root CAs"));
                    for (name, fingerprint) in cas {
                        Utils::kv(&name, &fingerprint[..16]);
                    }
                }
            } else if let Some(ca_path_or_url) = add_root {
                let ca_name = ca_name.ok_or_else(|| {
                    KamError::CommandFailed("--ca-name is required when adding a Root CA".to_string())
                })?;

                // Load CA certificate
                let ca_pem = if ca_path_or_url.starts_with("http://") || ca_path_or_url.starts_with("https://") {
                    // Fetch from URL
                    use crate::utils::Utils;
                    Utils::executing(&trf!("secret.fetching_root_ca", ca_path_or_url));
                    reqwest::blocking::get(&ca_path_or_url)
                        .map_err(|e| KamError::CommandFailed(trf!("secret.failed_fetch_ca", e)))?
                        .text()
                        .map_err(|e| KamError::CommandFailed(trf!("secret.failed_read_ca", e)))?
                } else {
                    // Load from file
                    fs::read_to_string(&ca_path_or_url).map_err(KamError::Io)?
                };

                // Add to trust store
                super::cert::add_trusted_ca(&ca_pem, &ca_name)?;
                use crate::utils::Utils;
                Utils::success(&trf!("secret.root_ca_added", ca_name));
            } else if let Some(ca_name) = remove {
                // Remove CA
                super::cert::remove_trusted_ca(&ca_name)?;
                use crate::utils::Utils;
                Utils::success(&trf!("secret.root_ca_removed", ca_name));
            } else {
                return Err(KamError::CommandFailed(
                    "Must provide --list, --add-root, or --remove".to_string(),
                ));
            }
        }
    }
    Ok(())
}
