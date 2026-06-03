use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::errors::KamError;
use crate::utils::Utils;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInfoRequest {
    pub packages: Vec<PathBuf>,
    pub quiet: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInfo {
    pub path: PathBuf,
    pub properties: BTreeMap<String, String>,
}

/// Show metadata from local module ZIP packages.
///
/// # Errors
///
/// Returns an error when no package is supplied, a ZIP cannot be read, or the
/// package does not contain a root `module.prop`.
pub fn handle_package_info(request: &PackageInfoRequest) -> Result<(), KamError> {
    if request.packages.is_empty() {
        return Err(KamError::CommandFailed(
            "Package query requires a zip path, e.g. `kam -Qp module.zip`".to_string(),
        ));
    }

    for package in &request.packages {
        let info = read_package_info(package)?;
        if request.quiet {
            println!("{}", property_or_dash(&info.properties, "id"));
        } else {
            print_package_info(&info);
        }
    }
    Ok(())
}

/// List files inside local module ZIP packages.
///
/// # Errors
///
/// Returns an error when no package is supplied or a ZIP cannot be read.
pub fn handle_package_files(request: &PackageInfoRequest) -> Result<(), KamError> {
    if request.packages.is_empty() {
        return Err(KamError::CommandFailed(
            "Package file query requires a zip path, e.g. `kam -Qpl module.zip`".to_string(),
        ));
    }

    for package in &request.packages {
        for entry in read_package_files(package)? {
            if request.quiet {
                println!("{entry}");
            } else {
                println!("{} {entry}", package.display());
            }
        }
    }
    Ok(())
}

pub fn read_package_info(path: &Path) -> Result<PackageInfo, KamError> {
    let file = File::open(path).map_err(KamError::Io)?;
    let mut archive = zip::ZipArchive::new(file).map_err(KamError::Zip)?;
    let mut module_prop = archive.by_name("module.prop").map_err(|_| {
        KamError::CommandFailed(format!("{}: missing root module.prop", path.display()))
    })?;
    let mut contents = String::new();
    module_prop
        .read_to_string(&mut contents)
        .map_err(KamError::Io)?;
    Ok(PackageInfo {
        path: path.to_path_buf(),
        properties: parse_module_prop(&contents),
    })
}

pub fn read_package_files(path: &Path) -> Result<Vec<String>, KamError> {
    let file = File::open(path).map_err(KamError::Io)?;
    let mut archive = zip::ZipArchive::new(file).map_err(KamError::Zip)?;
    let mut names = (0..archive.len())
        .filter_map(|idx| {
            archive
                .by_index(idx)
                .ok()
                .map(|entry| entry.name().to_string())
        })
        .collect::<Vec<_>>();
    names.sort();
    Ok(names)
}

#[must_use]
pub fn parse_module_prop(input: &str) -> BTreeMap<String, String> {
    let mut properties = BTreeMap::new();
    for line in input.lines() {
        let line = line.trim().trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            properties.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    properties
}

fn print_package_info(info: &PackageInfo) {
    Utils::section(info.path.display().to_string());
    println!(
        "Id             : {}",
        property_or_dash(&info.properties, "id")
    );
    println!(
        "Name           : {}",
        property_or_dash(&info.properties, "name")
    );
    println!(
        "Version        : {}",
        property_or_dash(&info.properties, "version")
    );
    println!(
        "Version Code   : {}",
        property_or_dash(&info.properties, "versionCode")
    );
    println!(
        "Author         : {}",
        property_or_dash(&info.properties, "author")
    );
    println!(
        "Description    : {}",
        property_or_dash(&info.properties, "description")
    );
    println!("Package        : {}", info.path.display());
}

fn property_or_dash<'a>(properties: &'a BTreeMap<String, String>, key: &str) -> &'a str {
    properties.get(key).map_or(
        "-",
        |value| if value.trim().is_empty() { "-" } else { value },
    )
}

#[cfg(test)]
mod tests {
    use super::parse_module_prop;

    #[test]
    fn parses_module_prop_metadata() {
        let properties = parse_module_prop(
            "# comment\n\
             id=MagicNet\n\
             name = MagicNet\n\
             version=v1.0.0\r\n\
             versionCode=42\n",
        );

        assert_eq!(properties.get("id").map(String::as_str), Some("MagicNet"));
        assert_eq!(properties.get("name").map(String::as_str), Some("MagicNet"));
        assert_eq!(
            properties.get("version").map(String::as_str),
            Some("v1.0.0")
        );
        assert_eq!(
            properties.get("versionCode").map(String::as_str),
            Some("42")
        );
    }
}
