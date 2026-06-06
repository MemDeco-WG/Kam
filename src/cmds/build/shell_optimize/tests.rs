use super::*;
use std::process::Command;

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

#[test]
fn trim_drops_unreferenced_shell_file() {
    let files = vec![
        PackageFile {
            rel_path: "service.sh".to_string(),
            content: b". \"$MODPATH/lib/kamfw/base.sh\"\nrun_main\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/base.sh".to_string(),
            content: b"run_main() { echo ok; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/unused.sh".to_string(),
            content: b"unused() { echo no; }\n".to_vec(),
        },
    ];
    let mut opt = ShellPackageOptimizer::new(&args(false, false), "demo");
    opt.prepare(&files).unwrap();
    assert!(opt.should_package("service.sh"));
    assert!(opt.should_package("lib/kamfw/base.sh"));
    assert!(!opt.should_package("lib/kamfw/unused.sh"));
}

#[test]
fn trim_functions_keeps_called_chain() {
    let file = PackageFile {
        rel_path: "service.sh".to_string(),
        content: b"main() { used; }\nused() { leaf; }\nleaf() { :; }\nunused() { :; }\nmain\n"
            .to_vec(),
    };
    let mut opt = ShellPackageOptimizer::new(&args(true, false), "demo");
    opt.prepare(std::slice::from_ref(&file)).unwrap();
    let out = String::from_utf8(opt.transform_file(&file).unwrap()).unwrap();
    assert!(out.contains("used()"));
    assert!(out.contains("leaf()"));
    assert!(!out.contains("unused()"));
}

#[test]
fn obfuscation_renames_private_symbols() {
    if !gpg_available_for_test() {
        return;
    }
    let file = PackageFile {
        rel_path: "service.sh".to_string(),
        content:
            b"#!/system/bin/sh\n_helper() { _value=1; echo $_value; }\nmain() { _helper; }\nmain\n"
                .to_vec(),
    };
    let mut opt = ShellPackageOptimizer::new(&args(false, true), "demo");
    opt.prepare(std::slice::from_ref(&file)).unwrap();
    let out = String::from_utf8(opt.transform_file(&file).unwrap()).unwrap();
    assert!(out.starts_with("#!/system/bin/sh\n# KAM-AUDIT-BACKUP-BEGIN"));
    assert!(out.contains("KAM-AUDIT-BACKUP-BEGIN"));
    assert!(out.contains("-----BEGIN PGP MESSAGE-----"));
    assert!(out.contains("__kam_audit_backup_required"));
    assert!(out.contains("refusing to run obfuscated shell"));
    assert!(!out.contains("_helper"));
    assert!(!out.contains("_value"));
    assert!(out.contains("main"));
}

#[test]
fn obfuscated_script_refuses_to_run_without_audit_backup() {
    if !gpg_available_for_test() {
        return;
    }
    let file = PackageFile {
        rel_path: "service.sh".to_string(),
        content: b"#!/bin/sh\n_private() { :; }\nmain() { _private; }\nmain\n".to_vec(),
    };
    let mut opt = ShellPackageOptimizer::new(&args(false, true), "demo");
    opt.prepare(std::slice::from_ref(&file)).unwrap();
    let out = String::from_utf8(opt.transform_file(&file).unwrap()).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let good = temp.path().join("good.sh");
    let bad = temp.path().join("bad.sh");
    std::fs::write(&good, &out).unwrap();
    std::fs::write(&bad, remove_audit_backup_block(&out)).unwrap();

    let good_status = Command::new("sh").arg(&good).status().unwrap();
    assert!(good_status.success());

    let bad_output = Command::new("sh").arg(&bad).output().unwrap();
    assert_eq!(bad_output.status.code(), Some(125));
    assert!(
        String::from_utf8_lossy(&bad_output.stderr)
            .contains("Kam audit backup missing; refusing to run obfuscated shell.")
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
