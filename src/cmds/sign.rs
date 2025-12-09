use crate::cmds::secret::read_secret_plaintext;
use crate::errors::KamError;
use clap::Args;
use colored::*;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::sign::Signer;
use std::fs;
use std::path::Path;

#[derive(Args, Debug)]
pub struct SignArgs {
    /// The artifact to sign (zip)
    pub src: String,

    /// Name of the secret in kam keyring that holds the private key
    #[arg(long, default_value = "main")]
    pub secret: String,

    /// Output directory (default: dist)
    #[arg(long, default_value = "dist")]
    pub out: String,

    /// Certificate PEM chain path to include in signature metadata
    #[arg(long)]
    pub cert: Option<String>,
    /// Optional path to a private key PEM file to use instead of the keyring secret
    #[arg(long)]
    pub key_path: Option<String>,
}

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
    // Need to create EcdsaSig from digest and private key
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
    if let Some(cert_path) = args.cert {
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
    Ok(())
}
