use super::args::VerifyArgs;

use crate::errors::KamError;
use base64::engine::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use colored::*;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::sign::Verifier;
use std::fs;
use std::path::Path;

pub fn run(args: VerifyArgs) -> Result<(), KamError> {
    // 1. Determine Source and Signature Paths
    let src_str = args
        .src
        .as_ref()
        .ok_or_else(|| KamError::CommandFailed("Source file is required for verification".to_string()))?;
    let src_path = Path::new(src_str);

    if !src_path.exists() {
        return Err(KamError::CommandFailed(format!(
            "Source file not found: {}",
            src_path.display()
        )));
    }

    let sig_path = if let Some(s) = &args.sig {
        Path::new(s).to_path_buf()
    } else {
        Path::new(&format!("{}.sig", src_str)).to_path_buf()
    };

    if !sig_path.exists() {
        return Err(KamError::CommandFailed(format!(
            "Signature file not found: {}",
            sig_path.display()
        )));
    }

    // 2. Read Source and Signature
    let data = fs::read(src_path).map_err(KamError::Io)?;
    let sig_b64 = fs::read_to_string(&sig_path).map_err(KamError::Io)?;
    let sig_bytes = BASE64_ENGINE
        .decode(sig_b64.trim().as_bytes())
        .map_err(|e| KamError::CommandFailed(format!("Failed to base64 decode signature: {}", e)))?;

    // 3. Obtain Public Key
    // Priority: --key > --cert-chain > --cert-name > secret
    let pkey = if let Some(key_path_str) = &args.key {
        // Direct public key file
        let key_path = Path::new(key_path_str);
        let key_bytes = fs::read(key_path).map_err(KamError::Io)?;
        PKey::public_key_from_pem(&key_bytes)
            .map_err(|e| KamError::CommandFailed(format!("Failed to parse public key PEM from {}: {}", key_path.display(), e)))?
    } else if let Some(cert_chain_path) = &args.cert_chain {
        // Certificate chain from file
        if args.verbose {
            println!("Loading certificate chain from {}...", cert_chain_path);
        }
        let chain_pem = fs::read_to_string(cert_chain_path).map_err(KamError::Io)?;

        // Load trusted CAs
        let trusted_cas = crate::cmds::secret::cert::load_trusted_cas()?;
        if trusted_cas.is_empty() {
            return Err(KamError::CommandFailed(
                "No trusted Root CAs found. Add one with: kam secret trust --add-root <ca.pem> --ca-name <name>".to_string()
            ));
        }

        // Verify chain and extract public key
        if args.verbose {
            println!("Verifying certificate chain...");
        }
        let pub_key_pem = crate::cmds::secret::cert::verify_cert_chain(&chain_pem, &trusted_cas)?;

        if args.verbose {
            println!("Certificate chain verified successfully.");
        }

        // Parse public key PEM
        PKey::public_key_from_pem(pub_key_pem.as_bytes())
            .map_err(|e| KamError::CommandFailed(format!("Failed to parse public key from certificate: {}", e)))?
    } else if let Some(cert_name) = &args.cert_name {
        // Cached certificate
        if args.verbose {
            println!("Loading cached certificate '{}'...", cert_name);
        }
        let chain_pem = crate::cmds::secret::cert::load_cert_chain(cert_name)?;

        // Load trusted CAs
        let trusted_cas = crate::cmds::secret::cert::load_trusted_cas()?;
        if trusted_cas.is_empty() {
            return Err(KamError::CommandFailed(
                "No trusted Root CAs found. Add one with: kam secret trust --add-root <ca.pem> --ca-name <name>".to_string()
            ));
        }

        // Verify chain and extract public key
        if args.verbose {
            println!("Verifying certificate chain...");
        }
        let pub_key_pem = crate::cmds::secret::cert::verify_cert_chain(&chain_pem, &trusted_cas)?;

        if args.verbose {
            println!("Certificate chain verified successfully.");
        }

        // Parse public key PEM
        PKey::public_key_from_pem(pub_key_pem.as_bytes())
            .map_err(|e| KamError::CommandFailed(format!("Failed to parse public key from certificate: {}", e)))?
    } else {
        // Use helper to get/refresh public key from secret (handles caching and fallback)
        match crate::cmds::secret::utils::get_or_refresh_public_key(&args.secret, args.verbose) {
            Ok(pk) => pk,
            Err(e) => return Err(KamError::CommandFailed(format!("Failed to retrieve public key: {}", e))),
        }
    };

    if args.verbose {
        println!("Calculating hash for '{}'...", src_path.display());
    }

    // 4. Verify
    let mut verifier = Verifier::new(MessageDigest::sha256(), &pkey)
        .map_err(|e| KamError::CommandFailed(format!("Failed to create verifier: {}", e)))?;

    if args.verbose {
        println!("Verifying signature...");
    }
    verifier.update(&data).map_err(|e| KamError::CommandFailed(format!("Failed to update verifier: {}", e)))?;

    let result = verifier.verify(&sig_bytes).map_err(|e| KamError::CommandFailed(format!("Verification error: {}", e)))?;

    if result {
        if args.verbose {
            println!("{} Verification successful.", "✓".green());
        } else {
             println!("Verified");
        }
        Ok(())
    } else {
         Err(KamError::CommandFailed(format!(
            "Verification FAILED for '{}'",
            src_path.display()
        )))
    }
}
