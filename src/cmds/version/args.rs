use clap::Args;

/// Arguments for the version command
#[derive(Args, Debug)]
pub struct VersionArgs {
    /// The new version (e.g. 1.0.1) or bump type (major, minor, patch)
    #[arg(value_name = "VERSION")]
    pub version: Option<String>,
}
