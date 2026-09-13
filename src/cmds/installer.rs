//! Convenience wrapper for creating a module downloader installer.

use clap::Args;

use crate::cmds::init::{self, InitArgs, KernelSuRepoMode};
use crate::errors::KamError;

/// Create an installer module from Kam's bundled downloader template.
#[derive(Args, Debug)]
pub struct InstallerArgs {
    /// Destination project directory.
    #[arg(value_name = "PATH")]
    pub path: String,
    /// Installer module ID.
    #[arg(long, default_value = "magicnet_installer")]
    pub id: String,
    /// Human-readable project name.
    #[arg(long, default_value = "Module Installer")]
    pub project_name: String,
    /// Installer version. Defaults to the template's version.
    #[arg(long)]
    pub version: Option<String>,
    /// Module author.
    #[arg(long, default_value = "Your Name")]
    pub author: String,
    /// GitHub owner/repository to download from.
    #[arg(long, default_value = "LIghtJUNction/MagicNet")]
    pub repository: String,
    /// Exact release asset name.
    #[arg(long, default_value = "MagicNet.zip")]
    pub asset: String,
    /// Expected module.prop ID in the downloaded archive.
    #[arg(long, default_value = "MagicNet")]
    pub module_id: String,
    /// Persistent cache directory. Empty uses /data/adb/cache/{module_id}.
    #[arg(long, default_value = "")]
    pub cache_dir: String,
    /// HTTPS mirror prefix; may be repeated.
    #[arg(long = "proxy")]
    pub proxies: Vec<String>,
    /// Direct-download speed threshold before mirror probing, in KiB/s.
    #[arg(long, default_value_t = 256)]
    pub direct_min_kib: i64,
    /// Overwrite an existing project.
    #[arg(short, long)]
    pub force: bool,
}

/// Initialize the configured installer project.
///
/// # Errors
///
/// Returns an error when the destination cannot be initialized or the bundled
/// downloader template cannot be rendered.
pub fn run(args: &InstallerArgs) -> Result<(), KamError> {
    let mut vars = vec![
        format!("target_repository={}", args.repository),
        format!("target_asset={}", args.asset),
        format!("target_module={}", args.module_id),
        format!("target_cache_dir={}", args.cache_dir),
        format!("target_direct_min_kib={}", args.direct_min_kib),
    ];
    if !args.proxies.is_empty() {
        let json = args
            .proxies
            .iter()
            .map(|proxy| format!("\"{}\"", proxy.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(", ");
        vars.push(format!("target_proxies_json={json}"));
    }
    init::run(&InitArgs {
        name: Some(args.path.clone()),
        id: Some(args.id.clone()),
        project_name: Some(args.project_name.clone()),
        version: args.version.clone(),
        author: Some(args.author.clone()),
        update_json: None,
        description: Some("Verified latest-release module installer".to_string()),
        repo_mode: KernelSuRepoMode::Full,
        source_url: None,
        metamodule: false,
        force: args.force,
        interactive: false,
        r#impl: None,
        var: vars,
        template: Some("downloader_template".to_string()),
        tmpl: false,
    })
}
