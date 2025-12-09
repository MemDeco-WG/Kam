use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct SecretArgs {
    #[command(subcommand)]
    pub command: SecretCommands,
}

#[derive(Subcommand, Debug)]
pub enum SecretCommands {
    /// List saved secrets
    List,

    /// Add a secret from a value or file
    Add {
        /// Name of the secret
        name: String,

        /// Path to a file to read the secret from
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Also accept path as a second positional parameter so users can run `kam secret add name path`
        #[arg(value_name = "FILE")]
        file_path: Option<PathBuf>,

        /// Provide value directly
        #[arg(short, long)]
        value: Option<String>,

        /// Force storing to local file instead of system keyring
        #[arg(long, default_value_t = false)]
        force_file: bool,
        /// Pass the password on the CLI (not recommended); password will be prompted if not set
        #[arg(long)]
        password: Option<String>,
        /// Also create a local fallback file under ~/.kam/secrets
        #[arg(long, default_value_t = false)]
        with_backup: bool,
    },

    /// Get a secret and print it to stdout (or --out file)
    Get {
        /// Name of the secret
        name: String,

        /// Write to file instead of stdout
        #[arg(short, long)]
        out: Option<PathBuf>,

        /// Pass the password on the CLI (not recommended). If not provided, will ask interactively
        #[arg(long)]
        password: Option<String>,
    },

    /// Remove a secret
    Remove { name: String },
    /// Export secret to a file (by default decrypted). Use --encrypted to export encrypted blob.
    Export {
        name: String,
        path: PathBuf,
        #[arg(long, default_value_t = false)]
        encrypted: bool,
    },

    /// Import secret from a file. If file is an encrypted KAM blob, it will be stored as-is.
    Import {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
}
