/// Handle List command.
fn handle_list() -> Result<(), KamError> {
    let idx = load_index()?;
    if idx.entries.is_empty() {
        Utils::info(&trf!("secret.no_secrets_stored"));
    } else {
        Utils::section(crate::i18n::tr("secret.stored_secrets"));
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
                let size_str = if meta.size > 0 {
                    format!("{size}B", size = meta.size)
                } else {
                    "-".to_string()
                };
                let last_probe = chrono::Utc
                    .timestamp_millis_opt(meta.last_probe)
                    .single()
                    .map_or_else(|| "-".to_string(), |t| t.to_rfc3339());
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
    Ok(())
}

/// Read secret data from file, value, or stdin.
fn read_secret_data(
    file: Option<std::path::PathBuf>,
    file_path: Option<std::path::PathBuf>,
    value: Option<String>,
) -> Result<Vec<u8>, KamError> {
    let chosen_file = file.or(file_path);
    if let Some(path) = chosen_file {
        Ok(fs::read(&path).map_err(KamError::Io)?)
    } else if let Some(v) = value {
        Ok(v.into_bytes())
    } else {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(KamError::Io)?;
        Ok(s.into_bytes())
    }
}

/// Prompt for password interactively with confirmation.
fn prompt_password_interactive() -> Result<String, KamError> {
    let p1 = prompt_password("Encryption password: ")
        .map_err(|e| KamError::CommandFailed(format!("Failed to read password: {e}")))?;
    let p2 = prompt_password("Confirm encryption password: ")
        .map_err(|e| KamError::CommandFailed(format!("Failed to read password: {e}")))?;
    if p1 != p2 {
        return Err(KamError::CommandFailed(
            "Passwords do not match; aborting".to_string(),
        ));
    }
    Ok(p1)
}

/// Extract public key and signature from data if it's a private key.
fn extract_pub_key_from_data(data: &[u8], password: &str) -> (Option<String>, Option<String>) {
    let pkey_res = openssl::pkey::PKey::private_key_from_pem(data).or_else(|_| {
        openssl::pkey::PKey::private_key_from_pem_passphrase(data, password.as_bytes())
    });

    if let Ok(pkey) = pkey_res
        && let Ok(pem) = pkey.public_key_to_pem()
    {
        let pem_s = String::from_utf8_lossy(&pem).to_string();
        let pub_key_pem = Some(pem_s.clone());

        if let Ok(mut signer) = Signer::new(MessageDigest::sha256(), &pkey)
            && signer.update(pem_s.as_bytes()).is_ok()
            && let Ok(sig) = signer.sign_to_vec()
        {
            let pub_key_signature = Some(BASE64_ENGINE.encode(&sig));
            return (pub_key_pem, pub_key_signature);
        }
        (pub_key_pem, None)
    } else {
        (None, None)
    }
}

/// Handle Add command.
fn handle_add(
    name: &str,
    file: Option<&std::path::PathBuf>,
    file_path: Option<&std::path::PathBuf>,
    value: Option<&str>,
    force_file: bool,
    password: Option<&str>,
    _with_backup: bool,
) -> Result<(), KamError> {
    let data = read_secret_data(file.cloned(), file_path.cloned(), value.map(str::to_string))?;

    let pw = if let Some(pw) = password {
        pw.to_string()
    } else {
        prompt_password_interactive()?
    };
    let blob = crate::cmds::secret_crypto::encrypt_with_password(&data, &pw)?;

    let (pub_key_pem, pub_key_signature) = extract_pub_key_from_data(&data, &pw);

    let _default_with_backup = global_with_backup_default();
    super::file::store_secret(
        name,
        &blob,
        true,
        force_file,
        pub_key_pem,
        pub_key_signature,
    )?;
    Utils::success(&trf!("secret.saved", redact_name(name)));
    Ok(())
}

/// Handle Get command.
fn handle_get(
    name: &str,
    out: Option<&std::path::PathBuf>,
    password: Option<&str>,
) -> Result<(), KamError> {
    let plaintext = if let Some(pw) = password {
        let blob = read_secret_blob(name)?;
        if blob.starts_with(b"KAMKEYv1") {
            crate::cmds::secret_crypto::decrypt_with_password(&blob, pw)?
        } else {
            return Err(KamError::CommandFailed("Stored secret is not encrypted; please re-import or add using the new required password flow (kam secret add ...)".to_string()));
        }
    } else {
        crate::cmds::secret::utils::read_secret_plaintext(name, true)?
    };

    if let Some(path) = out {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(KamError::Io)?;
        f.write_all(&plaintext).map_err(KamError::Io)?;
        Utils::success(&trf!(
            "secret.written_to",
            redact_name(name),
            path.display()
        ));
    } else {
        let s = String::from_utf8_lossy(&plaintext);
        println!("{s}");
    }
    Ok(())
}

/// Handle Remove command.
fn handle_remove(name: &str) -> Result<(), KamError> {
    if let Ok(p) = secret_file_path(name)
        && p.exists()
    {
        let _ = fs::remove_file(&p);
    }
    let mut idx = load_index()?;
    idx.entries.remove(name);
    save_index(&idx)?;
    Utils::success(&trf!("secret.removed", redact_name(name)));
    Ok(())
}

/// Handle Export command.
fn handle_export(name: &str, path: &std::path::PathBuf, encrypted: bool) -> Result<(), KamError> {
    let blob = read_secret_blob(name)?;
    if encrypted {
        fs::write(path, &blob).map_err(KamError::Io)?;
    } else {
        let plaintext = if blob.starts_with(b"KAMKEYv1") {
            crate::cmds::secret::utils::read_secret_plaintext(name, true)?
        } else {
            blob
        };
        fs::write(path, &plaintext).map_err(KamError::Io)?;
    }
    Utils::success(&trf!("secret.exported", redact_name(name), path.display()));
    Ok(())
}

/// Handle Import command.
fn handle_import(path: &std::path::PathBuf, name: Option<String>) -> Result<(), KamError> {
    let data = fs::read(path).map_err(KamError::Io)?;
    let final_name = name.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("imported")
            .to_string()
    });

    if data.starts_with(b"KAMKEYv1") {
        super::file::store_secret(&final_name, &data, true, false, None, None)?;
    } else {
        let pw = prompt_password("Encryption password for import: ")
            .map_err(|e| KamError::CommandFailed(format!("Failed to read password: {e}")))?;
        let pw2 = prompt_password("Confirm encryption password for import: ")
            .map_err(|e| KamError::CommandFailed(format!("Failed to read password: {e}")))?;
        if pw != pw2 {
            return Err(KamError::CommandFailed(
                "Passwords do not match; aborting import".to_string(),
            ));
        }
        let blob = crate::cmds::secret_crypto::encrypt_with_password(&data, &pw)?;
        let (pub_key_pem, pub_key_signature) =
            extract_public_key_and_signature(&data, &pw).unwrap_or((None, None));
        super::file::store_secret(
            &final_name,
            &blob,
            true,
            false,
            pub_key_pem,
            pub_key_signature,
        )?;
    }
    Utils::success(&trf!("secret.imported", redact_name(&final_name)));
    Ok(())
}

