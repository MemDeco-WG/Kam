use super::args::BuildArgs;
use super::package_filter::should_skip_file;
use super::package_module::{packaging_progress, update_packaging_progress};
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

use std::fs::File;
use std::path::{Path, PathBuf};
use tar::Builder as TarBuilder;

/// # Errors
/// Returns `KamError` if packaging or I/O fails while creating a template archive.
pub fn create_template_archive(
    kam_toml: &KamToml,
    output_dir: &Path,
    basename: &str,
    project_root: &Path,
    args: &BuildArgs,
) -> Result<PathBuf, KamError> {
    let source_filename = format!("{basename}.tar.gz");
    let source_output_file = output_dir.join(&source_filename);
    let tar_gz = File::create(&source_output_file)?;
    let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
    let mut tar = TarBuilder::new(enc);
    let exclude_patterns = build_exclude_patterns(kam_toml);
    let include_patterns = build_include_patterns(kam_toml);
    let exclude_dir_names = crate::utils::default_exclude_dir_names();
    let file_count = count_template_files(
        project_root,
        output_dir,
        &exclude_dir_names,
        &exclude_patterns,
        &include_patterns,
    );
    let walker = ignore::WalkBuilder::new(project_root)
        .git_ignore(false)
        .hidden(false)
        .filter_entry(move |entry| {
            let name = entry.file_name().to_string_lossy();
            !exclude_dir_names.iter().any(|d| d == name.as_ref())
        })
        .build();
    let pb = packaging_progress(args, file_count);
    let mut count = 0;

    for result in walker {
        let entry = result.map_err(|e| KamError::Io(std::io::Error::other(e)))?;
        let path = entry.path();
        if path == project_root {
            continue;
        }
        let rel_path = path
            .strip_prefix(project_root)
            .map_err(|e| KamError::InvalidDirectory(format!("strip_prefix failed: {e}")))?;
        if path.starts_with(output_dir) {
            continue;
        }
        let rel_str = rel_path.to_string_lossy();
        let file_name_opt = entry.file_name().to_str();
        if should_skip_file(
            &rel_str,
            file_name_opt,
            &exclude_patterns,
            &include_patterns,
        ) {
            continue;
        }
        if path.is_dir() {
            tar.append_dir(rel_path, path)?;
        } else if path.is_file() {
            update_packaging_progress(pb.as_ref(), &rel_str);
            tar.append_path_with_name(path, rel_path)?;
            count += 1;
            if let Some(p) = &pb {
                p.set_position(count);
            }
        }
    }
    if let Some(p) = pb {
        p.finish_with_message(format!("✓ Packaged {count} files"));
    }

    tar.finish()?;

    if !args.quiet {
        println!();
        Utils::success(&trf!(
            "packaging.success_template_built",
            source_output_file.display()
        ));
    }
    Ok(source_output_file)
}

fn count_template_files(
    project_root: &Path,
    output_dir: &Path,
    exclude_dir_names: &[String],
    exclude_patterns: &[String],
    include_patterns: &[String],
) -> usize {
    let exclude_dir_names = exclude_dir_names.to_owned();
    ignore::WalkBuilder::new(project_root)
        .git_ignore(false)
        .hidden(false)
        .filter_entry(move |entry| {
            let name = entry.file_name().to_string_lossy();
            !exclude_dir_names.iter().any(|d| d == name.as_ref())
        })
        .build()
        .filter_map(Result::ok)
        .filter(|e| {
            let path = e.path();
            if path == project_root || path.is_dir() || path.starts_with(output_dir) {
                return false;
            }
            let Ok(rel_path) = path.strip_prefix(project_root) else {
                return false;
            };
            let rel_str = rel_path.to_string_lossy();
            let file_name_opt = e.file_name().to_str();
            !should_skip_file(&rel_str, file_name_opt, exclude_patterns, include_patterns)
        })
        .count()
}

fn build_exclude_patterns(kam_toml: &KamToml) -> Vec<String> {
    kam_toml
        .kam
        .build
        .as_ref()
        .and_then(|build| build.exclude.clone())
        .unwrap_or_default()
}

fn build_include_patterns(kam_toml: &KamToml) -> Vec<String> {
    kam_toml
        .kam
        .build
        .as_ref()
        .and_then(|build| build.include.clone())
        .unwrap_or_default()
}
