use crate::cmds::secret::read_secret_plaintext;
use crate::cmds::sign::args::SignArgs;
use crate::errors::KamError;
use base64::engine::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use colored::*;
use openssl::asn1::Asn1Time;
use openssl::hash::{MessageDigest, hash};
use openssl::pkey::PKey;
use openssl::sign::Signer;
use openssl::x509::{X509, X509Builder, X509NameBuilder};
use reqwest::blocking::Client;
use serde_json::Value;
use std::collections::HashMap as StdHashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
// signature types used for TSA

// Helper: parse possible certificate encodings returned by Fulcio (PEM, base64 DER).
fn extract_cert_der_from_fulcio_response(body: &Value) -> Option<Vec<u8>> {
    // Parse a string that might be PEM or base64-encoded DER into certificate DER bytes.
    fn parse_cert_from_str(s: &str) -> Option<Vec<u8>> {
        // Try PEM first
        if s.contains("-----BEGIN CERTIFICATE-----") {
            if let Ok(x) = X509::from_pem(s.as_bytes()) {
                return x.to_der().ok();
            }
        }
        // Try base64 decode then interpret as DER or PEM
        if let Ok(decoded) = BASE64_ENGINE.decode(s.as_bytes()) {
            if let Ok(x) = X509::from_der(&decoded) {
                return x.to_der().ok();
            }
            if let Ok(x) = X509::from_pem(&decoded) {
                return x.to_der().ok();
            }
        }
        None
    }

    // Prefer 'signedCertificate' (single cert), but also handle various other fields
    if let Some(signed) = body.get("signedCertificate").and_then(|v| v.as_str()) {
        if let Some(der) = parse_cert_from_str(signed) {
            return Some(der);
        }
    }
    // 'certChain' may be an array or a concatenated PEM or base64
    if let Some(cc) = body.get("certChain") {
        if let Some(arr) = cc.as_array() {
            for item in arr.iter() {
                if let Some(s) = item.as_str() {
                    if let Some(der) = parse_cert_from_str(s) {
                        return Some(der);
                    }
                }
            }
        } else if let Some(s) = cc.as_str() {
            // If concatenated PEMs, extract the first PEM
            if s.contains("-----BEGIN CERTIFICATE-----") {
                let parts: Vec<&str> = s.split("-----BEGIN CERTIFICATE-----").collect();
                for p in parts {
                    let candidate = format!("-----BEGIN CERTIFICATE-----{}", p);
                    if let Some(der) = parse_cert_from_str(&candidate) {
                        return Some(der);
                    }
                }
            } else if let Some(der) = parse_cert_from_str(s) {
                return Some(der);
            }
        }
    }
    if let Some(cert_field) = body.get("certificate").and_then(|v| v.as_str()) {
        if let Some(der) = parse_cert_from_str(cert_field) {
            return Some(der);
        }
    }
    None
}

