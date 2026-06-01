use crate::errors::KamError;
use crate::utils::Utils;
use openssl::ec::{EcGroup, EcKey};
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use std::fs::{self, OpenOptions};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;
use x509_parser::prelude::parse_x509_pem;

const DEVELOPERS_ISSUE_URL: &str = "https://github.com/KernelSU-Modules-Repo/developers/issues/new";

/// Reason accepted by the KernelSU developer revocation issue form.
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum KsuRevokeReason {
    /// Private key or certificate is compromised.
    Compromised,
    /// Private key or certificate is lost.
    Lost,
    /// Certificate has been superseded by a newer one.
    Superseded,
    /// Other reason.
    Other,
}

impl std::fmt::Display for KsuRevokeReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compromised => write!(f, "Compromised"),
            Self::Lost => write!(f, "Lost"),
            Self::Superseded => write!(f, "Superseded"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Result of generating a KernelSU developer key pair.
pub struct KsuGeneratedKey {
    /// Public key PEM path.
    pub public_key_path: PathBuf,
    /// Private key storage path. When gpg is used this points to `.pem.gpg`.
    pub private_key_path: PathBuf,
    /// Whether the private key was encrypted with gpg.
    pub used_gpg: bool,
}

/// Build the KernelSU developer public-key submission URL.
#[must_use]
pub fn submit_issue_url(username: &str, public_key_pem: &str) -> String {
    issue_url(&[
        ("template", "keyring.yml".to_string()),
        ("title", format!("[keyring] {username}")),
        ("username", username.to_string()),
        ("public_key", public_key_pem.to_string()),
    ])
}

/// Build the KernelSU developer certificate revocation URL.
#[must_use]
pub fn revoke_issue_url(
    username: &str,
    serial_number: &str,
    reason: &KsuRevokeReason,
    details: &str,
) -> String {
    issue_url(&[
        ("template", "revoke.yml".to_string()),
        ("title", format!("[revoke] {username}")),
        ("username", username.to_string()),
        ("serial_number", serial_number.to_string()),
        ("reason", reason.to_string()),
        ("details", details.to_string()),
    ])
}

/// Generate a P-256 key pair and write it to `out_dir`.
///
/// The public key is always written as PEM. The private key is encrypted with
/// `gpg --symmetric` when gpg is available, terminal interaction is possible,
/// and `no_gpg` is false. Otherwise the private key is written as a 0600 PEM.
///
/// # Errors
/// Returns `KamError` when key generation, file writing, or gpg execution fails.
pub fn generate_key_pair(
    name: &str,
    out_dir: &Path,
    no_gpg: bool,
    force: bool,
) -> Result<KsuGeneratedKey, KamError> {
    fs::create_dir_all(out_dir).map_err(KamError::Io)?;

    let pkey = generate_p256_key()?;
    let public_pem = pkey
        .public_key_to_pem()
        .map_err(|e| KamError::CommandFailed(format!("Failed to export public key: {e}")))?;
    let private_pem = pkey
        .private_key_to_pem_pkcs8()
        .map_err(|e| KamError::CommandFailed(format!("Failed to export private key: {e}")))?;

    let public_key_path = out_dir.join(format!("{name}.public.pem"));
    write_new_file(&public_key_path, &public_pem, force, false)?;

    let use_gpg = !no_gpg && gpg_available() && std::io::stdin().is_terminal();
    let private_key_path = if use_gpg {
        let encrypted_path = out_dir.join(format!("{name}.private.pem.gpg"));
        write_gpg_symmetric(&private_pem, &encrypted_path, force)?;
        encrypted_path
    } else {
        let plain_path = out_dir.join(format!("{name}.private.pem"));
        write_new_file(&plain_path, &private_pem, force, true)?;
        plain_path
    };

    Ok(KsuGeneratedKey {
        public_key_path,
        private_key_path,
        used_gpg: use_gpg,
    })
}

/// Extract a lowercase hexadecimal serial number from a PEM X.509 certificate.
///
/// # Errors
/// Returns `KamError` if the file cannot be read or parsed as a certificate.
pub fn serial_from_certificate(path: &Path) -> Result<String, KamError> {
    use std::fmt::Write as _;

    let pem = fs::read_to_string(path).map_err(KamError::Io)?;
    let (_, pem_cert) = parse_x509_pem(pem.as_bytes())
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse certificate PEM: {e}")))?;
    let cert = pem_cert
        .parse_x509()
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse X.509 certificate: {e}")))?;

    let mut serial = String::with_capacity(cert.raw_serial().len() * 2);
    for byte in cert.raw_serial() {
        write!(serial, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(serial)
}

/// Read public key PEM from a file.
///
/// # Errors
/// Returns `KamError` when the file cannot be read or does not look like a PEM public key.
pub fn read_public_key(path: &Path) -> Result<String, KamError> {
    let pem = fs::read_to_string(path).map_err(KamError::Io)?;
    if !pem.contains("-----BEGIN PUBLIC KEY-----") || !pem.contains("-----END PUBLIC KEY-----") {
        return Err(KamError::CommandFailed(format!(
            "{} does not look like a PEM public key",
            path.display()
        )));
    }
    Ok(pem)
}

/// Open a URL in the user's browser using the platform launcher.
///
/// # Errors
/// Returns `KamError` if no opener is available or the opener fails.
pub fn open_url(url: &str) -> Result<(), KamError> {
    let mut command = if cfg!(target_os = "macos") {
        let mut cmd = Command::new("open");
        cmd.arg(url);
        cmd
    } else if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        cmd
    } else {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(url);
        cmd
    };

    let status = command.status().map_err(KamError::Io)?;
    if status.success() {
        Ok(())
    } else {
        Err(KamError::CommandFailed(format!(
            "Failed to open URL with platform launcher: {status}"
        )))
    }
}

fn issue_url(params: &[(&str, String)]) -> String {
    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{DEVELOPERS_ISSUE_URL}?{query}")
}

fn generate_p256_key() -> Result<PKey<Private>, KamError> {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)
        .map_err(|e| KamError::CommandFailed(format!("Failed to create P-256 group: {e}")))?;
    let ec_key = EcKey::generate(&group)
        .map_err(|e| KamError::CommandFailed(format!("Failed to generate P-256 key: {e}")))?;
    PKey::from_ec_key(ec_key)
        .map_err(|e| KamError::CommandFailed(format!("Failed to build private key: {e}")))
}

fn gpg_available() -> bool {
    Command::new("gpg")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn write_gpg_symmetric(data: &[u8], encrypted_path: &Path, force: bool) -> Result<(), KamError> {
    if encrypted_path.exists() && !force {
        return Err(KamError::CommandFailed(format!(
            "{} already exists; pass --force to overwrite",
            encrypted_path.display()
        )));
    }

    let mut tmp = NamedTempFile::new().map_err(KamError::Io)?;
    tmp.write_all(data).map_err(KamError::Io)?;
    tmp.flush().map_err(KamError::Io)?;

    let status = Command::new("gpg")
        .arg("--symmetric")
        .arg("--cipher-algo")
        .arg("AES256")
        .arg("--yes")
        .arg("--output")
        .arg(encrypted_path)
        .arg(tmp.path())
        .status()
        .map_err(KamError::Io)?;

    if status.success() {
        Ok(())
    } else {
        Err(KamError::CommandFailed(format!(
            "gpg failed to encrypt private key: {status}"
        )))
    }
}

fn write_new_file(path: &Path, data: &[u8], force: bool, private: bool) -> Result<(), KamError> {
    if path.exists() && !force {
        return Err(KamError::CommandFailed(format!(
            "{} already exists; pass --force to overwrite",
            path.display()
        )));
    }

    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        if private {
            options.mode(0o600);
        } else {
            options.mode(0o644);
        }
    }

    let mut file = options.open(path).map_err(KamError::Io)?;
    file.write_all(data).map_err(KamError::Io)?;
    Ok(())
}

fn percent_encode(input: impl AsRef<str>) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::new();
    for byte in input.as_ref().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            b' ' => encoded.push('+'),
            _ => write!(encoded, "%{byte:02X}").expect("writing to String cannot fail"),
        }
    }
    encoded
}

