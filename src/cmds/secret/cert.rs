use crate::errors::KamError;
use std::fs;
use std::path::PathBuf;

use x509_parser::prelude::*;

/// Get the directory for trusted Root CAs
fn trusted_cas_dir() -> Result<PathBuf, KamError> {
    let home = dirs::home_dir().ok_or_else(|| {
        KamError::InvalidDirectory("Could not determine home directory".to_string())
    })?;
    let dir = home.join(".kam").join("trusted-cas");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(KamError::Io)?;
    }
    Ok(dir)
}

/// Get the directory for cached certificates
fn certs_dir() -> Result<PathBuf, KamError> {
    let home = dirs::home_dir().ok_or_else(|| {
        KamError::InvalidDirectory("Could not determine home directory".to_string())
    })?;
    let dir = home.join(".kam").join("certs");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(KamError::Io)?;
    }
    Ok(dir)
}

/// Load all trusted Root CAs as PEM strings
pub fn load_trusted_cas() -> Result<Vec<String>, KamError> {
    let dir = trusted_cas_dir()?;
    let mut cas = Vec::new();

    for entry in fs::read_dir(&dir).map_err(KamError::Io)? {
        let entry = entry.map_err(KamError::Io)?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("pem") {
            let pem_data = fs::read_to_string(&path).map_err(KamError::Io)?;
            // Validate it's a valid certificate
            parse_x509_pem_certificate(&pem_data)?;
            cas.push(pem_data);
        }
    }

    Ok(cas)
}

/// Add a Root CA to the trust store
pub fn add_trusted_ca(pem: &str, name: &str) -> Result<(), KamError> {
    let dir = trusted_cas_dir()?;

    // Validate it's a valid certificate
    parse_x509_pem_certificate(pem)?;

    // Save to file
    let filename = format!("{}.pem", name);
    let path = dir.join(filename);
    fs::write(&path, pem).map_err(KamError::Io)?;

    Ok(())
}

/// List all trusted Root CAs with their fingerprints
pub fn list_trusted_cas() -> Result<Vec<(String, String)>, KamError> {
    let dir = trusted_cas_dir()?;
    let mut cas = Vec::new();

    for entry in fs::read_dir(&dir).map_err(KamError::Io)? {
        let entry = entry.map_err(KamError::Io)?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("pem") {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let pem_data = fs::read_to_string(&path).map_err(KamError::Io)?;
            let fingerprint = calculate_fingerprint_from_pem(&pem_data)?;

            cas.push((name, fingerprint));
        }
    }

    Ok(cas)
}

/// Remove a trusted Root CA by name
pub fn remove_trusted_ca(name: &str) -> Result<(), KamError> {
    let dir = trusted_cas_dir()?;
    let filename = format!("{}.pem", name);
    let path = dir.join(filename);

    if path.exists() {
        fs::remove_file(&path).map_err(KamError::Io)?;
        Ok(())
    } else {
        Err(KamError::CommandFailed(format!(
            "Trusted CA '{}' not found",
            name
        )))
    }
}

/// Load a cached certificate chain by name
pub fn load_cert_chain(name: &str) -> Result<String, KamError> {
    let dir = certs_dir()?;
    let filename = format!("{}.pem", name);
    let path = dir.join(filename);

    if !path.exists() {
        return Err(KamError::CommandFailed(format!(
            "Certificate '{}' not found",
            name
        )));
    }

    fs::read_to_string(&path).map_err(KamError::Io)
}

/// Store a certificate chain in the cache
pub fn store_cert_chain(name: &str, chain_pem: &str) -> Result<(), KamError> {
    let dir = certs_dir()?;

    // Validate the chain
    parse_x509_pem_chain(chain_pem)?;

    let filename = format!("{}.pem", name);
    let path = dir.join(filename);
    fs::write(&path, chain_pem).map_err(KamError::Io)?;

    Ok(())
}

/// Parse a single X.509 certificate from PEM (for validation)
fn parse_x509_pem_certificate(pem: &str) -> Result<(), KamError> {
    let (_, pem_cert) = parse_x509_pem(pem.as_bytes())
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse PEM: {}", e)))?;

    pem_cert.parse_x509().map_err(|e| {
        KamError::CommandFailed(format!("Failed to parse X.509 certificate: {}", e))
    })?;

    Ok(())
}

/// Parse a certificate chain from PEM (for validation)
fn parse_x509_pem_chain(pem: &str) -> Result<(), KamError> {
    let mut remaining = pem.as_bytes();
    let mut count = 0;

    loop {
        match parse_x509_pem(remaining) {
            Ok((rest, pem_cert)) => {
                pem_cert.parse_x509().map_err(|e| {
                    KamError::CommandFailed(format!("Failed to parse X.509 certificate: {}", e))
                })?;
                count += 1;

                if rest.is_empty() {
                    break;
                }
                remaining = rest;
            }
            Err(_) => break,
        }
    }

    if count == 0 {
        return Err(KamError::CommandFailed(
            "No valid certificates found in chain".to_string(),
        ));
    }

    Ok(())
}

