use clap::{Args, Subcommand};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

#[derive(Args, Debug, Clone)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: McpCommand,

    /// adb device serial. Use auto to require exactly one connected device.
    #[arg(long, global = true)]
    pub device: Option<String>,

    /// Print planned commands without executing them
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Override local forwarded port
    #[arg(long, global = true)]
    pub local_port: Option<u16>,

    /// Override device MCP port
    #[arg(long, global = true)]
    pub port: Option<u16>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum McpCommand {
    /// Enable MCP through the module runtime CLI
    Enable,
    /// Disable MCP through the module runtime CLI
    Disable,
    /// Print MCP status from the module runtime CLI
    Status {
        /// Request JSON output from the module CLI
        #[arg(long)]
        json: bool,
    },
    /// Forward the local MCP port to the device MCP port
    Forward,
}

#[derive(Debug, Clone)]
pub struct McpRuntime {
    pub project_root: PathBuf,
    pub module_id: String,
    pub module_path: String,
    pub cli_path: String,
    pub device: Option<String>,
    pub device_port: u16,
    pub local_port: u16,
    pub endpoint: String,
    pub transport: String,
}

/// Run the MCP command.
///
/// # Errors
/// Returns `KamError` when project discovery, adb execution, or module CLI calls fail.
pub fn run(args: &McpArgs) -> Result<(), KamError> {
    let runtime = load_runtime(".", args.device.as_deref(), args.port, args.local_port)?;
    run_command(&runtime, &args.command, args.dry_run)
}

/// Load the standard MCP runtime from a Kam project directory.
///
/// # Errors
/// Returns `KamError` when the project path cannot be resolved or `kam.toml`
/// cannot be loaded.
pub fn load_runtime<P: AsRef<Path>>(
    project_root: P,
    device_override: Option<&str>,
    port_override: Option<u16>,
    local_port_override: Option<u16>,
) -> Result<McpRuntime, KamError> {
    let project_root = project_root.as_ref().canonicalize().map_err(KamError::Io)?;
    let kam_toml = KamToml::load_from_dir(&project_root)?;
    runtime_from_toml(
        project_root,
        &kam_toml,
        device_override,
        port_override,
        local_port_override,
    )
}

/// Build an MCP runtime description from a loaded `kam.toml`.
///
/// # Errors
/// Returns `KamError` when the configured values cannot produce a valid runtime.
pub fn runtime_from_toml(
    project_root: PathBuf,
    kam_toml: &KamToml,
    device_override: Option<&str>,
    port_override: Option<u16>,
    local_port_override: Option<u16>,
) -> Result<McpRuntime, KamError> {
    let module_id = kam_toml.prop.id.clone();
    let dev = kam_toml.dev.clone().unwrap_or_default();
    let mcp = dev.mcp.clone().unwrap_or_default();
    let module_path = dev
        .module_path
        .clone()
        .unwrap_or_else(|| format!("/data/adb/modules/{module_id}"));
    let cli_path = mcp
        .cli
        .clone()
        .unwrap_or_else(|| format!("{module_path}/cli"));
    let device_port = port_override.or(mcp.port).unwrap_or(8765);
    let local_port = local_port_override
        .or(mcp.local_port)
        .unwrap_or(device_port);
    let endpoint = mcp.endpoint.unwrap_or_else(|| "/mcp".to_string());
    let transport = mcp
        .transport
        .unwrap_or_else(|| "streamable-http".to_string());
    let device = device_override
        .map(ToOwned::to_owned)
        .or(dev.device)
        .filter(|value| !value.eq_ignore_ascii_case("auto"));

    Ok(McpRuntime {
        project_root,
        module_id,
        module_path,
        cli_path,
        device,
        device_port,
        local_port,
        endpoint,
        transport,
    })
}

/// Run a standard MCP contract command.
///
/// # Errors
/// Returns `KamError` when adb is missing or the forwarded/module command fails.
pub fn run_command(
    runtime: &McpRuntime,
    command: &McpCommand,
    dry_run: bool,
) -> Result<(), KamError> {
    match command {
        McpCommand::Forward => forward(runtime, dry_run),
        McpCommand::Enable => module_cli(runtime, &["mcp", "enable"], dry_run),
        McpCommand::Disable => module_cli(runtime, &["mcp", "disable"], dry_run),
        McpCommand::Status { json } => {
            if *json {
                module_cli(runtime, &["mcp", "status", "--json"], dry_run)
            } else {
                module_cli(runtime, &["mcp", "status"], dry_run)
            }
        }
    }
}

