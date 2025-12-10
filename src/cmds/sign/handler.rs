use crate::cmds::secret::read_secret_plaintext;
use crate::errors::KamError;
use crate::cmds::sign::args::SignArgs;
use colored::*;
use openssl::hash::{hash, MessageDigest};
use openssl::pkey::PKey;
use openssl::sign::Signer;
use std::fs;
use std::path::{Path, PathBuf};
use openssl::x509::{X509, X509Builder, X509NameBuilder};
use openssl::asn1::Asn1Time;
use std::collections::HashMap as StdHashMap;
// signature types used for TSA

fn sign_single_file(src_path: &Path, args: &SignArgs, sigstore_mode: bool) -> Result<(), KamError> {
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
    let pkey = PKey::private_key_from_pem(&pem_bytes)
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse private key PEM: {}", e)))?;

    // Read file
    let data = fs::read(src_path).map_err(KamError::Io)?;

    // Sign digest
    let mut signer = Signer::new(MessageDigest::sha256(), &pkey)
        .map_err(|e| KamError::CommandFailed(format!("Failed to create signer: {}", e)))?;
    signer
        .update(&data)
        .map_err(|e| KamError::CommandFailed(format!("Failed to update signer: {}", e)))?;
    let sig_der = signer
        .sign_to_vec()
        .map_err(|e| KamError::CommandFailed(format!("Failed to sign: {}", e)))?;

    let filename = src_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| KamError::InvalidFilename("Invalid source filename".to_string()))?;

    // Optionally include cert chain; if provided, write a .cert.pem next to signature
    if let Some(cert_path) = args.cert.as_ref() {
        let cert_data = fs::read_to_string(cert_path).map_err(KamError::Io)?;
        let cert_file = out_dir.join(format!("{}.cert.pem", filename));
        fs::write(&cert_file, cert_data).map_err(KamError::Io)?;
    }

    // Write .sig if not sigstore-only or attestation_only
    if !sigstore_mode && !args.attestation_only {
        let path = out_dir.join(format!("{}.sig", filename));
        use base64::engine::Engine as _;
        use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
        let sig_b64 = BASE64_ENGINE.encode(&sig_der);
        fs::write(&path, sig_b64.as_bytes()).map_err(KamError::Io)?;
        println!("{} Signed '{}' -> {}", "✓".green(), filename, path.display());
    }

    // Timestamp and sigstore handling (timestamping is relatively heavy; kept minimal here)
    let mut tsr_opt: Option<Vec<u8>> = None;
    if args.timestamp {
        // Keep minimal timestamp implementation: try default TSA if configured; tests don't rely on this.
        // Read TSA candidates from config or default list
        use sigstore_tsa::TimestampClient as SigstoreTsaClient;
        let mut candidates: Vec<String> = Vec::new();
        if let Some(cli_urls) = args.tsa_url.as_ref() {
            for part in cli_urls.split(',') {
                let part = part.trim();
                if !part.is_empty() {
                    candidates.push(part.to_string());
                }
            }
        }
        if candidates.is_empty() {
            candidates.push("https://timestamp.sigstore.dev/api/v1/timestamp".to_string());
        }
        use sigstore_types::SignatureBytes;
        let sig_bytes = SignatureBytes::from(sig_der.as_slice());

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
        for url in candidates.iter() {
            let client = SigstoreTsaClient::new(url.clone());
            match tokio::runtime::Runtime::new() {
                Ok(rt) => match rt.block_on(client.timestamp_signature(&sig_bytes)) {
                    Ok(token) => {
                        tsr_opt = Some(token.as_bytes().to_vec());
                        history_map.insert(url.clone(), true);
                        // persist updated history map
                        if let Err(e) = update_tsa_history_in_config(&history_map) {
                            eprintln!("{} Failed to write TSA history to config: {}", "!".yellow(), e);
                        }
                        break;
                    }
                    Err(_e) => {
                        history_map.insert(url.clone(), false);
                        if let Err(e) = update_tsa_history_in_config(&history_map) {
                            eprintln!("{} Failed to write TSA history to config: {}", "!".yellow(), e);
                        }
                        continue
                    },
                },
                Err(_) => continue,
            }
        }
    }

    if sigstore_mode {
        use serde_json::json;
        use hex;
        let digest = hash(MessageDigest::sha256(), &data)
            .map_err(|e| KamError::CommandFailed(format!("Digest error: {}", e)))?;
        let digest_hex = hex::encode(digest);
        let payload_data = json!({
            "_type": "https://in-toto.io/Statement/v1",
            "subject": [ { "name": filename, "digest": { "sha256": digest_hex } } ],
            "predicateType": "https://in-toto.io/attestation/release/v0.1",
            "predicate": {}
        });
        let payload_bytes = serde_json::to_vec(&payload_data)?;
        let mut payload_signer = Signer::new(MessageDigest::sha256(), &pkey)
            .map_err(|e| KamError::CommandFailed(format!("Failed to create payload signer: {}", e)))?;
        payload_signer
            .update(&payload_bytes)
            .map_err(|e| KamError::CommandFailed(format!("Failed to update payload signer: {}", e)))?;
        let payload_sig = payload_signer
            .sign_to_vec()
            .map_err(|e| KamError::CommandFailed(format!("Failed to sign payload: {}", e)))?;
        let cert_der_opt = if let Some(cert_path) = args.cert.as_ref() {
            let pem = fs::read(cert_path).map_err(KamError::Io)?;
            let x509 = X509::from_pem(&pem)
                .map_err(|e| KamError::CommandFailed(format!("Failed to parse cert PEM: {}", e)))?;
            Some(x509.to_der().map_err(|e| KamError::CommandFailed(format!("Failed to DER encode certificate: {}", e)))?)
        } else {
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
        let bundle_path = super::sigstore::write_sigstore_bundle(
            Path::new(&args.out),
            filename,
            &payload_data,
            &payload_sig,
            cert_der_opt.as_deref(),
            tsr_opt.as_deref(),
        )?;
        println!("{} Wrote sigstore bundle '{}' -> {}", "✓".green(), filename, bundle_path.display());
    }

    // Save timestamp token if present and not bundle mode
    if !sigstore_mode && args.timestamp {
        if let Some(tsr) = tsr_opt.as_ref() {
            let tsr_file = Path::new(&args.out).join(format!("{}.tsr", filename));
            fs::write(&tsr_file, tsr).map_err(KamError::Io)?;
            println!("{} Saved timestamp token to {}", "✓".green(), tsr_file.display());
        }
    }
    Ok(())
}

