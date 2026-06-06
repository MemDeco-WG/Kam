use super::*;

#[test]
fn trim_kamfwrc_keeps_default_import_graph() {
    let files = vec![
        PackageFile {
            rel_path: "service.sh".to_string(),
            content: b". \"$MODDIR/lib/kamfw/.kamfwrc\"\nkamfw load rich\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/.kamfwrc".to_string(),
            content: b"import i18n\nimport base\nimport logging\nimport kam\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/i18n.sh".to_string(),
            content: b"i18n() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/base.sh".to_string(),
            content: b"base() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/logging.sh".to_string(),
            content: b"info() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/kam.sh".to_string(),
            content: b"import __runtime__\nimport init_dirs\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/__runtime__.sh".to_string(),
            content: b"import __termux__\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/init_dirs.sh".to_string(),
            content: b"kam_init_dirs() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/__termux__.sh".to_string(),
            content: b"active_termux_env() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/rich.sh".to_string(),
            content: b". \"${KAMFW_DIR}/rich_rendering/layout.sh\"\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/rich_rendering/layout.sh".to_string(),
            content: b"panel() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/unused.sh".to_string(),
            content: b"unused() { :; }\n".to_vec(),
        },
    ];
    let mut opt = ShellPackageOptimizer::new(&args(false, false), "demo");
    opt.prepare(&files).unwrap();

    for path in [
        "lib/kamfw/.kamfwrc",
        "lib/kamfw/i18n.sh",
        "lib/kamfw/base.sh",
        "lib/kamfw/logging.sh",
        "lib/kamfw/kam.sh",
        "lib/kamfw/__runtime__.sh",
        "lib/kamfw/init_dirs.sh",
        "lib/kamfw/__termux__.sh",
        "lib/kamfw/rich.sh",
        "lib/kamfw/rich_rendering/layout.sh",
    ] {
        assert!(opt.should_package(path), "{path}");
    }
    assert!(!opt.should_package("lib/kamfw/unused.sh"));
}

#[test]
fn trim_kamfwrc_loader_definition_does_not_force_whole_kamfw_library() {
    let files = vec![
        PackageFile {
            rel_path: "service.sh".to_string(),
            content: b". \"$MODDIR/lib/kamfw/.kamfwrc\"\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/.kamfwrc".to_string(),
            content: b"for _kamfw_rcfile in \"$MODDIR/.config/kamfw/env.d/\"*.envrc; do\n. \"$_kamfw_rcfile\"\ndone\nimport() { _kamfw_load \"$@\"; }\nimport base\n"
                .to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/base.sh".to_string(),
            content: b"base() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/watchdog.sh".to_string(),
            content: b"watchdog() { :; }\n".to_vec(),
        },
    ];
    let mut opt = ShellPackageOptimizer::new(&args(false, false), "demo");
    opt.prepare(&files).unwrap();

    assert!(opt.should_package("lib/kamfw/.kamfwrc"));
    assert!(opt.should_package("lib/kamfw/base.sh"));
    assert!(!opt.should_package("lib/kamfw/watchdog.sh"));
}

#[test]
fn trim_kamfw_runtime_semantics_keeps_phase_route_imports() {
    let files = vec![
        PackageFile {
            rel_path: "post-mount.sh".to_string(),
            content: b". \"$MODDIR/lib/kamfw/.kamfwrc\"\nkamfw run post-mount -- \"$@\"\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/.kamfwrc".to_string(),
            content: b"import kam\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/kam.sh".to_string(),
            content: b"import __runtime__\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/__runtime__.sh".to_string(),
            content: b"import __termux__\nkamfw_phase_post_mount() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/__termux__.sh".to_string(),
            content: b"active_termux_env() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/__install_core__.sh".to_string(),
            content: b"install_core() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/__uninstall__.sh".to_string(),
            content: b"uninstall_core() { :; }\n".to_vec(),
        },
    ];
    let mut opt = ShellPackageOptimizer::new(&args(false, false), "demo");
    opt.prepare(&files).unwrap();

    assert!(opt.should_package("lib/kamfw/__runtime__.sh"));
    assert!(opt.should_package("lib/kamfw/__termux__.sh"));
    assert!(opt.should_package("lib/kamfw/__install_core__.sh"));
    assert!(opt.should_package("lib/kamfw/__uninstall__.sh"));
}

#[test]
fn trim_dynamic_kamfw_load_keeps_kamfw_shell_library() {
    let files = vec![
        PackageFile {
            rel_path: "service.sh".to_string(),
            content: b". \"$MODDIR/lib/kamfw/.kamfwrc\"\nimport \"$feature\"\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/.kamfwrc".to_string(),
            content: b"import base\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/base.sh".to_string(),
            content: b"base() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/watchdog.sh".to_string(),
            content: b"watchdog() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "extra.sh".to_string(),
            content: b"extra() { :; }\n".to_vec(),
        },
    ];
    let mut opt = ShellPackageOptimizer::new(&args(false, false), "demo");
    opt.prepare(&files).unwrap();

    assert!(opt.should_package("lib/kamfw/base.sh"));
    assert!(opt.should_package("lib/kamfw/watchdog.sh"));
    assert!(!opt.should_package("extra.sh"));
}

#[test]
fn trim_external_dynamic_source_does_not_force_kamfw_library() {
    let files = vec![
        PackageFile {
            rel_path: "service.sh".to_string(),
            content: b". \"$MODDIR/lib/kamfw/.kamfwrc\"\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/.kamfwrc".to_string(),
            content: b"import __termux__\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/__termux__.sh".to_string(),
            content: b"_env_file=/data/data/com.termux/files/usr/etc/termux/termux.env\n. \"$_env_file\"\n"
                .to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/watchdog.sh".to_string(),
            content: b"watchdog() { :; }\n".to_vec(),
        },
    ];
    let mut opt = ShellPackageOptimizer::new(&args(false, false), "demo");
    opt.prepare(&files).unwrap();

    assert!(opt.should_package("lib/kamfw/__termux__.sh"));
    assert!(!opt.should_package("lib/kamfw/watchdog.sh"));
}

#[test]
fn trim_functions_keeps_kamfw_module_api_functions() {
    let files = vec![
        PackageFile {
            rel_path: "service.sh".to_string(),
            content: b". \"$MODDIR/lib/kamfw/.kamfwrc\"\ninfo ready\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/.kamfwrc".to_string(),
            content: b"import logging\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/logging.sh".to_string(),
            content: b"info() { log_emit info \"$@\"; }\ndebug() { log_emit debug \"$@\"; }\nlog_emit() { :; }\n"
                .to_vec(),
        },
    ];
    let mut opt = ShellPackageOptimizer::new(&args(true, false), "demo");
    opt.prepare(&files).unwrap();
    let out = String::from_utf8(opt.transform_file(&files[2]).unwrap()).unwrap();
    assert!(out.contains("info()"));
    assert!(out.contains("debug()"));
    assert!(out.contains("log_emit()"));
}
