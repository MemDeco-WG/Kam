use colored::Colorize;
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};
use std::fs;
use std::io::{Read, Write};

use super::args::{SecretArgs, SecretCommands};
use super::file::secret_file_path;
use super::index::{load_index, save_index};
use super::utils::{global_with_backup_default, read_secret_blob};
use crate::errors::KamError;
use crate::utils::Utils;
use base64::engine::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use chrono::TimeZone;
use openssl::hash::MessageDigest;
use openssl::sign::Signer;
use rpassword::prompt_password;

/// Redact secret names for logging to avoid leaking sensitive metadata.
/// Shows first 2 and last 2 characters where available, otherwise a fixed placeholder.
fn redact_name(name: &str) -> String {
    let len = name.chars().count();
    if len <= 4 {
        "<redacted>".to_string()
    } else {
        let first: String = name.chars().take(2).collect();
        let last: String = name
            .chars()
            .rev()
            .take(2)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        format!("{first}...{last}")
    }
}

fn prompt_input<P: AsRef<str>>(prompt: P, default: Option<&str>) -> Result<String, KamError> {
    let prompt_ref = prompt.as_ref();
    let default_str = default.unwrap_or("").to_string();

    // Prefer dialoguer Input for a nicer interactive UI; fall back to stdio if it fails
    if let Ok(v) = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt_ref)
        .allow_empty(true)
        .default(default_str.clone())
        .interact_text()
    {
        return Ok(v);
    }

    // Fallback: standard input (non-TTY or dialoguer failure)
    if default_str.is_empty() {
        print!("{prompt_ref}: ");
    } else {
        print!("{prompt_ref} [{default_str}]: ");
    }
    std::io::stdout().flush().map_err(KamError::Io)?;
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(KamError::Io)?;
    let input = input.trim();
    if input.is_empty() {
        Ok(default_str)
    } else {
        Ok(input.to_string())
    }
}

/// Extract public key and signature from private key data.
/// Returns (pub_key_pem, pub_key_signature) if successful.
fn extract_public_key_and_signature(
    data: &[u8],
    password: &str,
) -> Result<(Option<String>, Option<String>), KamError> {
    // Try to parse private key, first without password, then with password
    let pkey = openssl::pkey::PKey::private_key_from_pem(data)
        .or_else(|_| {
            openssl::pkey::PKey::private_key_from_pem_passphrase(data, password.as_bytes())
        })
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse private key: {e}")))?;

    // Extract public key PEM
    let pem_bytes = pkey
        .public_key_to_pem()
        .map_err(|e| KamError::CommandFailed(format!("Failed to extract public key: {e}")))?;
    let pem_s = String::from_utf8_lossy(&pem_bytes).to_string();

    // Sign the PEM string
    let mut signer = Signer::new(MessageDigest::sha256(), &pkey)
        .map_err(|e| KamError::CommandFailed(format!("Failed to create signer: {e}")))?;
    signer
        .update(pem_s.as_bytes())
        .map_err(|e| KamError::CommandFailed(format!("Failed to update signer: {e}")))?;
    let sig = signer
        .sign_to_vec()
        .map_err(|e| KamError::CommandFailed(format!("Failed to sign: {e}")))?;
    let signature = BASE64_ENGINE.encode(&sig);

    Ok((Some(pem_s), Some(signature)))
}

/// Prompt for password with confirmation.
/// Returns Ok(None) if passwords don't match (caller should continue loop).
fn prompt_password_with_confirmation(
    prompt: &str,
    confirm_prompt: &str,
) -> Result<Option<String>, KamError> {
    use crate::i18n::tr;
    let pw1 = prompt_password(prompt)
        .map_err(|e| KamError::CommandFailed(format!("Failed to read password: {e}")))?;
    let pw2 = prompt_password(confirm_prompt)
        .map_err(|e| KamError::CommandFailed(format!("Failed to read password: {e}")))?;
    if pw1 != pw2 {
        Utils::warn(tr("secret.interactive.error.password_mismatch"));
        return Ok(None);
    }
    Ok(Some(pw1))
}

/// Show summary and confirm before adding secret.
fn show_add_summary_and_confirm(
    name: &str,
    storage_desc: &str,
    bytes: usize,
) -> Result<bool, KamError> {
    let idx = load_index()?;
    let confirm_prompt = if idx.entries.contains_key(name) {
        crate::trf!("secret.interactive.confirm_overwrite", name.to_string())
    } else {
        crate::trf!(
            "secret.interactive.confirm_before_add",
            redact_name(name),
            storage_desc.to_string(),
            bytes
        )
    };

    Utils::section(crate::i18n::tr("secret.interactive.summary"));
    Utils::kv("secret.interactive.summary_name", redact_name(name));
    Utils::kv("secret.interactive.summary_storage", storage_desc);
    Utils::kv(
        "secret.interactive.summary_encrypted",
        crate::i18n::tr("secret.interactive.yes"),
    );

    let proceed = if let Ok(v) = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(&confirm_prompt)
        .default(true)
        .interact()
    {
        v
    } else {
        let resp = prompt_input(&confirm_prompt, Some("y"))?;
        let resp = resp.trim().to_lowercase();
        resp == "y" || resp == "yes"
    };

    Ok(proceed)
}