// Query Fulcio's signingCert endpoint to obtain a short-lived certificate for `pkey` using an OIDC token.
// Returns (leaf_cert_der, optional_full_pem_chain).
fn request_cert_from_fulcio(
    pkey: &PKey<openssl::pkey::Private>,
    token: &str,
    fulcio_url: &str,
) -> Result<(Vec<u8>, Option<String>), KamError> {
    use serde_json::json;
    // Build public key SPKI DER from the private key. The PKey API provides public key DER export.
    let pub_der = pkey
        .public_key_to_der()
        .map_err(|e| KamError::CommandFailed(format!("Failed to obtain public key DER: {}", e)))?;
    let spki_b64 = BASE64_ENGINE.encode(&pub_der);
    let payload = json!({
        "publicKey": { "content": spki_b64 }
    });

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| KamError::CommandFailed(format!("Failed to create HTTP client: {}", e)))?;

    let endpoint = if fulcio_url.ends_with('/') {
        format!("{}api/v1/signingCert", fulcio_url)
    } else {
        format!("{}/api/v1/signingCert", fulcio_url)
    };

    let resp = client
        .post(&endpoint)
        .bearer_auth(token)
        .json(&payload)
        .send()
        .map_err(|e| KamError::CommandFailed(format!("Fulcio request failed: {}", e)))?;

    // Read the response body once to avoid moving `resp` multiple times.
    let status = resp.status();
    let body_txt = resp.text().map_err(|e| {
        KamError::CommandFailed(format!("Failed to read Fulcio response body: {}", e))
    })?;

    if !status.is_success() {
        return Err(KamError::CommandFailed(format!(
            "Fulcio returned non-success status {}: {}",
            status.as_u16(),
            if body_txt.len() > 200 {
                format!("{}...", &body_txt[..200])
            } else {
                body_txt.clone()
            }
        )));
    }

    let resp_json: Value = serde_json::from_str(&body_txt)
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse Fulcio response: {}", e)))?;

    // Extract leaf DER
    if let Some(leaf_der) = extract_cert_der_from_fulcio_response(&resp_json) {
        // Try to build full PEM chain (if certChain or signedCertificate present)
        let mut pem_chain: Option<String> = None;
        if let Some(cc) = resp_json.get("certChain") {
            if let Some(arr) = cc.as_array() {
                let mut out = String::new();
                for item in arr {
                    if let Some(s) = item.as_str() {
                        if s.contains("-----BEGIN CERTIFICATE-----") {
                            out.push_str(s);
                            out.push('\n');
                        } else if let Ok(decoded) = BASE64_ENGINE.decode(s.as_bytes()) {
                            if let Ok(x) = X509::from_der(&decoded) {
                                if let Ok(p) = x.to_pem() {
                                    out.push_str(&String::from_utf8_lossy(&p));
                                }
                            }
                        }
                    }
                }
                if !out.is_empty() {
                    pem_chain = Some(out);
                }
            } else if let Some(s) = cc.as_str() {
                if s.contains("-----BEGIN CERTIFICATE-----") {
                    pem_chain = Some(s.to_string());
                } else if let Ok(decoded) = BASE64_ENGINE.decode(s.as_bytes()) {
                    if let Ok(x) = X509::from_der(&decoded) {
                        if let Ok(p) = x.to_pem() {
                            pem_chain = Some(String::from_utf8_lossy(&p).to_string());
                        }
                    }
                }
            }
        }
        if pem_chain.is_none() {
            if let Some(signed) = resp_json.get("signedCertificate").and_then(|v| v.as_str()) {
                if signed.contains("-----BEGIN CERTIFICATE-----") {
                    pem_chain = Some(signed.to_string());
                } else if let Ok(decoded) = BASE64_ENGINE.decode(signed.as_bytes()) {
                    if let Ok(x) = X509::from_der(&decoded) {
                        if let Ok(p) = x.to_pem() {
                            pem_chain = Some(String::from_utf8_lossy(&p).to_string());
                        }
                    }
                }
            }
        }
        Ok((leaf_der, pem_chain))
    } else {
        Err(KamError::CommandFailed(
            "Fulcio response did not contain a valid certificate".to_string(),
        ))
    }
}

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

    // Try parsing PEM directly; if it fails and a passphrase is supplied via
    // KAM_SIGN_PASSPHRASE, retry parsing as an encrypted PEM with the passphrase.
    let pkey = match PKey::private_key_from_pem(&pem_bytes) {
        Ok(pk) => pk,
        Err(orig_err) => {
            if let Ok(pass) = std::env::var("KAM_SIGN_PASSPHRASE") {
                // Attempt to parse an encrypted PEM using the provided passphrase.
                // Fallback: if parsing still fails, return a helpful error.
                PKey::private_key_from_pem_passphrase(&pem_bytes, pass.as_bytes()).map_err(|e| {
                    KamError::CommandFailed(format!(
                        "Failed to parse private key PEM with passphrase: {}",
                        e
                    ))
                })?
            } else {
                return Err(KamError::CommandFailed(format!(
                    "Failed to parse private key PEM: {}",
                    orig_err
                )));
            }
        }
    };

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

    // Write .sig if not sigstore-only
    if !sigstore_mode {
        let path = out_dir.join(format!("{}.sig", filename));
        let sig_b64 = BASE64_ENGINE.encode(&sig_der);
        fs::write(&path, sig_b64.as_bytes()).map_err(KamError::Io)?;
        println!(
            "{} Signed '{}' -> {}",
            "✓".green(),
            filename,
            path.display()
        );
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
                            eprintln!(
                                "{} Failed to write TSA history to config: {}",
                                "!".yellow(),
                                e
                            );
                        }
                        break;
                    }
                    Err(_e) => {
                        history_map.insert(url.clone(), false);
                        if let Err(e) = update_tsa_history_in_config(&history_map) {
                            eprintln!(
                                "{} Failed to write TSA history to config: {}",
                                "!".yellow(),
                                e
                            );
                        }
                        continue;
                    }
                },
                Err(_) => continue,
            }
        }
    }

    if sigstore_mode {
        use hex;
        use serde_json::json;
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
        let mut payload_signer = Signer::new(MessageDigest::sha256(), &pkey).map_err(|e| {
            KamError::CommandFailed(format!("Failed to create payload signer: {}", e))
        })?;
        payload_signer.update(&payload_bytes).map_err(|e| {
            KamError::CommandFailed(format!("Failed to update payload signer: {}", e))
        })?;
        let payload_sig = payload_signer
            .sign_to_vec()
            .map_err(|e| KamError::CommandFailed(format!("Failed to sign payload: {}", e)))?;
        let cert_der_opt = if let Some(cert_path) = args.cert.as_ref() {
            let pem = fs::read(cert_path).map_err(KamError::Io)?;
            let x509 = X509::from_pem(&pem)
                .map_err(|e| KamError::CommandFailed(format!("Failed to parse cert PEM: {}", e)))?;
            Some(x509.to_der().map_err(|e| {
                KamError::CommandFailed(format!("Failed to DER encode certificate: {}", e))
            })?)
        } else {
            // Attempt to obtain a Fulcio certificate if requested or if a token is present.
            let mut cert_der_local: Option<Vec<u8>> = None;
            let oidc_token_opt: Option<String> = if let Some(t) = args.oidc_token.as_ref() {
                Some(t.clone())
            } else {
                match env::var(&args.oidc_token_env) {
                    Ok(v) if !v.is_empty() => Some(v),
                    _ => None,
                }
            };

            let try_fulcio = args.fulcio || oidc_token_opt.is_some();
            if try_fulcio {
                if let Some(ref token) = oidc_token_opt {
                    match request_cert_from_fulcio(&pkey, token.as_str(), &args.fulcio_url) {
                        Ok((leaf_der, maybe_pem_chain)) => {
                            cert_der_local = Some(leaf_der.clone());
                            if let Some(pem_chain_str) = maybe_pem_chain {
                                let cert_file = out_dir.join(format!("{}.cert.pem", filename));
                                if let Err(e) = fs::write(&cert_file, pem_chain_str.as_bytes()) {
                                    eprintln!(
                                        "{} Failed to write Fulcio cert PEM: {}",
                                        "!".yellow(),
                                        e
                                    );
                                }
                            } else {
                                if let Ok(x) = X509::from_der(&leaf_der) {
                                    if let Ok(p) = x.to_pem() {
                                        let cert_file =
                                            out_dir.join(format!("{}.cert.pem", filename));
                                        if let Err(e) = fs::write(&cert_file, &p) {
                                            eprintln!(
                                                "{} Failed to write Fulcio leaf cert PEM: {}",
                                                "!".yellow(),
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("{} Fulcio certificate request failed: {}", "!".yellow(), e);
                        }
                    }
                } else {
                    eprintln!(
                        "{} Fulcio requested, but no OIDC token found (env: {}); falling back to self-signed certificate",
                        "!".yellow(),
                        args.oidc_token_env
                    );
                }
            }

            if cert_der_local.is_some() {
                cert_der_local
            } else {
                // Fallback to self-signed if no certificate from Fulcio
                let mut name_b = X509NameBuilder::new().map_err(|e| {
                    KamError::CommandFailed(format!("Failed to create X509NameBuilder: {}", e))
                })?;
                name_b
                    .append_entry_by_text("CN", filename)
                    .map_err(|e| KamError::CommandFailed(format!("Failed to append CN: {}", e)))?;
                let name = name_b.build();
                let mut builder = X509Builder::new().map_err(|e| {
                    KamError::CommandFailed(format!("Failed to create X509Builder: {}", e))
                })?;
                builder.set_subject_name(&name).map_err(|e| {
                    KamError::CommandFailed(format!("Failed to set subject name: {}", e))
                })?;
                builder.set_issuer_name(&name).map_err(|e| {
                    KamError::CommandFailed(format!("Failed to set issuer name: {}", e))
                })?;
                builder.set_pubkey(&pkey).map_err(|e| {
                    KamError::CommandFailed(format!("Failed to set public key: {}", e))
                })?;
                let not_before = Asn1Time::days_from_now(0).map_err(|e| {
                    KamError::CommandFailed(format!("Failed to set not_before: {}", e))
                })?;
                let not_after = Asn1Time::days_from_now(365).map_err(|e| {
                    KamError::CommandFailed(format!("Failed to set not_after: {}", e))
                })?;
                builder.set_not_before(&not_before).map_err(|e| {
                    KamError::CommandFailed(format!("Failed to set not_before on builder: {}", e))
                })?;
                builder.set_not_after(&not_after).map_err(|e| {
                    KamError::CommandFailed(format!("Failed to set not_after on builder: {}", e))
                })?;
                builder
                    .sign(&pkey, MessageDigest::sha256())
                    .map_err(|e| KamError::CommandFailed(format!("Failed to sign cert: {}", e)))?;
                let cert = builder.build();
                Some(cert.to_der().map_err(|e| {
                    KamError::CommandFailed(format!("Failed to encode certificate DER: {}", e))
                })?)
            }
        };
        let bundle_path = super::sigstore::write_sigstore_bundle(
            Path::new(&args.out),
            filename,
            &payload_data,
            &payload_sig,
            cert_der_opt.as_deref(),
            tsr_opt.as_deref(),
        )?;
        println!(
            "{} Wrote sigstore bundle '{}' -> {}",
            "✓".green(),
            filename,
            bundle_path.display()
        );
    }

    // Save timestamp token if present and not bundle mode
    if !sigstore_mode && args.timestamp {
        if let Some(tsr) = tsr_opt.as_ref() {
            let tsr_file = Path::new(&args.out).join(format!("{}.tsr", filename));
            fs::write(&tsr_file, tsr).map_err(KamError::Io)?;
            println!(
                "{} Saved timestamp token to {}",
                "✓".green(),
                tsr_file.display()
            );
        }
    }
    Ok(())
}

pub fn run(args: SignArgs) -> Result<(), KamError> {
    // determine sigstore mode
    let sigstore_mode = args.sigstore;
    // attestation-only flag removed; sigstore_mode follows args.sigstore

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
        return Err(KamError::CommandFailed(
            "Either specify 'src' or --dist/--all to sign artifacts".to_string(),
        ));
    };
    for entry in std::fs::read_dir(dist_dir).map_err(KamError::Io)? {
        let entry = entry.map_err(KamError::Io)?;
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
            match ext {
                "sig" | "tsr" | "json" => continue,
                _ => (),
            }
        }
        if let Err(e) = sign_single_file(&p, &args, sigstore_mode) {
            eprintln!("{} Signing failed for {}: {}", "!".yellow(), p.display(), e);
        }
    }
    Ok(())
}

// Persist TSA history into the global config file (~/.kam/config.toml)
fn update_tsa_history_in_config(history_map: &StdHashMap<String, bool>) -> Result<(), KamError> {
    let home = dirs::home_dir()
        .ok_or_else(|| KamError::CommandFailed("Cannot determine home directory".to_string()))?;
    let cfg_dir = home.join(".kam");
    if !cfg_dir.exists() {
        std::fs::create_dir_all(&cfg_dir).map_err(KamError::Io)?;
    }
    let cfg_path = cfg_dir.join("config.toml");
    // Load existing config or create a new table
    let mut root_val = if cfg_path.exists() {
        let s = std::fs::read_to_string(&cfg_path).map_err(KamError::Io)?;
        toml::from_str::<toml::Value>(&s)
            .map_err(|e| KamError::CommandFailed(format!("Failed parse config.toml: {}", e)))?
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
    let out = toml::to_string_pretty(&root_val)
        .map_err(|e| KamError::CommandFailed(format!("Failed to serialize config: {}", e)))?;
    std::fs::write(&cfg_path, out).map_err(KamError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
    use openssl::asn1::Asn1Time;
    use openssl::hash::MessageDigest;
    use openssl::pkey::PKey;
    use openssl::rsa::Rsa;
    use openssl::x509::X509;
    use openssl::x509::{X509Builder, X509NameBuilder};
    use serde_json::json;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    // Helper to construct a small self-signed certificate, returning (PEM string, DER bytes)
    fn make_cert(cn: &str) -> (String, Vec<u8>) {
        let rsa = Rsa::generate(2048).unwrap();
        let pkey = PKey::from_rsa(rsa).unwrap();
        let mut name_b = X509NameBuilder::new().unwrap();
        name_b.append_entry_by_text("CN", cn).unwrap();
        let name = name_b.build();
        let mut builder = X509Builder::new().unwrap();
        builder.set_subject_name(&name).unwrap();
        builder.set_issuer_name(&name).unwrap();
        builder.set_pubkey(&pkey).unwrap();
        let not_before = Asn1Time::days_from_now(0).unwrap();
        let not_after = Asn1Time::days_from_now(365).unwrap();
        builder.set_not_before(&not_before).unwrap();
        builder.set_not_after(&not_after).unwrap();
        builder.sign(&pkey, MessageDigest::sha256()).unwrap();
        let cert = builder.build();
        let pem = cert.to_pem().unwrap();
        let der = cert.to_der().unwrap();
        (String::from_utf8_lossy(&pem).to_string(), der)
    }

    #[test]
    fn extract_cert_from_signed_certificate_pem() {
        let (pem_str, _) = make_cert("test-cert-pem");
        let v = json!({ "signedCertificate": pem_str });
        let der = extract_cert_der_from_fulcio_response(&v).expect("expected a DER result");
        let parsed = X509::from_der(&der).unwrap();
        let cn = parsed
            .subject_name()
            .entries()
            .next()
            .unwrap()
            .data()
            .as_utf8()
            .unwrap()
            .to_string();
        assert_eq!(cn, "test-cert-pem");
    }

    #[test]
    fn extract_cert_from_signed_certificate_base64() {
        let (_, der_bytes) = make_cert("test-cert-b64");
        let b64 = BASE64_ENGINE.encode(&der_bytes);
        let v = json!({ "signedCertificate": b64 });
        let der = extract_cert_der_from_fulcio_response(&v).expect("expected a DER result");
        let parsed = X509::from_der(&der).unwrap();
        let cn = parsed
            .subject_name()
            .entries()
            .next()
            .unwrap()
            .data()
            .as_utf8()
            .unwrap()
            .to_string();
        assert_eq!(cn, "test-cert-b64");
    }

    #[test]
    fn extract_cert_from_certchain_array_pem() {
        let (pem_str, _) = make_cert("test-cert-chain-pem");
        let v = json!({ "certChain": [ pem_str ] });
        let der = extract_cert_der_from_fulcio_response(&v).expect("expected a DER result");
        let parsed = X509::from_der(&der).unwrap();
        let cn = parsed
            .subject_name()
            .entries()
            .next()
            .unwrap()
            .data()
            .as_utf8()
            .unwrap()
            .to_string();
        assert_eq!(cn, "test-cert-chain-pem");
    }

    #[test]
    fn extract_cert_from_certchain_array_base64() {
        let (_, der_bytes) = make_cert("test-cert-chain-b64");
        let b64 = BASE64_ENGINE.encode(&der_bytes);
        let v = json!({ "certChain": [ b64 ] });
        let der = extract_cert_der_from_fulcio_response(&v).expect("expected a DER result");
        let parsed = X509::from_der(&der).unwrap();
        let cn = parsed
            .subject_name()
            .entries()
            .next()
            .unwrap()
            .data()
            .as_utf8()
            .unwrap()
            .to_string();
        assert_eq!(cn, "test-cert-chain-b64");
    }

    #[test]
    fn extract_cert_from_certchain_concatenated_pem() {
        let (pem_str, _) = make_cert("test-cert-concat-pem");
        let combined = format!("{}{}", pem_str, pem_str);
        let v = json!({ "certChain": combined });
        let der = extract_cert_der_from_fulcio_response(&v).expect("expected a DER result");
        let parsed = X509::from_der(&der).unwrap();
        let cn = parsed
            .subject_name()
            .entries()
            .next()
            .unwrap()
            .data()
            .as_utf8()
            .unwrap()
            .to_string();
        assert_eq!(cn, "test-cert-concat-pem");
    }

    #[test]
    fn extract_cert_from_alternate_fields() {
        let (pem_str, _) = make_cert("test-cert-alt");
        let v = json!({ "certificate": pem_str.clone(), "certificateChain": pem_str });
        let der = extract_cert_der_from_fulcio_response(&v).expect("expected a DER result");
        let parsed = X509::from_der(&der).unwrap();
        let cn = parsed
            .subject_name()
            .entries()
            .next()
            .unwrap()
            .data()
            .as_utf8()
            .unwrap()
            .to_string();
        assert_eq!(cn, "test-cert-alt");
    }

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
            fulcio: false,
            fulcio_url: "https://fulcio.sigstore.dev".to_string(),
            oidc_token_env: "SIGSTORE_ID_TOKEN".to_string(),
            oidc_token: None,
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
            fulcio: false,
            fulcio_url: "https://fulcio.sigstore.dev".to_string(),
            oidc_token_env: "SIGSTORE_ID_TOKEN".to_string(),
            oidc_token: None,
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