/// Handle ExportPub command.
fn handle_export_pub(name: &str, out: Option<std::path::PathBuf>) -> Result<(), KamError> {
    let pkey = match crate::cmds::secret::utils::get_or_refresh_public_key(name, true) {
        Ok(pk) => pk,
        Err(e) => {
            return Err(KamError::CommandFailed(trf!(
                "secret.failed_retrieve_public_key",
                name,
                e
            )));
        }
    };

    let pub_pem = pkey
        .public_key_to_pem()
        .map_err(|e| KamError::CommandFailed(format!("Failed to derive public key: {e}")))?;

    if let Some(path) = out {
        fs::write(&path, &pub_pem).map_err(KamError::Io)?;
        Utils::success(&trf!(
            "secret.public_key_exported",
            redact_name(name),
            path.display()
        ));
    } else {
        let s = String::from_utf8_lossy(&pub_pem);
        print!("{s}");
    }
    Ok(())
}

/// Handle ImportCert command.
fn handle_import_cert(
    repo: Option<String>,
    issue: Option<u32>,
    cert_chain: Option<&std::path::PathBuf>,
    name: &str,
) -> Result<(), KamError> {
    let chain_pem = if let Some(chain_path) = cert_chain {
        fs::read_to_string(chain_path).map_err(KamError::Io)?
    } else if let (Some(repo_str), Some(issue_num)) = (repo, issue) {
        let parts: Vec<&str> = repo_str.split('/').collect();
        if parts.len() != 2 {
            return Err(KamError::CommandFailed(
                "Repository must be in format 'owner/repo'".to_string(),
            ));
        }
        let owner = parts[0];
        let repo_name = parts[1];
        Utils::executing(&trf!("secret.fetching_cert_from_github", issue_num));
        super::github::fetch_cert_from_issue(owner, repo_name, issue_num)?
    } else {
        return Err(KamError::CommandFailed(
            "Must provide either --cert-chain or both --repo and --issue".to_string(),
        ));
    };

    super::cert::store_cert_chain(name, &chain_pem)?;
    Utils::success(&trf!("secret.cert_chain_imported", redact_name(name)));
    Ok(())
}

