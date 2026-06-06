use super::*;

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
fn trim_functions_keeps_all_functions_in_dynamic_script() {
    let file = PackageFile {
        rel_path: "service.sh".to_string(),
        content: b"main() { eval \"$1\"; }\nused_later() { :; }\nmain \"$@\"\n".to_vec(),
    };
    let mut opt = ShellPackageOptimizer::new(&args(true, false), "demo");
    opt.prepare(std::slice::from_ref(&file)).unwrap();
    let out = String::from_utf8(opt.transform_file(&file).unwrap()).unwrap();
    assert!(out.contains("used_later()"));
}

#[test]
fn trim_functions_keeps_duplicate_function_names_across_sources() {
    let files = vec![
        PackageFile {
            rel_path: "service.sh".to_string(),
            content: b". \"$MODPATH/lib/a.sh\"\n. \"$MODPATH/lib/b.sh\"\nsame\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/a.sh".to_string(),
            content: b"same() { echo a; }\nonly_a() { :; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/b.sh".to_string(),
            content: b"same() { echo b; }\nonly_b() { :; }\n".to_vec(),
        },
    ];
    let mut opt = ShellPackageOptimizer::new(&args(true, false), "demo");
    opt.prepare(&files).unwrap();

    let a = String::from_utf8(opt.transform_file(&files[1]).unwrap()).unwrap();
    let b = String::from_utf8(opt.transform_file(&files[2]).unwrap()).unwrap();
    assert!(a.contains("same() { echo a; }"));
    assert!(b.contains("same() { echo b; }"));
    assert!(!a.contains("only_a()"));
    assert!(!b.contains("only_b()"));
}

#[test]
fn trim_functions_keeps_cross_file_called_chain() {
    let files = vec![
        PackageFile {
            rel_path: "service.sh".to_string(),
            content: b". \"$MODPATH/lib/a.sh\"\nmain\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/a.sh".to_string(),
            content: b"main() { helper; }\nhelper() { leaf; }\nleaf() { :; }\nunused() { :; }\n"
                .to_vec(),
        },
    ];
    let mut opt = ShellPackageOptimizer::new(&args(true, false), "demo");
    opt.prepare(&files).unwrap();
    let out = String::from_utf8(opt.transform_file(&files[1]).unwrap()).unwrap();
    assert!(out.contains("main()"));
    assert!(out.contains("helper()"));
    assert!(out.contains("leaf()"));
    assert!(!out.contains("unused()"));
}

#[test]
fn trim_import_keeps_kamfw_module() {
    let files = vec![
        PackageFile {
            rel_path: "service.sh".to_string(),
            content: b"import base\nmain\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/base.sh".to_string(),
            content: b"main() { echo ok; }\n".to_vec(),
        },
        PackageFile {
            rel_path: "lib/kamfw/other.sh".to_string(),
            content: b"other() { :; }\n".to_vec(),
        },
    ];
    let mut opt = ShellPackageOptimizer::new(&args(false, false), "demo");
    opt.prepare(&files).unwrap();
    assert!(opt.should_package("lib/kamfw/base.sh"));
    assert!(!opt.should_package("lib/kamfw/other.sh"));
}
