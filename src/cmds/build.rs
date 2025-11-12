mod args;
mod build_all;
mod build_project;
mod post_build;
mod pre_build;

pub use args::BuildArgs;
pub use build_all::run_build_all;
pub use build_project::build_project;
pub use post_build::handle_post_build_hook;
pub use pre_build::handle_pre_build_hook;

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
