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

