use super::analysis::ScriptInfo;
use std::collections::BTreeSet;

const KAMFW_DEFAULT_IMPORTS: &[&str] = &["i18n", "base", "logging", "kam"];
const KAMFW_CORE_IMPORTS: &[&str] = &["__runtime__", "init_dirs", "magisk", "ksu", "ap"];
const RUNTIME_IMPORTS: &[&str] = &["__termux__", "__install_core__", "__uninstall__"];
const CUSTOMIZE_IMPORTS: &[&str] = &["__termux__", "__at_exit__", "__installer__"];
const INSTALLER_IMPORTS: &[&str] = &["__at_exit__", "__install_core__", "__installer_cmd__"];
const RICH_PARTS: &[&str] = &[
    "lib/kamfw/rich_rendering/layout.sh",
    "lib/kamfw/rich_rendering/interactive_prompts.sh",
    "lib/kamfw/rich_rendering/install_prompts.sh",
];

pub(super) fn semantic_sources(info: &ScriptInfo) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    match info.rel_path.as_str() {
        "lib/kamfw/.kamfwrc" => add_imports(&mut out, KAMFW_DEFAULT_IMPORTS),
        "lib/kamfw/kam.sh" => add_imports(&mut out, KAMFW_CORE_IMPORTS),
        "lib/kamfw/__runtime__.sh" => add_imports(&mut out, RUNTIME_IMPORTS),
        "lib/kamfw/__customize__.sh" => add_imports(&mut out, CUSTOMIZE_IMPORTS),
        "lib/kamfw/__installer__.sh" => add_imports(&mut out, INSTALLER_IMPORTS),
        "lib/kamfw/rich.sh" => out.extend(RICH_PARTS.iter().map(|path| (*path).to_string())),
        _ => {}
    }
    out
}

pub(super) fn should_keep_all_kamfw(info: &ScriptInfo) -> bool {
    info.dynamic_load && is_kamfw_context(&info.rel_path)
}

pub(super) fn semantic_function_roots(info: &ScriptInfo) -> BTreeSet<String> {
    if is_kamfw_shell_path(&info.rel_path) {
        return info.functions.keys().cloned().collect();
    }
    BTreeSet::new()
}

pub(super) fn is_kamfw_shell_path(path: &str) -> bool {
    path == "lib/kamfw/.kamfwrc" || (path.starts_with("lib/kamfw/") && path.ends_with(".sh"))
}

fn add_imports(out: &mut BTreeSet<String>, imports: &[&str]) {
    out.extend(imports.iter().map(|name| format!("lib/kamfw/{name}.sh")));
}

fn is_kamfw_context(path: &str) -> bool {
    path.starts_with("lib/kamfw/")
        || matches!(
            path,
            "customize.sh"
                | "post-fs-data.sh"
                | "service.sh"
                | "uninstall.sh"
                | "action.sh"
                | "boot-completed.sh"
                | "post-mount.sh"
        )
}
