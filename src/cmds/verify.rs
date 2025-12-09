use clap::Args;


#[derive(Args, Debug)]
pub struct VerifyArgs {
    /// Path to the artifact to verify (required for .sig verification)
    pub src: Option<String>,

    /// Path to signature file (base64 .sig). If omitted, defaults to <src>.sig
    #[arg(long)]
    pub sig: Option<String>,

    /// Path to .sigstore.json bundle containing DSSE envelope and certs
    #[arg(long)]
    pub bundle: Option<String>,

    /// Optional certificate PEM to use for verification (overrides bundle cert)
    #[arg(long)]
    pub cert: Option<String>,
    /// Optional root CA PEM to verify a certificate chain (trusted anchor)
    #[arg(long)]
    pub root: Option<String>,

    /// Name of secret in kam keyring that holds the private key; used to derive public key for verification
    #[arg(long, default_value = "main")]
    pub secret: String,
}

pub fn run(args: VerifyArgs) -> Result<(), crate::errors::KamError> {

    Ok(())
}