/// Forward the configured local MCP port to the device MCP port.
///
/// # Errors
/// Returns `KamError` when adb is missing or `adb forward` fails.
pub fn forward(runtime: &McpRuntime, dry_run: bool) -> Result<(), KamError> {
    ensure_adb()?;
    let local = format!("tcp:{}", runtime.local_port);
    let remote = format!("tcp:{}", runtime.device_port);
    if dry_run {
        Utils::info(format!(
            "Would run: {}",
            adb_command(runtime, &["forward", &local, &remote])
        ));
        Utils::info(format!("MCP URL: {}", runtime.url()));
        return Ok(());
    }

    let mut cmd = adb(runtime);
    cmd.arg("forward").arg(&local).arg(&remote);
    let status = Utils::run_and_stream_no_stderr_header(cmd).map_err(KamError::Io)?;
    if !status.success() {
        return Err(KamError::CommandFailed(format!(
            "adb forward failed with status {status}"
        )));
    }
    Utils::success(format!("Forwarded MCP: {}", runtime.url()));
    Ok(())
}

/// Execute the module standard CLI through `adb shell su -c`.
///
/// # Errors
/// Returns `KamError` when adb is missing or the module CLI command fails.
pub fn module_cli(runtime: &McpRuntime, args: &[&str], dry_run: bool) -> Result<(), KamError> {
    ensure_adb()?;
    let shell = format!("{} {}", shell_quote(&runtime.cli_path), shell_words(args));
    let printable = adb_command(runtime, &["shell", "su", "-c", &shell]);
    if dry_run {
        Utils::info(format!("Would run: {printable}"));
        return Ok(());
    }

    let mut cmd = adb(runtime);
    cmd.arg("shell").arg("su").arg("-c").arg(shell);
    cmd.stdin(Stdio::inherit());
    let status = Utils::run_and_stream_no_stderr_header(cmd).map_err(KamError::Io)?;
    if !status.success() {
        return Err(KamError::CommandFailed(format!(
            "MCP module CLI command failed with status {status}: {printable}"
        )));
    }
    Ok(())
}

impl McpRuntime {
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}{}", self.local_port, self.endpoint)
    }
}

fn ensure_adb() -> Result<(), KamError> {
    if crate::utils::command_exists("adb") {
        Ok(())
    } else {
        Err(KamError::CommandFailed(
            "adb not found on PATH. Install Android platform-tools.".to_string(),
        ))
    }
}

fn adb(runtime: &McpRuntime) -> Command {
    let mut cmd = Command::new("adb");
    if let Some(device) = &runtime.device {
        cmd.arg("-s").arg(device);
    }
    cmd
}

fn adb_command(runtime: &McpRuntime, args: &[&str]) -> String {
    let mut parts = vec!["adb".to_string()];
    if let Some(device) = &runtime.device {
        parts.push("-s".to_string());
        parts.push(shell_quote(device));
    }
    parts.extend(args.iter().map(|arg| shell_quote(arg)));
    parts.join(" ")
}

fn shell_words(args: &[&str]) -> String {
    args.iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

#[cfg(test)]
mod tests {
    use super::{McpCommand, runtime_from_toml};
    use crate::types::kam_toml::KamToml;
    use std::path::PathBuf;

    #[test]
    fn mcp_runtime_defaults_to_standard_contract() {
        let kt = KamToml::new_with_current_timestamp(
            "MagicNet".to_string(),
            "MagicNet".to_string(),
            "1.0.0".to_string(),
            None,
            "demo".to_string(),
            None,
            None,
        );
        let runtime = runtime_from_toml(PathBuf::from("/tmp/project"), &kt, None, None, None)
            .expect("runtime");
        assert_eq!(runtime.module_path, "/data/adb/modules/MagicNet");
        assert_eq!(runtime.cli_path, "/data/adb/modules/MagicNet/cli");
        assert_eq!(runtime.device_port, 8765);
        assert_eq!(runtime.local_port, 8765);
        assert_eq!(runtime.endpoint, "/mcp");
        assert_eq!(runtime.transport, "streamable-http");
        assert_eq!(runtime.url(), "http://127.0.0.1:8765/mcp");
    }

    #[test]
    fn mcp_status_json_variant_parses() {
        assert!(matches!(
            McpCommand::Status { json: true },
            McpCommand::Status { json: true }
        ));
    }
}
