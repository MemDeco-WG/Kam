use super::*;
use std::process::Command;

mod audit;
mod basic;
mod kamfw;

fn args(trim_functions: bool, obfuscate: bool) -> BuildArgs {
    BuildArgs {
        path: ".".to_string(),
        all: false,
        output: None,
        bump: false,
        release: false,
        sign: false,
        interactive: false,
        pre_release: false,
        quiet: true,
        jobs: None,
        trim_shell: true,
        trim_shell_functions: trim_functions,
        obfuscate_shell: obfuscate,
    }
}

fn assert_tampered_script_exits_125(
    original: &str,
    name: &str,
    tamper: impl FnOnce(&str) -> String,
) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(name);
    std::fs::write(&path, tamper(original)).unwrap();
    let output = Command::new("sh").arg(&path).output().unwrap();
    assert_eq!(output.status.code(), Some(125), "{name}");
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Kam audit backup missing; refusing to run obfuscated shell."),
        "{name}"
    );
}

fn remove_audit_backup_block(script: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in script.lines() {
        if line.starts_with("# KAM-AUDIT-BACKUP-BEGIN") {
            skipping = true;
            continue;
        }
        if line.starts_with("# KAM-AUDIT-BACKUP-END") {
            skipping = false;
            continue;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn gpg_available_for_test() -> bool {
    Command::new("gpg")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}
