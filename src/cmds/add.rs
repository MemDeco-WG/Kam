use clap::{Args, Subcommand, ValueEnum};
use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

#[derive(Args, Debug)]
pub struct AddArgs {
    #[command(subcommand)]
    pub command: AddCommands,
}

#[derive(Subcommand, Debug)]
pub enum AddCommands {
    /// Add a runtime module script such as service.sh or action.sh
    Script {
        /// Runtime phase/script to add
        phase: ScriptPhase,

        /// Print planned changes without writing files
        #[arg(long)]
        dry_run: bool,

        /// Overwrite an existing script
        #[arg(short, long)]
        force: bool,
    },

    /// Add a build hook under hooks/pre-build or hooks/post-build
    Hook {
        /// Hook phase directory
        phase: HookPhase,

        /// Hook name, for example sync-version
        name: String,

        /// Numeric hook order prefix
        #[arg(long, default_value_t = 10)]
        order: u32,

        /// Print planned changes without writing files
        #[arg(long)]
        dry_run: bool,

        /// Overwrite an existing hook
        #[arg(short, long)]
        force: bool,
    },

    /// Add imports for an existing kamfw helper module
    Kamfw {
        /// kamfw module name, for example watchdog, notify, fswatch, rich
        module: String,

        /// Runtime script to receive the import
        #[arg(long, default_value_t = ScriptPhase::Service)]
        phase: ScriptPhase,

        /// Print planned changes without writing files
        #[arg(long)]
        dry_run: bool,

        /// Create the target runtime script if it does not exist
        #[arg(short, long)]
        force: bool,
    },

    /// Add a basic WebUI skeleton
    Webui {
        /// WebUI template kind
        #[arg(long, default_value_t = WebuiTemplate::Static)]
        template: WebuiTemplate,

        /// Print planned changes without writing files
        #[arg(long)]
        dry_run: bool,

        /// Overwrite existing WebUI files
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum ScriptPhase {
    Customize,
    PostFsData,
    Service,
    Uninstall,
    Action,
    BootCompleted,
    PostMount,
}

impl std::fmt::Display for ScriptPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.file_name().trim_end_matches(".sh"))
    }
}

impl ScriptPhase {
    fn file_name(self) -> &'static str {
        match self {
            Self::Customize => "customize.sh",
            Self::PostFsData => "post-fs-data.sh",
            Self::Service => "service.sh",
            Self::Uninstall => "uninstall.sh",
            Self::Action => "action.sh",
            Self::BootCompleted => "boot-completed.sh",
            Self::PostMount => "post-mount.sh",
        }
    }

    fn function_name(self) -> &'static str {
        match self {
            Self::Customize => "kamfw_phase_install",
            Self::PostFsData => "kamfw_phase_post_fs_data",
            Self::Service => "kamfw_phase_service",
            Self::Uninstall => "kamfw_phase_uninstall",
            Self::Action => "kamfw_phase_action",
            Self::BootCompleted => "kamfw_phase_boot_completed",
            Self::PostMount => "kamfw_phase_post_mount",
        }
    }

    fn phase_name(self) -> &'static str {
        match self {
            Self::Customize => "install",
            Self::PostFsData => "post-fs-data",
            Self::Service => "service",
            Self::Uninstall => "uninstall",
            Self::Action => "action",
            Self::BootCompleted => "boot-completed",
            Self::PostMount => "post-mount",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum HookPhase {
    PreBuild,
    PostBuild,
}

impl HookPhase {
    fn dir_name(self) -> &'static str {
        match self {
            Self::PreBuild => "pre-build",
            Self::PostBuild => "post-build",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum WebuiTemplate {
    Static,
}

impl std::fmt::Display for WebuiTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Static => f.write_str("static"),
        }
    }
}

struct ProjectContext {
    root: PathBuf,
    module_id: String,
    module_dir: PathBuf,
}

