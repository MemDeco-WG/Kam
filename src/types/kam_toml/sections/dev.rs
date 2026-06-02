use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[allow(non_snake_case)]
/// Development-loop defaults used by `kam dev`.
pub struct DevSection {
    /// adb serial to use. Use `auto` to require exactly one connected device.
    pub device: Option<String>,
    /// Remote module directory. Defaults to `/data/adb/modules/<id>`.
    pub module_path: Option<String>,
    /// Glob allowlist for hot-push paths relative to the module source directory.
    pub hot: Option<Vec<String>>,
    /// Local paths watched by `kam dev --watch`.
    pub watch: Option<Vec<String>>,
    /// Device log files or glob patterns collected by `kam dev --logs`.
    pub logs: Option<Vec<String>>,
    /// Forward declarations such as `mcp` or `webui`.
    pub forward: Option<Vec<String>>,
    /// Device-side WebUI port used by `kam dev --webui`.
    pub webui_port: Option<u16>,
    /// Host-side forwarded WebUI port.
    pub webui_local_port: Option<u16>,
    /// Optional root shell command to run after a successful hot update.
    pub restart_command: Option<String>,
    /// Standard MCP runtime contract.
    pub mcp: Option<McpSection>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[allow(non_snake_case)]
/// Standard MCP runtime contract used by `kam mcp` and `kam dev --mcp`.
pub struct McpSection {
    /// Whether MCP is enabled for dev sessions.
    pub enabled: Option<bool>,
    /// Device-side MCP port.
    pub port: Option<u16>,
    /// Host-side forwarded port.
    pub local_port: Option<u16>,
    /// MCP HTTP endpoint path.
    pub endpoint: Option<String>,
    /// MCP transport name. Defaults to `streamable-http`.
    pub transport: Option<String>,
    /// Standard module CLI path. Defaults to `/data/adb/modules/<id>/cli`.
    pub cli: Option<String>,
}

impl Default for DevSection {
    fn default() -> Self {
        Self {
            device: Some("auto".to_string()),
            module_path: None,
            hot: Some(vec![
                "webroot/**".to_string(),
                "customize.sh".to_string(),
                "post-fs-data.sh".to_string(),
                "service.sh".to_string(),
                "uninstall.sh".to_string(),
                "action.sh".to_string(),
                "boot-completed.sh".to_string(),
                "post-mount.sh".to_string(),
                ".local/bin/**".to_string(),
                "templates/**".to_string(),
                "config-templates/**".to_string(),
                "system.prop".to_string(),
                "sepolicy.rule".to_string(),
            ]),
            watch: Some(vec![
                "webui".to_string(),
                "crates".to_string(),
                "src/{{id}}".to_string(),
                "hooks/dev-build".to_string(),
                "hooks/dev-webui".to_string(),
                "hooks/dev-binary".to_string(),
                "hooks/dev-sync".to_string(),
            ]),
            logs: None,
            forward: Some(Vec::new()),
            webui_port: None,
            webui_local_port: None,
            restart_command: None,
            mcp: Some(McpSection::default()),
        }
    }
}

impl Default for McpSection {
    fn default() -> Self {
        Self {
            enabled: Some(true),
            port: Some(8765),
            local_port: None,
            endpoint: Some("/mcp".to_string()),
            transport: Some("streamable-http".to_string()),
            cli: None,
        }
    }
}
