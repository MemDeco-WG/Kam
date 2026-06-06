use super::*;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::sections::build::BuildSection;
use tempfile::tempdir;

fn quiet_args() -> BuildArgs {
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
        trim_shell: false,
        trim_shell_functions: false,
        obfuscate_shell: false,
    }
}

fn base_kam_toml() -> KamToml {
    let mut kam_toml = KamToml::default();
    kam_toml.prop.id = "example".to_string();
    kam_toml.prop.version = "v1.0.0".to_string();
    kam_toml.prop.versionCode = 1;
    kam_toml.prop.description = "Example".to_string();
    kam_toml.kam.build = Some(BuildSection {
        source_dir: Some("src/example".to_string()),
        ..BuildSection::default()
    });
    kam_toml
}

#[test]
fn module_zip_respects_source_dir_kamignore() {
    let temp = tempdir().unwrap();
    let project = temp.path();
    let src = project.join("src/example");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("keep.sh"), "echo keep\n").unwrap();
    std::fs::write(src.join("debug.log"), "debug\n").unwrap();
    std::fs::write(src.join("important.log"), "important\n").unwrap();
    std::fs::write(src.join(".kamignore"), "*.log\n!important.log\n").unwrap();

    let kam_toml = base_kam_toml();
    let out = project.join("dist");
    std::fs::create_dir_all(&out).unwrap();

    let zip_path = create_kam_module_zip(
        &kam_toml,
        &out,
        "example",
        "example",
        project,
        &quiet_args(),
    )
    .unwrap();
    let names = zip_names(&zip_path);

    assert!(names.iter().any(|name| name == "keep.sh"));
    assert!(names.iter().any(|name| name == "important.log"));
    assert!(!names.iter().any(|name| name == "debug.log"));
}

#[test]
fn module_zip_can_trim_and_obfuscate_shell_payload() {
    let temp = tempdir().unwrap();
    let project = temp.path();
    let src = project.join("src/example");
    std::fs::create_dir_all(src.join("lib/kamfw")).unwrap();
    std::fs::write(
        src.join("service.sh"),
        ". \"$MODPATH/lib/kamfw/base.sh\"\nmain\n",
    )
    .unwrap();
    std::fs::write(
        src.join("lib/kamfw/base.sh"),
        "main() { _helper; }\n_helper() { _value=1; echo $_value; }\n_unused() { echo no; }\n",
    )
    .unwrap();
    std::fs::write(src.join("lib/kamfw/unused.sh"), "_unused_file() { :; }\n").unwrap();

    let kam_toml = base_kam_toml();
    let out = project.join("dist");
    std::fs::create_dir_all(&out).unwrap();
    let mut args = quiet_args();
    args.trim_shell = true;
    args.trim_shell_functions = true;
    args.obfuscate_shell = true;

    let zip_path =
        create_kam_module_zip(&kam_toml, &out, "example", "example", project, &args).unwrap();
    let names = zip_names(&zip_path);

    assert!(names.iter().any(|name| name == "service.sh"));
    assert!(names.iter().any(|name| name == "lib/kamfw/base.sh"));
    assert!(!names.iter().any(|name| name == "lib/kamfw/unused.sh"));

    let base = zip_text(&zip_path, "lib/kamfw/base.sh");
    assert!(base.contains("KAM-AUDIT-BACKUP-BEGIN"));
    assert!(base.contains("-----BEGIN PGP MESSAGE-----"));
    assert!(!base.contains("_unused()"));
    assert!(!base.contains("_helper"));
    assert!(!base.contains("_value"));
}

#[test]
fn module_zip_trim_functions_config_implies_shell_trim() {
    let temp = tempdir().unwrap();
    let project = temp.path();
    let src = project.join("src/example");
    std::fs::create_dir_all(src.join("lib")).unwrap();
    std::fs::write(src.join("service.sh"), ". \"$MODPATH/lib/app.sh\"\nmain\n").unwrap();
    std::fs::write(src.join("lib/app.sh"), "main() { :; }\nunused() { :; }\n").unwrap();
    std::fs::write(src.join("unused.sh"), "unused_file() { :; }\n").unwrap();

    let mut kam_toml = base_kam_toml();
    kam_toml.kam.build.as_mut().unwrap().trim_shell_functions = Some(true);
    let out = project.join("dist");
    std::fs::create_dir_all(&out).unwrap();

    let zip_path = create_kam_module_zip(
        &kam_toml,
        &out,
        "example",
        "example",
        project,
        &quiet_args(),
    )
    .unwrap();
    let names = zip_names(&zip_path);

    assert!(names.iter().any(|name| name == "lib/app.sh"));
    assert!(!names.iter().any(|name| name == "unused.sh"));

    let base = zip_text(&zip_path, "lib/app.sh");
    assert!(base.contains("main()"));
    assert!(!base.contains("unused()"));
}

fn zip_names(path: &Path) -> Vec<String> {
    let file = File::open(path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    (0..archive.len())
        .map(|idx| archive.by_index(idx).unwrap().name().to_string())
        .collect()
}

fn zip_text(path: &Path, name: &str) -> String {
    let file = File::open(path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}
