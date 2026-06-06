use crate::errors::kam::KamError;

use sha2::{Digest, Sha256};
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

const AUDIT_PUBLIC_KEY: &str = include_str!("../../../../public.asc");
const AUDIT_KEY_FINGERPRINT: &str = "2135EC6E1EC54E54B96CF64DE78F1DC59509F8E9";

pub(super) fn obfuscation_audit_preamble(
    rel_path: &str,
    content: &str,
) -> Result<String, KamError> {
    let audit_backup = encrypt_audit_backup(rel_path, content)?;
    let audit_guard = audit_guard_script(rel_path);
    Ok(format!("{audit_backup}{audit_guard}"))
}

pub(super) fn insert_audit_preamble_after_shebang(script: &str, audit_preamble: &str) -> String {
    if let Some(rest) = script.strip_prefix("#!")
        && let Some(newline) = rest.find('\n')
    {
        let shebang_end = 2 + newline + 1;
        return format!(
            "{}{}{}",
            &script[..shebang_end],
            audit_preamble,
            &script[shebang_end..]
        );
    }
    format!("{audit_preamble}{script}")
}

fn encrypt_audit_backup(rel_path: &str, content: &str) -> Result<String, KamError> {
    if AUDIT_PUBLIC_KEY.trim().is_empty() {
        return Err(KamError::CommandFailed(
            "Shell obfuscation requires Kam's bundled audit public key".to_string(),
        ));
    }

    let gpg_home = tempfile::tempdir().map_err(KamError::Io)?;
    let import_status = Command::new("gpg")
        .arg("--batch")
        .arg("--homedir")
        .arg(gpg_home.path())
        .arg("--import")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(AUDIT_PUBLIC_KEY.as_bytes())?;
            }
            child.wait_with_output()
        })
        .map_err(KamError::Io)?;
    if !import_status.status.success() {
        return Err(KamError::CommandFailed(format!(
            "Failed to import bundled audit public key: {}",
            String::from_utf8_lossy(&import_status.stderr)
        )));
    }

    let mut source = NamedTempFile::new().map_err(KamError::Io)?;
    source.write_all(content.as_bytes()).map_err(KamError::Io)?;
    source.flush().map_err(KamError::Io)?;
    let encrypted = NamedTempFile::new().map_err(KamError::Io)?;
    let encrypt_status = Command::new("gpg")
        .arg("--batch")
        .arg("--yes")
        .arg("--trust-model")
        .arg("always")
        .arg("--homedir")
        .arg(gpg_home.path())
        .arg("--armor")
        .arg("--encrypt")
        .arg("--recipient")
        .arg(AUDIT_KEY_FINGERPRINT)
        .arg("--output")
        .arg(encrypted.path())
        .arg(source.path())
        .output()
        .map_err(KamError::Io)?;
    if !encrypt_status.status.success() {
        return Err(KamError::CommandFailed(format!(
            "Failed to encrypt shell audit backup for {rel_path}: {}",
            String::from_utf8_lossy(&encrypt_status.stderr)
        )));
    }

    let armored = std::fs::read_to_string(encrypted.path()).map_err(KamError::Io)?;
    Ok(format_audit_backup(rel_path, &armored))
}

fn format_audit_backup(rel_path: &str, armored: &str) -> String {
    let digest = digest_prefix("audit", rel_path, armored);
    let mut out = String::new();
    out.push_str("# KAM-AUDIT-BACKUP-BEGIN v1\n");
    out.push_str("# purpose: encrypted original shell code for code review\n");
    out.push_str(&format!("# path: {rel_path}\n"));
    out.push_str(&format!("# recipient: {AUDIT_KEY_FINGERPRINT}\n"));
    out.push_str(&format!("# sha256: {}\n", &digest[..64]));
    for line in armored.lines() {
        out.push_str("# ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("# KAM-AUDIT-BACKUP-END\n");
    out
}

fn audit_guard_script(rel_path: &str) -> String {
    let rel = shell_single_quote(rel_path);
    format!(
        r#"__kam_audit_backup_required() {{
__kam_audit_rel={rel}
__kam_audit_file=
__kam_audit_begin='KAM-AUDIT-BACKUP-BEGIN'' v1'
__kam_audit_pgp='-----BEGIN ''PGP MESSAGE-----'
__kam_audit_end='KAM-AUDIT-BACKUP-''END'
case "$__kam_audit_rel" in
*/*) [ -n "${{MODDIR:-}}" ] && __kam_audit_file="$MODDIR/$__kam_audit_rel" ;;
esac
[ -n "$__kam_audit_file" ] || __kam_audit_file="${{0:-}}"
if [ -r "$__kam_audit_file" ] &&
    grep -q "$__kam_audit_begin" "$__kam_audit_file" 2>/dev/null &&
    grep -q -- "$__kam_audit_pgp" "$__kam_audit_file" 2>/dev/null &&
    grep -q "$__kam_audit_end" "$__kam_audit_file" 2>/dev/null; then
    unset __kam_audit_rel __kam_audit_file __kam_audit_begin __kam_audit_pgp __kam_audit_end
    return 0
fi
printf '%s\n' 'Kam audit backup missing; refusing to run obfuscated shell.' >&2
unset __kam_audit_rel __kam_audit_file __kam_audit_begin __kam_audit_pgp __kam_audit_end
return 125 2>/dev/null || exit 125
}}
__kam_audit_backup_required
__kam_audit_status=$?
if [ "$__kam_audit_status" -ne 0 ]; then
    (return "$__kam_audit_status") 2>/dev/null && return "$__kam_audit_status"
    exit "$__kam_audit_status"
fi
unset __kam_audit_status
unset -f __kam_audit_backup_required 2>/dev/null || true
"#
    )
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn digest_prefix(module_id: &str, rel_path: &str, content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(module_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(rel_path.as_bytes());
    hasher.update(b"\0");
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}
