use clap::Args;

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

    /// Generate sigstore DSSE bundle JSON in addition to signature
    #[arg(long, default_value_t = false)]
    pub sigstore: bool,
}
