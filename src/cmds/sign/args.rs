use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct SignArgs {
    /// The artifact to sign (zip). If omitted, use --dist or --all to sign multiple files.
    pub src: Option<String>,

    /// Name of the secret in kam keyring that holds the private key
    #[arg(long, default_value = "main")]
    pub secret: String,

    /// Output directory (default: dist)
    #[arg(long, default_value = "dist")]
    pub out: String,

    /// Sign all artifacts in given directory (instead of specifying single src file)
    #[arg(long, value_name = "DIR")]
    pub dist: Option<PathBuf>,

    /// Sign all artifacts inside dist (alias of --dist <dir> with default dist)
    #[arg(long, default_value_t = false)]
    pub all: bool,

    /// Certificate PEM chain path to include in signature metadata
    #[arg(long)]
    pub cert: Option<String>,
    /// Optional path to a private key PEM file to use instead of the keyring secret
    #[arg(long)]
    pub key_path: Option<String>,

    /// Generate sigstore DSSE bundle JSON in addition to signature
    #[arg(long, default_value_t = false)]
    pub sigstore: bool,
    /// Contact a Timestamp Authority to get RFC3161 timestamp for the signed artifact.
    /// Disabled by default. Use `--timestamp` to enable requesting an RFC3161 timestamp (network required).
    #[arg(short, long, default_value_t = false)]
    pub timestamp: bool,
    /// Time Stamp Authority (TSA) URL to use for RFC3161 timestamps
    /// If omitted, uses global config `sign.tsa.url` or default Sigstore TSA.
    #[arg(long)]
    pub tsa_url: Option<String>,

    /// Request a Fulcio-issued certificate using OIDC and include it in the sigstore bundle.
    /// When enabled, kam will attempt to obtain a short-lived certificate from Fulcio using an OIDC token.
    #[arg(long, default_value_t = false)]
    pub fulcio: bool,

    /// Fulcio endpoint URL to use when requesting certificates (default: https://fulcio.sigstore.dev).
    #[arg(long, default_value = "https://fulcio.sigstore.dev")]
    pub fulcio_url: String,

    /// Name of environment variable that contains an OIDC token for Fulcio (used if --fulcio is enabled).
    /// Default points to a common token env var; the handler may check multiple variables as well.
    #[arg(long, default_value = "SIGSTORE_ID_TOKEN")]
    pub oidc_token_env: String,

    /// OIDC token provided directly via CLI (this takes precedence over the environment variable if set).
    #[arg(long)]
    pub oidc_token: Option<String>,
}