/// Verify a certificate chain against trusted CAs and extract public key
pub fn verify_cert_chain(chain_pem: &str, trusted_cas_pem: &[String]) -> Result<String, KamError> {
    // Parse the chain
    let mut der_blobs: Vec<Vec<u8>> = Vec::new();
    let mut remaining = chain_pem.as_bytes();

    loop {
        match parse_x509_pem(remaining) {
            Ok((rest, pem_cert)) => {
                // Copy the DER contents into an owned buffer and keep it alive via Arc
                let der_vec = pem_cert.contents.to_vec();
                let der_arc = Arc::new(der_vec);
                // Store the Arc so the owned DER bytes outlive this scope
                der_blobs.push(der_arc);

                if rest.is_empty() {
                    break;
                }
                remaining = rest;
            }
            Err(_) => break,
        }
    }

    if der_blobs.is_empty() {
        return Err(KamError::CommandFailed(
            "Empty certificate chain".to_string(),
        ));
    }

    // Basic verification: check issuer-subject relationships
    for i in 0..der_blobs.len() - 1 {
        // parse the two certs on demand so their borrows are local to this iteration
        let (_, cert) = x509_parser::parse_x509_certificate(&der_blobs[i][..]).map_err(|e| {
            KamError::CommandFailed(format!("Failed to parse X.509 certificate: {}", e))
        })?;
        let (_, issuer_cert) =
            x509_parser::parse_x509_certificate(&der_blobs[i + 1][..]).map_err(|e| {
                KamError::CommandFailed(format!("Failed to parse X.509 certificate: {}", e))
            })?;

        if cert.issuer() != issuer_cert.subject() {
            return Err(KamError::CommandFailed(format!(
                "Certificate {} issuer does not match certificate {} subject",
                i,
                i + 1
            )));
        }
    }

    // Verify the root is trusted
    let (_, root_cert) = x509_parser::parse_x509_certificate(&der_blobs[der_blobs.len() - 1][..])
        .map_err(|e| {
        KamError::CommandFailed(format!("Failed to parse X.509 certificate: {}", e))
    })?;
    let mut trusted = false;

    for ca_pem in trusted_cas_pem {
        let (_, ca_pem_cert) = parse_x509_pem(ca_pem.as_bytes())
            .map_err(|e| KamError::CommandFailed(format!("Failed to parse trusted CA: {}", e)))?;
        let ca_cert = ca_pem_cert.parse_x509().map_err(|e| {
            KamError::CommandFailed(format!("Failed to parse trusted CA certificate: {}", e))
        })?;

        if root_cert.tbs_certificate.as_ref() == ca_cert.tbs_certificate.as_ref() {
            trusted = true;
            break;
        }
    }

    if !trusted {
        return Err(KamError::CommandFailed(
            "Certificate chain root is not trusted".to_string(),
        ));
    }

    // Extract public key from end-entity certificate (first in chain)
    let (_, end_entity) = x509_parser::parse_x509_certificate(&der_blobs[0][..]).map_err(|e| {
        KamError::CommandFailed(format!("Failed to parse X.509 certificate: {}", e))
    })?;
    extract_public_key_pem_from_cert(&end_entity)
}

/// Extract public key from certificate as PEM
fn extract_public_key_pem_from_cert(cert: &X509Certificate) -> Result<String, KamError> {
    let public_key = cert.public_key();
    let key_der = public_key.raw;

    // Convert DER to PEM using base64
    use base64::engine::Engine as _;
    use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;

    let b64 = BASE64_ENGINE.encode(key_der);
    let mut pem = String::from("-----BEGIN PUBLIC KEY-----\n");

    // Split into 64-character lines
    for chunk in b64.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(chunk).unwrap());
        pem.push('\n');
    }

    pem.push_str("-----END PUBLIC KEY-----\n");

    Ok(pem)
}

/// Calculate SHA-256 fingerprint from PEM
fn calculate_fingerprint_from_pem(pem: &str) -> Result<String, KamError> {
    use sha2::{Digest, Sha256};

    let (_, pem_cert) = parse_x509_pem(pem.as_bytes())
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse PEM: {}", e)))?;
    let cert = pem_cert
        .parse_x509()
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse certificate: {}", e)))?;

    let mut hasher = Sha256::new();
    hasher.update(cert.tbs_certificate.as_ref());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}