/// Run the add command.
///
/// # Errors
/// Returns `KamError` if the current directory is not inside a Kam project, the
/// requested target already exists without `--force`, or required kamfw files
/// are missing.
pub fn run(args: &AddArgs) -> Result<(), KamError> {
    match &args.command {
        AddCommands::Script {
            phase,
            dry_run,
            force,
        } => add_script(*phase, *dry_run, *force),
        AddCommands::Hook {
            phase,
            name,
            order,
            dry_run,
            force,
        } => add_hook(*phase, name, *order, *dry_run, *force),
        AddCommands::Kamfw {
            module,
            phase,
            dry_run,
            force,
        } => add_kamfw(module, *phase, *dry_run, *force),
        AddCommands::Webui {
            template,
            dry_run,
            force,
        } => add_webui(*template, *dry_run, *force),
    }
}

fn load_project() -> Result<ProjectContext, KamError> {
    let root = find_project_root()?;
    let kt = KamToml::load_from_dir(&root)?;
    let module_id = kt.prop.id;
    if module_id.trim().is_empty() {
        return Err(KamError::InvalidConfig(
            "kam.toml prop.id cannot be empty".to_string(),
        ));
    }

    let module_dir = root.join("src").join(&module_id);
    Ok(ProjectContext {
        root,
        module_id,
        module_dir,
    })
}

fn find_project_root() -> Result<PathBuf, KamError> {
    let mut cwd = std::env::current_dir().map_err(KamError::Io)?;
    loop {
        if cwd.join("kam.toml").exists() {
            return Ok(cwd);
        }
        if !cwd.pop() {
            return Err(KamError::InvalidDirectory(
                "kam.toml not found. Run this command inside a Kam project.".to_string(),
            ));
        }
    }
}

fn add_script(phase: ScriptPhase, dry_run: bool, force: bool) -> Result<(), KamError> {
    let project = load_project()?;
    let path = project.module_dir.join(phase.file_name());
    let content = script_content(phase);
    write_generated_file(&path, &content, dry_run, force)?;
    success_or_plan(
        dry_run,
        format!("Added runtime script {}", relative(&project.root, &path)),
    );
    Ok(())
}

fn add_hook(
    phase: HookPhase,
    name: &str,
    order: u32,
    dry_run: bool,
    force: bool,
) -> Result<(), KamError> {
    validate_slug(name, "hook name")?;
    let project = load_project()?;
    let file_name = format!("{order:04}.{name}.sh");
    let path = project
        .root
        .join("hooks")
        .join(phase.dir_name())
        .join(file_name);
    let content = hook_content(name);
    write_generated_file(&path, &content, dry_run, force)?;
    success_or_plan(
        dry_run,
        format!("Added build hook {}", relative(&project.root, &path)),
    );
    Ok(())
}

fn add_kamfw(module: &str, phase: ScriptPhase, dry_run: bool, force: bool) -> Result<(), KamError> {
    validate_slug(module, "kamfw module")?;
    let project = load_project()?;
    let module_file = project
        .module_dir
        .join("lib")
        .join("kamfw")
        .join(format!("{module}.sh"));
    if !module_file.exists() {
        return Err(KamError::InvalidDirectory(format!(
            "kamfw module not found: {}",
            relative(&project.root, &module_file)
        )));
    }

    let script = project.module_dir.join(phase.file_name());
    let content = if script.exists() {
        let current = fs::read_to_string(&script).map_err(KamError::Io)?;
        add_import_to_script(&current, module)
    } else {
        kamfw_script_content(phase, module)
    };

    write_generated_file(&script, &content, dry_run, force || script.exists())?;
    success_or_plan(
        dry_run,
        format!(
            "Enabled kamfw module '{module}' in {}",
            relative(&project.root, &script)
        ),
    );
    Ok(())
}

fn add_webui(template: WebuiTemplate, dry_run: bool, force: bool) -> Result<(), KamError> {
    match template {
        WebuiTemplate::Static => add_static_webui(dry_run, force),
    }
}

fn add_static_webui(dry_run: bool, force: bool) -> Result<(), KamError> {
    let project = load_project()?;
    let webroot = project.module_dir.join("webroot");
    let index = webroot.join("index.html");
    let css = webroot.join("style.css");
    let js = webroot.join("main.js");

    write_generated_file(&index, &webui_index(&project.module_id), dry_run, force)?;
    write_generated_file(&css, WEBUI_CSS, dry_run, force)?;
    write_generated_file(&js, WEBUI_JS, dry_run, force)?;
    success_or_plan(
        dry_run,
        format!(
            "Added static WebUI under {}",
            relative(&project.root, &webroot)
        ),
    );
    Ok(())
}

