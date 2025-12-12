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
}