/// Print the URL and optionally open it.
///
/// # Errors
/// Returns `KamError` if opening the URL fails.
pub fn emit_issue_url(url: &str, open: bool) -> Result<(), KamError> {
    println!("{url}");
    if open {
        Utils::executing("Opening GitHub issue form");
        open_url(url)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{KsuRevokeReason, percent_encode, revoke_issue_url, submit_issue_url};

    #[test]
    fn submit_issue_url_matches_kernel_su_issue_form_fields() {
        let url = submit_issue_url("octo", "-----BEGIN PUBLIC KEY-----\nabc\n");

        assert!(url.starts_with("https://github.com/KernelSU-Modules-Repo/developers/issues/new?"));
        assert!(url.contains("template=keyring.yml"));
        assert!(url.contains("title=%5Bkeyring%5D+octo"));
        assert!(url.contains("username=octo"));
        assert!(url.contains("public_key=-----BEGIN+PUBLIC+KEY-----%0Aabc%0A"));
    }

    #[test]
    fn revoke_issue_url_matches_kernel_su_issue_form_fields() {
        let url = revoke_issue_url("octo", "01ab", &KsuRevokeReason::Lost, "phone lost");

        assert!(url.contains("template=revoke.yml"));
        assert!(url.contains("title=%5Brevoke%5D+octo"));
        assert!(url.contains("username=octo"));
        assert!(url.contains("serial_number=01ab"));
        assert!(url.contains("reason=Lost"));
        assert!(url.contains("details=phone+lost"));
    }

    #[test]
    fn percent_encoding_uses_url_query_encoding() {
        assert_eq!(percent_encode("[keyring] a/b+c"), "%5Bkeyring%5D+a%2Fb%2Bc");
    }
}