/// Handle Trust command.
fn handle_trust(
    add_root: Option<String>,
    ca_name: Option<String>,
    list: bool,
    remove: Option<String>,
) -> Result<(), KamError> {
    if list {
        let cas = super::cert::list_trusted_cas()?;
        if cas.is_empty() {
            Utils::info(crate::i18n::tr("secret.no_trusted_root_cas"));
        } else {
            Utils::section(crate::i18n::tr("Trusted Root CAs"));
            for (name, fingerprint) in cas {
                Utils::kv(&name, &fingerprint[..16]);
            }
        }
    } else if let Some(ca_path_or_url) = add_root {
        let ca_name = ca_name.ok_or_else(|| {
            KamError::CommandFailed("--ca-name is required when adding a Root CA".to_string())
        })?;

        let ca_pem =
            if ca_path_or_url.starts_with("http://") || ca_path_or_url.starts_with("https://") {
                Utils::executing(&trf!("secret.fetching_root_ca", ca_path_or_url));
                reqwest::blocking::get(&ca_path_or_url)
                    .map_err(|e| KamError::CommandFailed(trf!("secret.failed_fetch_ca", e)))?
                    .text()
                    .map_err(|e| KamError::CommandFailed(trf!("secret.failed_read_ca", e)))?
            } else {
                fs::read_to_string(&ca_path_or_url).map_err(KamError::Io)?
            };

        super::cert::add_trusted_ca(&ca_pem, &ca_name)?;
        Utils::success(&trf!("secret.root_ca_added", ca_name));
    } else if let Some(ca_name) = remove {
        super::cert::remove_trusted_ca(&ca_name)?;
        Utils::success(&trf!("secret.root_ca_removed", ca_name));
    } else {
        return Err(KamError::CommandFailed(
            "Must provide --list, --add-root, or --remove".to_string(),
        ));
    }
    Ok(())
}

fn handle_ksu_generate(
    name: &str,
    out: &std::path::Path,
    no_gpg: bool,
    force: bool,
) -> Result<(), KamError> {
    let generated = super::ksu::generate_key_pair(name, out, no_gpg, force)?;
    Utils::success(format!(
        "KernelSU developer public key: {}",
        generated.public_key_path.display()
    ));
    if generated.used_gpg {
        Utils::success(format!(
            "KernelSU developer private key encrypted with gpg: {}",
            generated.private_key_path.display()
        ));
    } else {
        Utils::warn(format!(
            "KernelSU developer private key stored as PEM: {}",
            generated.private_key_path.display()
        ));
    }
    Ok(())
}

fn handle_ksu_submit(
    username: &str,
    public_key: &std::path::Path,
    open: bool,
) -> Result<(), KamError> {
    let public_key_pem = super::ksu::read_public_key(public_key)?;
    let url = super::ksu::submit_issue_url(username, &public_key_pem);
    super::ksu::emit_issue_url(&url, open)
}

fn handle_ksu_revoke(
    username: &str,
    serial_number: Option<String>,
    cert: Option<std::path::PathBuf>,
    reason: &super::ksu::KsuRevokeReason,
    details: &str,
    open: bool,
) -> Result<(), KamError> {
    let serial = match (serial_number, cert) {
        (Some(serial), _) => serial,
        (None, Some(cert_path)) => super::ksu::serial_from_certificate(&cert_path)?,
        (None, None) => {
            return Err(KamError::CommandFailed(
                "Provide --serial-number or --cert".to_string(),
            ));
        }
    };
    let url = super::ksu::revoke_issue_url(username, &serial, reason, details);
    super::ksu::emit_issue_url(&url, open)
}

