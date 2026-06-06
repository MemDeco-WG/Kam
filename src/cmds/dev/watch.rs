use ignore::WalkBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::cmds::build::hooks::{run_dev_binary_hooks, run_dev_sync_hooks, run_dev_webui_hooks};
use crate::errors::KamError;
use crate::utils::Utils;

use super::adb::detect_device;
use super::args::DevArgs;
use super::context::DevContext;
use super::forward::{enable_mcp, run_forwards};
use super::logs::show_logs;
use super::session::should_show_logs;
use super::sync::{
    collect_hot_files, matches_hot_path, run_restart_command, sync_incremental_hot_files,
    sync_matching_hot_files,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct WatchPlan {
    changed: Vec<PathBuf>,
    webui: bool,
    binary: bool,
    hot_files: Vec<PathBuf>,
    structure: bool,
}

pub(super) fn watch(ctx: &DevContext, args: &DevArgs) -> Result<(), KamError> {
    Utils::section("Watching for Kam dev changes");
    let mut previous = snapshot(ctx)?;
    loop {
        thread::sleep(Duration::from_secs(2));
        let current = snapshot(ctx)?;
        if current != previous {
            let plan = plan_watch_changes(ctx, &previous, &current)?;
            report_watch_plan(ctx, &plan);
            run_incremental(ctx, args, &plan)?;
            previous = current;
        }
    }
}

fn plan_watch_changes(
    ctx: &DevContext,
    previous: &BTreeMap<PathBuf, SystemTime>,
    current: &BTreeMap<PathBuf, SystemTime>,
) -> Result<WatchPlan, KamError> {
    let changed = changed_files(previous, current);
    if changed.is_empty() {
        return Ok(WatchPlan::default());
    }

    let mut plan = WatchPlan {
        changed,
        ..WatchPlan::default()
    };

    for file in &plan.changed {
        let rel_project = rel_to_project(ctx, file);
        if rel_project.starts_with("webui/") {
            plan.webui = true;
        } else if rel_project.starts_with("crates/") {
            plan.binary = true;
        } else if is_module_rel(ctx, file, "webroot/") {
            plan.webui = true;
        } else if is_module_rel(ctx, file, ".local/bin/") {
            plan.binary = true;
        } else if matches_hot_path(ctx, file)? {
            plan.hot_files.push(file.clone());
        } else {
            plan.structure = true;
        }
    }

    plan.hot_files.sort();
    plan.hot_files.dedup();
    Ok(plan)
}

fn report_watch_plan(ctx: &DevContext, plan: &WatchPlan) {
    if plan.changed.is_empty() {
        Utils::info("Detected file changes; no actionable path changes found.");
        return;
    }
    for file in &plan.changed {
        Utils::info(format!("Changed: {}", rel_to_project(ctx, file)));
    }
    if plan.webui {
        Utils::info("Detected WebUI changes; running dev-webui hooks, then hot syncing webroot.");
    }
    if plan.binary {
        Utils::info(
            "Detected CLI/binary changes; running dev-binary hooks, then hot syncing .local/bin.",
        );
    }
    if !plan.hot_files.is_empty() {
        Utils::info("Detected hot-file changes; pushing matching allowlisted files.");
    }
    if plan.structure {
        Utils::warn(
            "Detected module structure changes; a full `kam dev --install` may be required.",
        );
    }
}

fn run_incremental(ctx: &DevContext, args: &DevArgs, plan: &WatchPlan) -> Result<(), KamError> {
    if plan.changed.is_empty() {
        return Ok(());
    }

    detect_device(ctx)?;
    if plan.webui {
        if args.sync_only {
            Utils::info("Sync-only mode: skipping dev-webui hooks.");
        } else {
            run_dev_webui_hooks(
                &ctx.project_root,
                &ctx.kam_toml,
                &ctx.output_dir,
                &ctx.build_args,
            )?;
        }
        sync_matching_hot_files(ctx, &["webroot/**"])?;
    }
    if plan.binary {
        if args.sync_only {
            Utils::info("Sync-only mode: skipping dev-binary hooks.");
        } else {
            run_dev_binary_hooks(
                &ctx.project_root,
                &ctx.kam_toml,
                &ctx.output_dir,
                &ctx.build_args,
            )?;
        }
        sync_matching_hot_files(ctx, &[".local/bin/**"])?;
    }
    if !plan.hot_files.is_empty() {
        sync_incremental_hot_files(ctx, &plan.hot_files)?;
    }
    if plan.webui || plan.binary || !plan.hot_files.is_empty() {
        run_dev_sync_hooks(
            &ctx.project_root,
            &ctx.kam_toml,
            &ctx.output_dir,
            &ctx.build_args,
        )?;
        run_restart_command(ctx)?;
    }
    if plan.structure {
        Utils::warn(
            "Skipped non-hot structural changes. Run `kam dev --install` for a full install.",
        );
    }
    run_forwards(ctx, args, false)?;
    if args.mcp {
        enable_mcp(ctx, false)?;
    }
    if should_show_logs(args) {
        show_logs(ctx, false)?;
    }
    Ok(())
}

fn rel_to_project(ctx: &DevContext, file: &Path) -> String {
    file.strip_prefix(&ctx.project_root)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_module_rel(ctx: &DevContext, file: &Path, prefix: &str) -> bool {
    file.strip_prefix(&ctx.module_root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .is_some_and(|rel| rel.starts_with(prefix))
}

fn changed_files(
    previous: &BTreeMap<PathBuf, SystemTime>,
    current: &BTreeMap<PathBuf, SystemTime>,
) -> Vec<PathBuf> {
    let mut files = BTreeSet::new();
    files.extend(previous.keys().cloned());
    files.extend(current.keys().cloned());
    files
        .into_iter()
        .filter(|file| previous.get(file) != current.get(file))
        .collect()
}

fn snapshot(ctx: &DevContext) -> Result<BTreeMap<PathBuf, SystemTime>, KamError> {
    let mut out = BTreeMap::new();
    for file in collect_hot_files(ctx)? {
        let modified = fs::metadata(&file)
            .and_then(|meta| meta.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        out.insert(file, modified);
    }
    for root in &ctx.watch_paths {
        if !root.exists() {
            continue;
        }
        for entry in WalkBuilder::new(root).git_ignore(false).build() {
            let entry =
                entry.map_err(|err| KamError::CommandFailed(format!("Walk error: {err}")))?;
            let path = entry.path();
            if path.is_file() {
                let modified = fs::metadata(path)
                    .and_then(|meta| meta.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                out.insert(path.to_path_buf(), modified);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::sync::default_hot_patterns;
    use super::super::sync::default_sync_policy;
    use super::*;
    use crate::cmds::build::args::BuildArgs;
    use crate::cmds::mcp::McpRuntime;
    use crate::types::kam_toml::KamToml;

    fn test_context() -> DevContext {
        let project_root = PathBuf::from("/tmp/kam-dev-test");
        let module_id = "MagicNet".to_string();
        let module_root = project_root.join("src").join(&module_id);
        let kam_toml = KamToml::default();
        let build_args = BuildArgs {
            path: ".".to_string(),
            all: false,
            output: None,
            bump: false,
            release: false,
            sign: false,
            interactive: false,
            pre_release: false,
            quiet: false,
            jobs: None,
            trim_shell: false,
            trim_shell_functions: false,
            obfuscate_shell: false,
        };
        let mcp = McpRuntime {
            project_root: project_root.clone(),
            module_id: module_id.clone(),
            module_path: format!("/data/adb/modules/{module_id}"),
            cli_path: format!("/data/adb/modules/{module_id}/cli"),
            device: None,
            device_port: 8765,
            local_port: 8765,
            endpoint: "/mcp".to_string(),
            transport: "streamable-http".to_string(),
        };
        DevContext {
            project_root,
            kam_toml,
            module_id,
            module_root,
            module_path: "/data/adb/modules/MagicNet".to_string(),
            device: None,
            hot_patterns: default_hot_patterns(),
            sync_policy: super::super::sync_plan::SyncPolicy::from_section(&default_sync_policy()),
            watch_paths: Vec::new(),
            logs: Vec::new(),
            forwards: Vec::new(),
            webui_port: None,
            webui_local_port: None,
            restart_command: None,
            session_log: PathBuf::from("/tmp/kam-dev-test/.kam/dev/last-session.log"),
            output_dir: PathBuf::from("/tmp/kam-dev-test/dist"),
            build_args,
            mcp,
        }
    }

    fn snapshot_from(files: &[PathBuf]) -> BTreeMap<PathBuf, SystemTime> {
        files
            .iter()
            .cloned()
            .map(|file| (file, SystemTime::UNIX_EPOCH))
            .collect()
    }

    fn changed_plan(file: PathBuf) -> WatchPlan {
        let ctx = test_context();
        let previous = BTreeMap::new();
        let current = snapshot_from(&[file]);
        plan_watch_changes(&ctx, &previous, &current).expect("watch plan")
    }

    #[test]
    fn watch_plan_routes_webui_source_to_webui_stage() {
        let ctx = test_context();
        let plan = changed_plan(ctx.project_root.join("webui/src/App.tsx"));
        assert!(plan.webui);
        assert!(!plan.binary);
        assert!(!plan.structure);
        assert!(plan.hot_files.is_empty());
    }

    #[test]
    fn watch_plan_routes_crate_source_to_binary_stage() {
        let ctx = test_context();
        let plan = changed_plan(ctx.project_root.join("crates/cli/src/main.rs"));
        assert!(plan.binary);
        assert!(!plan.webui);
        assert!(!plan.structure);
        assert!(plan.hot_files.is_empty());
    }

    #[test]
    fn watch_plan_routes_allowlisted_script_to_hot_sync() {
        let ctx = test_context();
        let script = ctx.module_root.join("service.sh");
        let plan = changed_plan(script.clone());
        assert!(!plan.webui);
        assert!(!plan.binary);
        assert!(!plan.structure);
        assert_eq!(plan.hot_files, vec![script]);
    }

    #[test]
    fn watch_plan_treats_runtime_config_as_structure_not_hot() {
        let ctx = test_context();
        let plan = changed_plan(ctx.module_root.join(".config/subscription.json"));
        assert!(!plan.webui);
        assert!(!plan.binary);
        assert!(plan.structure);
        assert!(plan.hot_files.is_empty());
    }
}
