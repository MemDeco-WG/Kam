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

        // Write bundle file
        super::sigstore::write_sigstore_bundle(
            out_dir,
            filename,
            &payload_data,
            &payload_sig,
            cert_der_opt.as_deref(),
        )?;
    }

    Ok(())
}
