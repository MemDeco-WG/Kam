use super::args::BuildArgs;
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;

use std::fs;
use std::path::{Path, PathBuf};

/// # Errors
/// Returns `KamError` if the output directory cannot be created or resolved.
pub fn determine_output_dir(
    project_root: &Path,
    _args: &BuildArgs,
    kam_toml: &KamToml,
) -> Result<PathBuf, KamError> {
    let target_dir = kam_toml
        .kam
        .build
        .as_ref()
        .and_then(|b| b.target_dir.as_ref())
        .map_or("dist", |s| s.as_str());

    let output_dir = if Path::new(target_dir).is_absolute() {
        PathBuf::from(target_dir)
    } else {
        project_root.join(target_dir)
    };

    fs::create_dir_all(&output_dir)?;
    Ok(output_dir.canonicalize().unwrap_or(output_dir))
}

/// # Errors
/// Returns `KamError` if required fields are missing or invalid when computing a basename.
pub fn determine_basename(kam_toml: &KamToml) -> Result<String, KamError> {
    if let Some(build) = &kam_toml.kam.build
        && let Some(output_file) = &build.output_file
        && !output_file.is_empty()
    {
        let mut name = output_file.clone();
        name = name.replace("{{id}}", &kam_toml.prop.id);
        name = name.replace("{{version}}", &kam_toml.prop.version);
        name = name.replace("{{versionCode}}", &kam_toml.prop.versionCode.to_string());
        name = name.replace("{{name}}", kam_toml.prop.get_name());
        return Ok(name);
    }

    Ok(format!(
        "{}-{}-{}",
        kam_toml.prop.id, kam_toml.prop.versionCode, kam_toml.prop.version
    ))
}