fn script_content(phase: ScriptPhase) -> String {
    kamfw_script_content(phase, "")
}

fn kamfw_script_content(phase: ScriptPhase, module: &str) -> String {
    let import = if module.is_empty() {
        String::new()
    } else {
        format!("import {module}\n\n")
    };
    let moddir = if phase == ScriptPhase::Customize {
        r#"MODDIR="${MODDIR:-$MODPATH}""#
    } else {
        r#"MODDIR="${0%/*}""#
    };
    format!(
        r#"#!/system/bin/sh
{moddir}
. "$MODDIR/lib/kamfw/.kamfwrc" || exit 1
import __runtime__
{import}{function_name}() {{
    :
}}

kamfw run {phase_name} -- "$@"
"#,
        function_name = phase.function_name(),
        phase_name = phase.phase_name(),
    )
}

fn hook_content(name: &str) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

echo "kam hook: {name}"
"#
    )
}

fn add_import_to_script(content: &str, module: &str) -> String {
    let import_line = format!("import {module}");
    if content.lines().any(|line| line.trim() == import_line) {
        return content.to_string();
    }

    let mut out = String::new();
    let mut inserted = false;
    for line in content.lines() {
        out.push_str(line);
        out.push('\n');
        if !inserted && line.contains(".kamfwrc") {
            out.push_str(&import_line);
            out.push('\n');
            inserted = true;
        }
    }
    if !inserted {
        out.push_str(&import_line);
        out.push('\n');
    }
    out
}

fn write_generated_file(
    path: &Path,
    content: &str,
    dry_run: bool,
    force: bool,
) -> Result<(), KamError> {
    if path.exists() && !force {
        return Err(KamError::InvalidConfig(format!(
            "{} already exists. Pass --force to overwrite it.",
            path.display()
        )));
    }
    if dry_run {
        Utils::info(format!("Would write {}", path.display()));
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }
    fs::write(path, content).map_err(KamError::Io)?;
    set_executable_if_shell(path)?;
    Ok(())
}

fn set_executable_if_shell(path: &Path) -> Result<(), KamError> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("sh") {
        return Ok(());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).map_err(KamError::Io)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(KamError::Io)?;
    }

    Ok(())
}

fn validate_slug(value: &str, label: &str) -> Result<(), KamError> {
    let valid = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        Ok(())
    } else {
        Err(KamError::InvalidConfig(format!(
            "Invalid {label} '{value}'. Use only letters, digits, '-' and '_'."
        )))
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn success_or_plan(dry_run: bool, msg: String) {
    if dry_run {
        Utils::info(format!("Plan: {msg}"));
    } else {
        Utils::success(msg);
    }
}

fn webui_index(module_id: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{module_id}</title>
    <link rel="stylesheet" href="./style.css">
  </head>
  <body>
    <main>
      <h1>{module_id}</h1>
      <p>Kam module WebUI</p>
      <button id="refresh" type="button">Refresh</button>
      <pre id="status">ready</pre>
    </main>
    <script src="./main.js"></script>
  </body>
</html>
"#
    )
}

const WEBUI_CSS: &str = r"body {
  margin: 0;
  font-family: system-ui, sans-serif;
  color: #f5f7fa;
  background: #101418;
}

main {
  max-width: 720px;
  margin: 0 auto;
  padding: 24px;
}

button {
  min-height: 40px;
  padding: 0 14px;
}

pre {
  overflow: auto;
  padding: 12px;
  background: #1b2229;
}
";

const WEBUI_JS: &str = r#"document.getElementById("refresh")?.addEventListener("click", () => {
  document.getElementById("status").textContent = new Date().toISOString();
});
"#;

#[cfg(test)]
mod tests {
    use super::add_import_to_script;

    #[test]
    fn add_import_after_kamfwrc_source() {
        let script = "#!/system/bin/sh\n. \"$MODDIR/lib/kamfw/.kamfwrc\" || exit 1\n";
        let updated = add_import_to_script(script, "watchdog");
        assert!(updated.contains(".kamfwrc\" || exit 1\nimport watchdog\n"));
    }

    #[test]
    fn add_import_is_idempotent() {
        let script = "#!/system/bin/sh\nimport notify\n";
        assert_eq!(add_import_to_script(script, "notify"), script);
    }
}
