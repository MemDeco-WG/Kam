use std::path::PathBuf;

use crate::cmds::build::args::BuildArgs;
use crate::cmds::build::build_project::determine_output_dir;
use crate::cmds::mcp::{self, McpRuntime};
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;

use super::args::DevArgs;
use super::sync::{default_hot_patterns, default_sync_policy, default_watch_paths};
use super::sync_plan::SyncPolicy;

#[derive(Debug)]
pub(super) struct DevContext {
    pub(super) project_root: PathBuf,
    pub(super) kam_toml: KamToml,
    pub(super) module_id: String,
    pub(super) module_root: PathBuf,
    pub(super) module_path: String,
    pub(super) device: Option<String>,
    pub(super) hot_patterns: Vec<String>,
    pub(super) sync_policy: SyncPolicy,
    pub(super) watch_paths: Vec<PathBuf>,
    pub(super) logs: Vec<String>,
    pub(super) forwards: Vec<String>,
    pub(super) webui_port: Option<u16>,
    pub(super) webui_local_port: Option<u16>,
    pub(super) restart_command: Option<String>,
    pub(super) session_log: PathBuf,
    pub(super) output_dir: PathBuf,
    pub(super) build_args: BuildArgs,
    pub(super) mcp: McpRuntime,
}

pub(super) fn load_context(args: &DevArgs) -> Result<DevContext, KamError> {
    let project_root = std::env::current_dir()
        .map_err(KamError::Io)?
        .canonicalize()?;
    let kam_toml = KamToml::load_from_dir(&project_root)?;
    let module_id = kam_toml.prop.id.clone();
    let dev = kam_toml.dev.clone().unwrap_or_default();
    let module_root = kam_toml.kam.build.as_ref().map_or_else(
        || project_root.join("src").join(&module_id),
        |build| {
            build.source_dir.as_ref().map_or_else(
                || project_root.join("src").join(&module_id),
                |source| project_root.join(source.replace("{{id}}", &module_id)),
            )
        },
    );
    let module_path = dev
        .module_path
        .clone()
        .unwrap_or_else(|| format!("/data/adb/modules/{module_id}"));
    let device = args
        .device
        .clone()
        .or(dev.device.clone())
        .filter(|value| !value.eq_ignore_ascii_case("auto"));
    let hot_patterns = dev.hot.clone().unwrap_or_else(default_hot_patterns);
    let sync_policy = SyncPolicy::from_section(&dev.sync.unwrap_or_else(default_sync_policy));
    let watch_paths = dev
        .watch
        .clone()
        .unwrap_or_else(default_watch_paths)
        .into_iter()
        .map(|path| project_root.join(path.replace("{{id}}", &module_id)))
        .collect();
    let logs = dev.logs.clone().unwrap_or_else(|| {
        vec![
            format!("{module_path}/logs/*.log"),
            format!("{module_path}/.log/*.log"),
        ]
    });
    let forwards = dev.forward.clone().unwrap_or_default();
    let webui_port = dev.webui_port;
    let webui_local_port = dev.webui_local_port.or(webui_port);
    let restart_command = dev.restart_command.clone();
    let session_log = project_root
        .join(".kam")
        .join("dev")
        .join("last-session.log");
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
    let output_dir = determine_output_dir(&project_root, &build_args, &kam_toml)?;
    let mcp = mcp::runtime_from_toml(
        project_root.clone(),
        &kam_toml,
        device.as_deref(),
        None,
        None,
    )?;

    Ok(DevContext {
        project_root,
        kam_toml,
        module_id,
        module_root,
        module_path,
        device,
        hot_patterns,
        sync_policy,
        watch_paths,
        logs,
        forwards,
        webui_port,
        webui_local_port,
        restart_command,
        session_log,
        output_dir,
        build_args,
        mcp,
    })
}
