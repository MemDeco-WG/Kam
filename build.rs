use regex::Regex;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Build-time checker for i18n coverage.
///
/// This script scans the Rust source for literal i18n keys used via:
/// - `tr_key("...")`
/// - `tr_fmt("...")`
/// - `trf!("...")`
///
/// For every dotted key (e.g. `repo.result_line_simple`) it verifies that the
/// key is present either:
/// - as a keyed literal in `src/i18n.rs` (the legacy keyed maps), or
/// - as a Fluent message id in compiled-in FTL files under `src/locales/<lang>/main.ftl`
///   (dotted key -> fluent id: dots & underscores replaced by `-`).
///
/// If any dotted keys are missing from both sources, this build script panics
/// which causes the compile to fail (fail-fast).
///
/// NOTE: Tests that intentionally exercise "missing" keys should NOT use
/// literal dotted key strings (they should generate keys at runtime) so that
/// this static checker doesn't trigger on test-only code.
fn main() {
    // Ensure Cargo reruns this script when translation sources change.
    println!("cargo:rerun-if-changed=src/i18n.rs");
    println!("cargo:rerun-if-changed=src/locales/en-US/main.ftl");
    println!("cargo:rerun-if-changed=src/locales/zh-CN/main.ftl");

    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set by Cargo"),
    );
    let src_root = manifest_dir.join("src");

    // Patterns to capture literal keys passed to i18n helpers/macros.
    // Note: keep these intentionally conservative (literal double-quoted strings).
    let re_tr_key = Regex::new(r#"tr_key\s*\(\s*"([^"]+)""#).unwrap();
    let re_tr_fmt = Regex::new(r#"tr_fmt\s*\(\s*"([^"]+)""#).unwrap();
    let re_trf = Regex::new(r#"trf!\s*\(\s*"([^"]+)""#).unwrap();
    let re_tr = Regex::new(r#"\btr\s*\(\s*"([^"]+)""#).unwrap();

    // Collect all literal keys found.
    let mut keys: HashSet<String> = HashSet::new();

    // Walk source tree and scan .rs files
    visit_rs_files(&src_root, &mut |path: &Path| {
        if let Ok(s) = fs::read_to_string(path) {
            for cap in re_tr_key.captures_iter(&s) {
                keys.insert(cap[1].to_string());
            }
            for cap in re_tr_fmt.captures_iter(&s) {
                keys.insert(cap[1].to_string());
            }
            for cap in re_trf.captures_iter(&s) {
                keys.insert(cap[1].to_string());
            }
            for cap in re_tr.captures_iter(&s) {
                keys.insert(cap[1].to_string());
            }
        }
    });

    // Only consider dotted keys as "keyed" i18n entries (the ones we want to enforce)
    let dotted_keys: Vec<String> = keys.into_iter().filter(|k| k.contains('.')).collect();

    if dotted_keys.is_empty() {
        // Nothing to check -> fast exit
        return;
    }

    // Load keyed map source (legacy keyed maps)
    let i18n_rs =
        fs::read_to_string(manifest_dir.join("src/i18n.rs")).unwrap_or_else(|_| String::new());

    // Load compiled-in FTL sources (if present)
    let en_ftl = fs::read_to_string(manifest_dir.join("src/locales/en-US/main.ftl"))
        .unwrap_or_else(|_| String::new());
    let zh_ftl = fs::read_to_string(manifest_dir.join("src/locales/zh-CN/main.ftl"))
        .unwrap_or_else(|_| String::new());

    let mut missing: Vec<String> = Vec::new();

    for key in dotted_keys.iter() {
        let mut found = false;

        // 1) keyed map check (look for `"key" =>` in src/i18n.rs)
        let keyed_pattern = format!(r#""{}"\s*=>"#, regex::escape(key));
        if Regex::new(&keyed_pattern).unwrap().is_match(&i18n_rs) {
            found = true;
        }

        // 2) FTL check: dotted_to_fluent_id (replace '.' and '_' with '-')
        if !found {
            let ftl_id = key.replace(&['.', '_'][..], "-");
            let ftl_re_pattern = format!(r"(?m)^\s*{}\s*=", regex::escape(&ftl_id));
            let ftl_re = Regex::new(&ftl_re_pattern).unwrap();

            if ftl_re.is_match(&en_ftl) || ftl_re.is_match(&zh_ftl) {
                found = true;
            }
        }

        if !found {
            missing.push(key.clone());
        }
    }

    if !missing.is_empty() {
        eprintln!();
        eprintln!("ERROR: Missing i18n keys detected (build will fail):");
        for k in &missing {
            eprintln!("  - {}", k);
        }
        eprintln!();
        eprintln!("Please add these keys either as keyed entries in `src/i18n.rs`");
        eprintln!(
            "or as Fluent messages in `src/locales/<lang>/main.ftl` (use id: dotted/underscore -> hyphen)"
        );
        eprintln!("Example (en-US):");
        eprintln!("  repo-result-line-simple = ...");
        eprintln!();
        // Fail the build explicitly.
        panic!(
            "Missing i18n keys found ({} missing). Aborting build to enforce i18n coverage.",
            missing.len()
        );
    }
}

/// Recursively visit all `.rs` files under `dir` and call `visit` on each file path.
fn visit_rs_files<F: FnMut(&Path)>(dir: &Path, visit: &mut F) {
    fn inner<F: FnMut(&Path)>(dir: &Path, visit: &mut F) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                inner(&path, visit);
            } else if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "rs" {
                        visit(&path);
                    }
                }
            }
        }
    }
    inner(dir, visit);
}