/// Handle adding secret via direct input.
fn handle_interactive_add_direct(name: &str) -> Result<(), KamError> {
    use crate::i18n::tr;
    let value = prompt_input(tr("secret.interactive.enter_value"), None)?;
    if value.is_empty() {
        Utils::info(tr("secret.interactive.no_value_entered"));
        return Ok(());
    }

    let pw_prompt = tr("secret.interactive.encryption_password");
    let pw_confirm = tr("secret.interactive.confirm_encryption_password");
    let Some(password) = prompt_password_with_confirmation(&pw_prompt, &pw_confirm)? else {
        return Ok(());
    };

    let storage_desc = crate::i18n::tr("secret.interactive.storage.keyring").clone();
    let bytes = value.len();

    if !show_add_summary_and_confirm(name, &storage_desc, bytes)? {
        Utils::info(crate::i18n::tr("secret.interactive.aborted"));
        return Ok(());
    }

    run(SecretArgs {
        interactive: false,
        command: Some(SecretCommands::Add {
            name: name.to_string(),
            file: None,
            file_path: None,
            value: Some(value),
            force_file: false,
            password: Some(password),
            with_backup: false,
        }),
    })?;

    Utils::info(&crate::trf!(
        "secret.interactive.summary_added",
        redact_name(name),
        storage_desc,
        bytes
    ));
    Ok(())
}

/// Handle adding secret via file input.
fn handle_interactive_add_file(name: &str) -> Result<(), KamError> {
    use crate::i18n::tr;
    let path = prompt_input(tr("secret.interactive.enter_file_path"), None)?;
    if path.trim().is_empty() {
        Utils::info(tr("secret.interactive.no_file_entered"));
        return Ok(());
    }

    let pbuf = std::path::PathBuf::from(path);
    if !pbuf.exists() {
        Utils::warn(tr("secret.interactive.file_not_found"));
        return Ok(());
    }

    let pw_prompt = tr("secret.interactive.encryption_password");
    let pw_confirm = tr("secret.interactive.confirm_encryption_password");
    let Some(password) = prompt_password_with_confirmation(&pw_prompt, &pw_confirm)? else {
        return Ok(());
    };

    let len = std::fs::metadata(&pbuf).map_err(KamError::Io)?.len();
    let bytes = usize::try_from(len).map_err(|_| {
        KamError::CommandFailed("File size too large for this platform".to_string())
    })?;
    let storage_desc = crate::trf!("secret.interactive.storage.file", pbuf.display());

    if !show_add_summary_and_confirm(name, &storage_desc, bytes)? {
        Utils::info(crate::i18n::tr("secret.interactive.aborted"));
        return Ok(());
    }

    run(SecretArgs {
        interactive: false,
        command: Some(SecretCommands::Add {
            name: name.to_string(),
            file: Some(pbuf),
            file_path: None,
            value: None,
            force_file: true,
            password: Some(password),
            with_backup: false,
        }),
    })?;

    Utils::info(&crate::trf!(
        "secret.interactive.summary_added",
        redact_name(name),
        storage_desc,
        bytes
    ));
    Ok(())
}

/// Handle adding secret interactively.
fn handle_interactive_add() -> Result<(), KamError> {
    use crate::i18n::tr;
    let name = prompt_input(tr("secret.interactive.enter_name"), None)?;
    if name.trim().is_empty() {
        Utils::info(tr("secret.interactive.no_name_entered"));
        return Ok(());
    }

    let input_methods = vec![
        tr("secret.interactive.input_method.direct").clone(),
        tr("secret.interactive.input_method.file").clone(),
        tr("secret.interactive.cancel").clone(),
    ];
    let pick2 = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(tr("secret.interactive.choose_input_method"))
        .items(&input_methods)
        .default(0)
        .interact_opt();

    let method_idx = if let Ok(Some(i)) = pick2 {
        i
    } else {
        let input = prompt_input(tr("secret.interactive.select_input_method"), Some("1"))?;
        match input.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= input_methods.len() => n - 1,
            _ => {
                Utils::warn(tr("secret.interactive.invalid_selection"));
                return Ok(());
            }
        }
    };

    match method_idx {
        0 => handle_interactive_add_direct(&name)?,
        1 => handle_interactive_add_file(&name)?,
        _ => {}
    }
    Ok(())
}

