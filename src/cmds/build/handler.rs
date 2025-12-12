use crate::errors::kam::KamError;
use std::path::Path;

use super::args::BuildArgs;
use super::{build_all, build_project};

/// Run the build command
pub fn run(args: BuildArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);

    if args.all {
        build_all::run_build_all(project_path, &args)?;
    } else {
        build_project::build_project(project_path, &args, None)?;
    }

    Ok(())
}
