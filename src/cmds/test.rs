use clap::Args;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::errors::KamError;
use crate::utils::Utils;

/// Arguments for `kam test`.
#[derive(Args, Debug)]
pub struct TestArgs {
    /// Project directory to test.
    #[arg(short = 'C', long = "directory", default_value = ".")]
    pub directory: PathBuf,

    /// Test script to execute instead of the project default.
    #[arg(long, value_name = "SCRIPT")]
    pub script: Option<PathBuf>,

    /// Arguments passed through to the test script, for example `quick`, `package`, or `avd`.
    #[arg(value_name = "ARGS", num_args = 0.., trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Run a project-local test script.
///
/// Kam intentionally delegates project-specific testing to a repository script.
/// The default lookup order is `scripts/kam-test.sh`, then `kam-test.sh`.
///
/// # Errors
/// Returns `KamError` if the project directory, script, or test command fails.
pub fn run(args: &TestArgs) -> Result<(), KamError> {
    let project_dir = resolve_project_dir(&args.directory)?;
    let script = resolve_script(&project_dir, args.script.as_deref())?;

    Utils::info(format!("Running test script: {}", script.display()));
    if !args.args.is_empty() {
        Utils::info(format!("Test arguments: {}", args.args.join(" ")));
    }

    let current_exe = env::current_exe().ok();
    let mut command = Command::new("bash");
    command
        .arg(&script)
        .args(&args.args)
        .current_dir(&project_dir)
        .env("KAM_PROJECT_DIR", &project_dir)
        .env("KAM_TEST_SCRIPT", &script)
        .env("KAM_TEST", "1")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(exe) = current_exe {
        command.env("KAM_BIN", exe);
    }

    let status = command.status()?;
    if status.success() {
        return Ok(());
    }

    let detail = status.code().map_or_else(
        || "terminated by signal".to_string(),
        |code| format!("exit code {code}"),
    );
    Err(KamError::CommandFailed(format!(
        "{} failed with {detail}",
        script.display()
    )))
}

fn resolve_project_dir(directory: &Path) -> Result<PathBuf, KamError> {
    let cwd = env::current_dir()?;
    let candidate = if directory.is_absolute() {
        directory.to_path_buf()
    } else {
        cwd.join(directory)
    };
    let project_dir = candidate
        .canonicalize()
        .map_err(|err| KamError::InvalidDirectory(format!("{} ({err})", candidate.display())))?;
    if !project_dir.is_dir() {
        return Err(KamError::InvalidDirectory(
            project_dir.display().to_string(),
        ));
    }
    Ok(project_dir)
}

fn resolve_script(project_dir: &Path, explicit: Option<&Path>) -> Result<PathBuf, KamError> {
    if let Some(script) = explicit {
        let path = if script.is_absolute() {
            script.to_path_buf()
        } else {
            project_dir.join(script)
        };
        return validate_script(path);
    }

    for relative in ["scripts/kam-test.sh", "kam-test.sh"] {
        let path = project_dir.join(relative);
        if path.is_file() {
            return Ok(path);
        }
    }

    Err(KamError::PackageNotFound(format!(
        "no test script found in {}; expected scripts/kam-test.sh or kam-test.sh",
        project_dir.display()
    )))
}

fn validate_script(path: PathBuf) -> Result<PathBuf, KamError> {
    if path.is_file() {
        Ok(path)
    } else {
        Err(KamError::PackageNotFound(format!(
            "test script not found: {}",
            path.display()
        )))
    }
}