/// Handle getting secret interactively.
fn handle_interactive_get() -> Result<(), KamError> {
    use crate::i18n::tr;
    let name = prompt_input(tr("secret.interactive.enter_name"), None)?;
    if name.trim().is_empty() {
        Utils::info(tr("secret.interactive.no_name_entered"));
    } else {
        run(SecretArgs {
            interactive: false,
            command: Some(SecretCommands::Get {
                name,
                out: None,
                password: None,
            }),
        })?;
    }
    Ok(())
}

/// Handle removing secret interactively.
fn handle_interactive_remove() -> Result<(), KamError> {
    use crate::i18n::tr;
    let name = prompt_input(tr("secret.interactive.enter_name"), None)?;
    if name.trim().is_empty() {
        Utils::info(tr("secret.interactive.no_name_entered"));
        return Ok(());
    }

    let confirmed = if let Ok(v) = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(crate::trf!("secret.confirm_remove", name.clone()))
        .default(false)
        .interact()
    {
        v
    } else {
        let prompt = crate::trf!("secret.confirm_remove", name.clone());
        let resp = prompt_input(&prompt, Some("n"))?;
        let resp = resp.trim().to_lowercase();
        resp == "y" || resp == "yes"
    };

    if confirmed {
        run(SecretArgs {
            interactive: false,
            command: Some(SecretCommands::Remove { name }),
        })?;
    }
    Ok(())
}

/// Select menu option from interactive menu.
fn select_menu_option(menu: &[String]) -> Result<Option<usize>, KamError> {
    use crate::i18n::tr;
    let pick = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(tr("secret.interactive.choose_action"))
        .items(menu)
        .default(0)
        .interact_opt();

    let idx = if let Ok(Some(i)) = pick {
        Some(i)
    } else {
        let input = prompt_input(tr("secret.interactive.select_option"), Some("1"))?;
        match input.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= menu.len() => Some(n - 1),
            _ => {
                Utils::warn(tr("secret.interactive.invalid_selection"));
                None
            }
        }
    };
    Ok(idx)
}

fn interactive_secrets() -> Result<(), KamError> {
    use crate::i18n::tr;
    Utils::banner(tr("secret.interactive.title"));
    Utils::info(tr("secret.interactive.intro"));
    println!();

    loop {
        let menu = vec![
            tr("secret.interactive.menu.add").clone(),
            tr("secret.interactive.menu.list").clone(),
            tr("secret.interactive.menu.get").clone(),
            tr("secret.interactive.menu.remove").clone(),
            tr("secret.interactive.menu.exit").clone(),
        ];

        let Some(idx) = select_menu_option(&menu)? else {
            continue;
        };

        match idx {
            0 => handle_interactive_add()?,
            1 => {
                run(SecretArgs {
                    interactive: false,
                    command: Some(SecretCommands::List),
                })?;
            }
            2 => handle_interactive_get()?,
            3 => handle_interactive_remove()?,
            4 => break,
            _ => unreachable!(),
        }
    }

    Ok(())
}

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

/// AUTO-GENERATED BY Kam/scripts/add_missing_docs.py: placeholder documentation for public function 'run'. Replace with real documentation. (2025-12-30T22:23:21Z)
///
/// # Errors
/// Returns `KamError` if secret operations fail (I/O errors, encryption/decryption failures, invalid input).
pub fn run(args: SecretArgs) -> Result<(), KamError> {
    if args.interactive {
        if args.command.is_some() {
            return Err(KamError::CommandFailed(crate::i18n::tr(
                "secret.interactive.error.conflict_with_subcommand",
            )));
        }
        return interactive_secrets();
    }

    let Some(cmd) = args.command else {
        return Err(KamError::CommandFailed(crate::i18n::tr(
            "secret.error.no_subcommand",
        )));
    };

    match cmd {
        SecretCommands::List => handle_list()?,
        SecretCommands::Add {
            name,
            file,
            file_path,
            value,
            force_file,
            password,
            with_backup,
        } => handle_add(
            &name,
            file.as_ref(),
            file_path.as_ref(),
            value.as_deref(),
            force_file,
            password.as_deref(),
            with_backup,
        )?,
        SecretCommands::Get {
            name,
            out,
            password,
        } => handle_get(&name, out.as_ref(), password.as_deref())?,
        SecretCommands::Remove { name } => handle_remove(&name)?,
        SecretCommands::Export {
            name,
            path,
            encrypted,
        } => handle_export(&name, &path, encrypted)?,
        SecretCommands::Import { path, name } => handle_import(&path, name)?,
        SecretCommands::ExportPub { name, out } => handle_export_pub(&name, out)?,
        SecretCommands::ImportCert {
            repo,
            issue,
            cert_chain,
            name,
        } => handle_import_cert(repo, issue, cert_chain.as_ref(), &name)?,
        SecretCommands::Trust {
            add_root,
            ca_name,
            list,
            remove,
        } => handle_trust(add_root, ca_name, list, remove)?,
    }
    Ok(())
}
