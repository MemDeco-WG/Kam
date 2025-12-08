mod args;
mod build_all;
mod build_project;
mod hooks;

pub use args::BuildArgs;
pub use build_all::run_build_all;
pub use build_project::build_project;
pub use hooks::{run_post_build_hooks, run_pre_build_hooks};

use crate::errors::kam::KamError;
use std::path::Path;

/// Run the build command
pub fn run(args: BuildArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);

    if args.all {
        run_build_all(project_path, &args)?;
    } else {
        build_project(project_path, &args, None)?;
    }

    Ok(())
}