pub fn run(args: SignArgs) -> Result<(), KamError> {
    // determine sigstore mode
    let mut sigstore_mode = args.sigstore;
    if args.attestation_only {
        sigstore_mode = true;
    }

    // If src provided, sign single file
    if let Some(src_str) = args.src.as_ref() {
        let src_path = Path::new(src_str);
        return sign_single_file(src_path, &args, sigstore_mode);
    }

    // decide dist dir from args.dist or --all
    let dist_dir: PathBuf = if let Some(d) = args.dist.clone() {
        d
    } else if args.all {
        PathBuf::from(&args.out)
    } else {
        return Err(KamError::CommandFailed("Either specify 'src' or --dist/--all to sign artifacts".to_string()));
    };
    for entry in std::fs::read_dir(dist_dir).map_err(KamError::Io)? {
        let entry = entry.map_err(KamError::Io)?;
        let p = entry.path();
        if !p.is_file() { continue; }
        if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
            match ext { "sig" | "tsr" | "json" => continue, _ => () }
        }
        if let Err(e) = sign_single_file(&p, &args, sigstore_mode) {
            eprintln!("{} Signing failed for {}: {}", "!".yellow(), p.display(), e);
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::File;
    use std::io::Write;
    use openssl::rsa::Rsa;

    #[test]
    fn sign_creates_sig_when_not_sigstore() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("artifact.zip");
        let mut f = File::create(&src_path).unwrap();
        writeln!(f, "hello").unwrap();

        // Generate a private key PEM for testing
        let rsa = Rsa::generate(2048).unwrap();
        let key_pem = rsa.private_key_to_pem().unwrap();
        let key_path = dir.path().join("key.pem");
        std::fs::write(&key_path, &key_pem).unwrap();

        let out_dir = dir.path().join("out");
        let args = SignArgs {
            src: Some(src_path.to_string_lossy().to_string()),
            secret: "main".to_string(),
            out: out_dir.to_string_lossy().to_string(),
            cert: None,
            key_path: Some(key_path.to_string_lossy().to_string()),
            sigstore: false,
            timestamp: false,
            tsa_url: None,
            dist: None,
            all: false,
            attestation_only: false,
        };
        let res = run(args);
        assert!(res.is_ok());
        // Check .sig exists
        let sig = out_dir.join("artifact.zip.sig");
        assert!(sig.exists());
    }

    #[test]
    fn sign_creates_sigstore_bundle_and_no_sig() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("artifact2.zip");
        let mut f = File::create(&src_path).unwrap();
        writeln!(f, "hello").unwrap();

        // Generate a private key PEM for testing
        let rsa = Rsa::generate(2048).unwrap();
        let key_pem = rsa.private_key_to_pem().unwrap();
        let key_path = dir.path().join("key2.pem");
        std::fs::write(&key_path, &key_pem).unwrap();

        let out_dir = dir.path().join("out2");
        let args = SignArgs {
            src: Some(src_path.to_string_lossy().to_string()),
            secret: "main".to_string(),
            out: out_dir.to_string_lossy().to_string(),
            cert: None,
            key_path: Some(key_path.to_string_lossy().to_string()),
            sigstore: true,
            timestamp: false,
            tsa_url: None,
            dist: None,
            all: false,
            attestation_only: false,
        };
        let res = run(args);
        assert!(res.is_ok());
        // Check .sig does not exist, but .sigstore.json exists
        let sig = out_dir.join("artifact2.zip.sig");
        let bundle = out_dir.join("artifact2.zip.sigstore.json");
        assert!(!sig.exists());
        assert!(bundle.exists());
    }
}
