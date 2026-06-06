use super::*;
use std::process::Command;

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

#[test]
fn obfuscated_script_refuses_tampered_audit_backup_parts() {
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

    assert_tampered_script_exits_125(&out, "no-pgp.sh", |script| {
        script
            .lines()
            .filter(|line| !line.contains("-----BEGIN PGP MESSAGE-----"))
            .map(|line| format!("{line}\n"))
            .collect()
    });
    assert_tampered_script_exits_125(&out, "no-end.sh", |script| {
        script
            .lines()
            .filter(|line| !line.starts_with("# KAM-AUDIT-BACKUP-END"))
            .map(|line| format!("{line}\n"))
            .collect()
    });
    assert_tampered_script_exits_125(&out, "fake-markers.sh", |script| {
        format!(
            "#!/bin/sh\n# KAM-AUDIT-BACKUP-BEGIN v1\n# KAM-AUDIT-BACKUP-END\n{}",
            remove_audit_backup_block(script).trim_start_matches("#!/bin/sh\n")
        )
    });
}

#[test]
fn obfuscation_keeps_system_shell_shebang_first() {
    if !gpg_available_for_test() {
        return;
    }
    let file = PackageFile {
        rel_path: "post-fs-data.sh".to_string(),
        content: b"#!/system/bin/sh\nmain() { :; }\nmain\n".to_vec(),
    };
    let mut opt = ShellPackageOptimizer::new(&args(false, true), "demo");
    opt.prepare(std::slice::from_ref(&file)).unwrap();
    let out = String::from_utf8(opt.transform_file(&file).unwrap()).unwrap();
    assert!(out.starts_with("#!/system/bin/sh\n# KAM-AUDIT-BACKUP-BEGIN"));
}
