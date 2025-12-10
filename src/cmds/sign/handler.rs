use crate::cmds::secret::read_secret_plaintext;
use crate::errors::KamError;
use crate::cmds::sign::args::SignArgs;
use colored::*;
use openssl::hash::{hash, MessageDigest};
use openssl::pkey::PKey;
use openssl::sign::Signer;
use std::fs;
use std::path::Path;
use openssl::x509::{X509, X509Builder, X509NameBuilder};
use openssl::asn1::Asn1Time;
use std::collections::HashMap as StdHashMap;
// signature types used for TSA

pub fn run(args: SignArgs) -> Result<(), KamError> {
    let src_path = Path::new(&args.src);
    if !src_path.exists() {
        return Err(KamError::InvalidDirectory(format!(
            "Source not found: {}",
            args.src
        )));
    }

    // Ensure out dir exists
    let out_dir = Path::new(&args.out);
    if !out_dir.exists() {
        fs::create_dir_all(out_dir).map_err(KamError::Io)?;
    }

    // Read private key: prefer --key-path if provided, otherwise use secret in keyring
    let pem_bytes = if let Some(kp) = args.key_path.as_ref() {
        fs::read(kp).map_err(KamError::Io)?
    } else {
        read_secret_plaintext(&args.secret, true)?
    };
    // Parse key
    let pkey = PKey::private_key_from_pem(&pem_bytes)
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse private key PEM: {}", e)))?;

    // Read file
    let data = fs::read(src_path).map_err(KamError::Io)?;

    // Sign digest using ECDSA
    let mut signer = Signer::new(MessageDigest::sha256(), &pkey)
        .map_err(|e| KamError::CommandFailed(format!("Failed to create signer: {}", e)))?;
    signer
        .update(&data)
        .map_err(|e| KamError::CommandFailed(format!("Failed to update signer: {}", e)))?;
    let sig_der = signer
        .sign_to_vec()
        .map_err(|e| KamError::CommandFailed(format!("Failed to sign: {}", e)))?;

    // Output signature file
    let filename = src_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| KamError::InvalidFilename("Invalid source filename".to_string()))?;
    let sig_file = out_dir.join(format!("{}.sig", filename));
    use base64::engine::Engine as _;
    use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
    let sig_b64 = BASE64_ENGINE.encode(&sig_der);
    fs::write(&sig_file, sig_b64.as_bytes()).map_err(KamError::Io)?;

    // Optionally include cert chain; if provided, write a .cert.pem next to signature
    if let Some(cert_path) = args.cert.as_ref() {
        let cert_data = fs::read_to_string(cert_path).map_err(KamError::Io)?;
        let cert_file = out_dir.join(format!("{}.cert.pem", filename));
        fs::write(&cert_file, cert_data).map_err(KamError::Io)?;
    }

    println!(
        "{} Signed '{}' -> {}",
        "✓".green(),
        filename,
        sig_file.display()
    );

    // Optionally create a Sigstore DSSE bundle JSON if requested
    // Also attempt to obtain an RFC3161 timestamp if timestamping is enabled
    let mut tsr_opt: Option<Vec<u8>> = None;
    if args.timestamp {
        // Attempt to fetch timestamp from Sigstore TSA using the default endpoint
        use sigstore_tsa::TimestampClient as SigstoreTsaClient;
        // We'll use RFC3161 timestamp requests; no Sigstore JSON body is used
        // Build TSA candidates list
        let mut candidates: Vec<String> = Vec::new();
        // accept a comma-separated list from CLI --tsa-url
        if let Some(cli_urls) = args.tsa_url.as_ref() {
            for part in cli_urls.split(',') {
                let part = part.trim();
                if !part.is_empty() {
                    candidates.push(part.to_string());
                }
            }
        }

        // read global config sign.tsa.candidates if exists
        if let Some(home) = dirs::home_dir() {
            let cfg_path = home.join(".kam").join("config.toml");
            if cfg_path.exists() {
                if let Ok(s) = std::fs::read_to_string(&cfg_path) {
                    if let Ok(v) = toml::from_str::<toml::Value>(&s) {
                        if let Some(arr) = v
                            .get("sign")
                            .and_then(|x| x.get("tsa"))
                            .and_then(|x| x.get("candidates"))
                            .and_then(|x| x.as_array())
                        {
                            for it in arr {
                                if let Some(url) = it.as_str() {
                                    if !candidates.contains(&url.to_string()) {
                                        candidates.push(url.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Default TSA endpoints if none provided
        if candidates.is_empty() {
            candidates.push("https://timestamp.sigstore.dev/api/v1/timestamp".to_string());
            candidates.push("https://timestamp.digicert.com".to_string());
            candidates.push("https://freetsa.org/tsr".to_string());
        }

        // load/history map from config: sign.tsa.history
        let mut history_map: StdHashMap<String, bool> = StdHashMap::new();
        if let Some(home) = dirs::home_dir() {
            let cfg_path = home.join(".kam").join("config.toml");
            if cfg_path.exists() {
                if let Ok(s) = std::fs::read_to_string(&cfg_path) {
                    if let Ok(v) = toml::from_str::<toml::Value>(&s) {
                        if let Some(tbl) = v
                            .get("sign")
                            .and_then(|x| x.get("tsa"))
                            .and_then(|x| x.get("history"))
                            .and_then(|x| x.as_table())
                        {
                            for (k, v) in tbl.iter() {
                                history_map.insert(k.clone(), v.as_bool().unwrap_or(false));
                            }
                        }
                    }
                }
            }
        }

        // sort candidates: previously successful first
        candidates.sort_by(|a, b| {
            let a_success = history_map.get(a).copied().unwrap_or(false);
            let b_success = history_map.get(b).copied().unwrap_or(false);
            // successes first
            match (a_success, b_success) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        });

        // Prepare SignatureBytes wrapper for timestamp_signature API
        use sigstore_types::SignatureBytes;
        let sig_bytes = SignatureBytes::from(sig_der.as_slice());

        // We'll attempt each candidate in order until we get a successful timestamp
        let mut last_err: Option<String> = None;
        for url in candidates.iter() {
            // Use sigstore-tsa client to build and send RFC3161 requests
            let client = SigstoreTsaClient::new(url.clone());
            // Use tokio runtime to call async API from sync code
            match tokio::runtime::Runtime::new() {
                Ok(rt) => {
                    match rt.block_on(client.timestamp_signature(&sig_bytes)) {
                        Ok(token) => {
                            tsr_opt = Some(token.as_bytes().to_vec());
                            println!("{} Obtained RFC3161 timestamp from TSA {}", "✓".green(), url);
                            history_map.insert(url.clone(), true);
                            // persist updated history map
                            if let Err(e) = update_tsa_history_in_config(&history_map) {
                                eprintln!("{} Failed to write TSA history to config: {}", "!".yellow(), e);
                            }
                            break;
                        }
                        Err(e) => {
                            last_err = Some(format!("TSA {} error: {}", url, e));
                            history_map.insert(url.clone(), false);
                            if let Err(e) = update_tsa_history_in_config(&history_map) {
                                eprintln!("{} Failed to write TSA history to config: {}", "!".yellow(), e);
                            }
                            // try next candidate
                        }
                    }
                }
                Err(e) => {
                    last_err = Some(format!("TSA runtime error for {}: {}", url, e));
                    history_map.insert(url.clone(), false);
                }
            }
        }
        if tsr_opt.is_none() {
            if let Some(err) = last_err {
                eprintln!("{} Failed to obtain TSA timestamp: {}. Skipping timestamp.", "!".yellow(), err);
            } else {
                eprintln!("{} Failed to obtain TSA timestamp for unknown reasons; skipping timestamp.", "!".yellow());
            }
        }
    }

    // the closing brace of `if args.timestamp` was here; `run` function continues below

    if args.sigstore {
        use serde_json::json;
        // base64 encoder available from outer scope
        use hex;
        // Compute artifact digest
        let digest = hash(MessageDigest::sha256(), &data)
            .map_err(|e| KamError::CommandFailed(format!("Digest error: {}", e)))?;
        let digest_hex = hex::encode(digest);
        let payload_data = json!({
            "_type": "https://in-toto.io/Statement/v1",
            "subject": [
                { "name": filename, "digest": { "sha256": digest_hex } }
            ],
            "predicateType": "https://in-toto.io/attestation/release/v0.1",
            "predicate": {}
        });
        let payload_bytes = serde_json::to_vec(&payload_data)?;
        // Sign the payload
        let mut payload_signer = Signer::new(MessageDigest::sha256(), &pkey)
            .map_err(|e| KamError::CommandFailed(format!("Failed to create payload signer: {}", e)))?;
        payload_signer
            .update(&payload_bytes)
            .map_err(|e| KamError::CommandFailed(format!("Failed to update payload signer: {}", e)))?;
        let payload_sig = payload_signer
            .sign_to_vec()
            .map_err(|e| KamError::CommandFailed(format!("Failed to sign payload: {}", e)))?;

        // Prepare certificate DER bytes: prefer provided cert, otherwise self-signed
        let cert_der_opt = if let Some(cert_path) = args.cert.as_ref() {
            let pem = fs::read(cert_path).map_err(KamError::Io)?;
            let x509 = X509::from_pem(&pem)
                .map_err(|e| KamError::CommandFailed(format!("Failed to parse cert PEM: {}", e)))?;
            Some(x509.to_der().map_err(|e| KamError::CommandFailed(format!("Failed to DER encode certificate: {}", e)))?)
        } else {
            // Build self-signed cert from private key info
            let mut name_b = X509NameBuilder::new().map_err(|e| KamError::CommandFailed(format!("Failed to create X509NameBuilder: {}", e)))?;
            name_b.append_entry_by_text("CN", filename).map_err(|e| KamError::CommandFailed(format!("Failed to append CN: {}", e)))?;
            let name = name_b.build();
            let mut builder = X509Builder::new().map_err(|e| KamError::CommandFailed(format!("Failed to create X509Builder: {}", e)))?;
            builder.set_subject_name(&name).map_err(|e| KamError::CommandFailed(format!("Failed to set subject name: {}", e)))?;
            builder.set_issuer_name(&name).map_err(|e| KamError::CommandFailed(format!("Failed to set issuer name: {}", e)))?;
            builder.set_pubkey(&pkey).map_err(|e| KamError::CommandFailed(format!("Failed to set public key: {}", e)))?;
            let not_before = Asn1Time::days_from_now(0).map_err(|e| KamError::CommandFailed(format!("Failed to set not_before: {}", e)))?;
            let not_after = Asn1Time::days_from_now(365).map_err(|e| KamError::CommandFailed(format!("Failed to set not_after: {}", e)))?;
            builder.set_not_before(&not_before).map_err(|e| KamError::CommandFailed(format!("Failed to set not_before on builder: {}", e)))?;
            builder.set_not_after(&not_after).map_err(|e| KamError::CommandFailed(format!("Failed to set not_after on builder: {}", e)))?;
            builder.sign(&pkey, MessageDigest::sha256()).map_err(|e| KamError::CommandFailed(format!("Failed to sign cert: {}", e)))?;
            let cert = builder.build();
            Some(cert.to_der().map_err(|e| KamError::CommandFailed(format!("Failed to encode certificate DER: {}", e)))?)
        };

            // Write bundle file, including timestamp token if present
            super::sigstore::write_sigstore_bundle(
                out_dir,
                filename,
                &payload_data,
                &payload_sig,
                cert_der_opt.as_deref(),
                tsr_opt.as_deref(),
            )?;
    }

    // If not generating a Sigstore bundle but a timestamp was obtained, write it as a separate file
    if !args.sigstore {
        if let Some(tsr) = tsr_opt.as_ref() {
            let tsr_file = out_dir.join(format!("{}.tsr", filename));
            fs::write(&tsr_file, tsr).map_err(KamError::Io)?;
            println!("{} Saved timestamp token to {}", "✓".green(), tsr_file.display());
        }
    }

    Ok(())
}

// Persist TSA history into the global config file (~/.kam/config.toml)
fn update_tsa_history_in_config(history_map: &StdHashMap<String, bool>) -> Result<(), KamError> {
    let home = dirs::home_dir().ok_or_else(|| KamError::CommandFailed("Cannot determine home directory".to_string()))?;
    let cfg_dir = home.join(".kam");
    if !cfg_dir.exists() {
        std::fs::create_dir_all(&cfg_dir).map_err(KamError::Io)?;
    }
    let cfg_path = cfg_dir.join("config.toml");
    // Load existing config or create a new table
    let mut root_val = if cfg_path.exists() {
        let s = std::fs::read_to_string(&cfg_path).map_err(KamError::Io)?;
        toml::from_str::<toml::Value>(&s).map_err(|e| KamError::CommandFailed(format!("Failed parse config.toml: {}", e)))?
    } else {
        toml::Value::Table(Default::default())
    };

    // Ensure sign.tsa.history exists
    let sign_table = root_val.as_table_mut().unwrap();
    let tsa_table_val = sign_table
        .entry("sign")
        .or_insert_with(|| toml::Value::Table(Default::default()));
    let tsa_table = tsa_table_val.as_table_mut().unwrap();
    let history_val = tsa_table
        .entry("tsa")
        .or_insert_with(|| toml::Value::Table(Default::default()));
    let hist_table = history_val.as_table_mut().unwrap();

    // Overwrite history keys
    for (k, v) in history_map.iter() {
        hist_table.insert(k.clone(), toml::Value::Boolean(*v));
    }

    // Write back
    let out = toml::to_string_pretty(&root_val).map_err(|e| KamError::CommandFailed(format!("Failed to serialize config: {}", e)))?;
    std::fs::write(&cfg_path, out).map_err(KamError::Io)?;
    Ok(())
}
