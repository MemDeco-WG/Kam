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

    /// Path to public key PEM for verification (overrides derived key from secret)
    #[arg(long)]
    pub key: Option<String>,

    /// Name of cached developer certificate to use for verification
    #[arg(long, conflicts_with = "key")]
    pub cert_name: Option<String>,

    /// Path to certificate chain PEM file for verification
    #[arg(long, conflicts_with_all = ["key", "cert_name"])]
    pub cert_chain: Option<String>,

    /// Skip CRL (Certificate Revocation List) check
    #[arg(long)]
    pub skip_crl: bool,

    /// Verbose output showing verification steps
    #[arg(short, long)]
    pub verbose: bool,
}
