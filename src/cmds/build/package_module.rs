use super::args::BuildArgs;
use super::package_filter::PackageFilter;
use super::shell_optimize::{PackageFile, ShellPackageOptimizer};
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::{ZipWriter, write::FileOptions};

/// # Errors
///
/// Returns `KamError` if any I/O, serialization, archive, or compression operations fail.
#[allow(clippy::too_many_lines)]
pub fn create_kam_module_zip(
    kam_toml: &KamToml,
    output_dir: &Path,
    basename: &str,
    module_id: &str,
    project_path: &Path,
    args: &BuildArgs,
) -> Result<PathBuf, KamError> {
    let module_output_file = output_dir.join(format!("{basename}.zip"));

    let src_dir = kam_toml.kam.build.as_ref().map_or_else(
        || project_path.join("src").join(module_id),
        |build| {
            build.source_dir.as_ref().map_or_else(
                || project_path.join("src").join(module_id),
                |custom_src| project_path.join(custom_src),
            )
        },
    );

    if !src_dir.exists() && !args.quiet {
        Utils::warn(&trf!(
            "packaging.source_directory_not_found",
            src_dir.display()
        ));
    }

    let zip_file = File::create(&module_output_file).map_err(|e| {
        KamError::Io(std::io::Error::new(
            e.kind(),
            format!(
                "Failed to create output zip file '{}': {}",
                module_output_file.display(),
                e
            ),
        ))
    })?;
    let mut zip = ZipWriter::new(zip_file);
    let options: FileOptions<()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    let module_prop_path = src_dir.join("module.prop");
    if !module_prop_path.exists() {
        if !args.quiet {
            Utils::info(crate::i18n::tr("packaging.generating_module_prop"));
        }
        let prop_content = module_prop_content(kam_toml)?;
        zip.start_file("module.prop", options)?;
        zip.write_all(prop_content.as_bytes())?;
    } else if !args.quiet {
        Utils::info(crate::i18n::tr(
            "packaging.using_existing_module_prop_from_hook",
        ));
    }

    if src_dir.exists() {
        add_module_source_files(kam_toml, args, &mut zip, options, &src_dir)?;
    }
    add_mmrl_files(kam_toml, args, &mut zip, options, project_path)?;
    zip.finish()?;

    if !args.quiet {
        println!();
        Utils::success(&trf!(
            "packaging.success_module_built",
            module_output_file.display()
        ));
    }
    Ok(module_output_file)
}

fn module_prop_content(kam_toml: &KamToml) -> Result<String, KamError> {
    let mut prop_content = String::new();
    std::fmt::Write::write_fmt(
        &mut prop_content,
        format_args!("id={id}\n", id = kam_toml.prop.id),
    )
    .map_err(|_| KamError::CommandFailed("failed to format module.prop".to_string()))?;
    std::fmt::Write::write_fmt(
        &mut prop_content,
        format_args!("name={name}\n", name = kam_toml.prop.get_name()),
    )
    .map_err(|_| KamError::CommandFailed("failed to format module.prop".to_string()))?;
    std::fmt::Write::write_fmt(
        &mut prop_content,
        format_args!("version={version}\n", version = kam_toml.prop.version),
    )
    .map_err(|_| KamError::CommandFailed("failed to format module.prop".to_string()))?;
    std::fmt::Write::write_fmt(
        &mut prop_content,
        format_args!(
            "versionCode={versionCode}\n",
            versionCode = kam_toml.prop.versionCode
        ),
    )
    .map_err(|_| KamError::CommandFailed("failed to format module.prop".to_string()))?;
    if let Some(author) = &kam_toml.prop.author {
        std::fmt::Write::write_fmt(&mut prop_content, format_args!("author={author}\n"))
            .map_err(|_| KamError::CommandFailed("failed to format module.prop".to_string()))?;
    }
    std::fmt::Write::write_fmt(
        &mut prop_content,
        format_args!(
            "description={desc}\n",
            desc = kam_toml.prop.get_description()
        ),
    )
    .map_err(|_| KamError::CommandFailed("failed to format module.prop".to_string()))?;
    if let Some(uj) = &kam_toml.prop.updateJson {
        std::fmt::Write::write_fmt(&mut prop_content, format_args!("updateJson={uj}\n"))
            .map_err(|_| KamError::CommandFailed("failed to format module.prop".to_string()))?;
    }
    std::fmt::Write::write_fmt(
        &mut prop_content,
        format_args!(
            "metamodule={metamodule}\n",
            metamodule = kam_toml.prop.metamodule
        ),
    )
    .map_err(|_| KamError::CommandFailed("failed to format module.prop".to_string()))?;
    Ok(prop_content)
}

fn add_module_source_files(
    kam_toml: &KamToml,
    args: &BuildArgs,
    zip: &mut ZipWriter<File>,
    options: FileOptions<()>,
    src_dir: &Path,
) -> Result<(), KamError> {
    let filter = PackageFilter::from_root(
        src_dir,
        build_exclude_patterns(kam_toml),
        build_include_patterns(kam_toml),
    );
    let mut files = collect_module_files(src_dir, &filter)?;
    let effective_args = effective_package_args(args, kam_toml);
    let mut optimizer = ShellPackageOptimizer::new(&effective_args, &kam_toml.prop.id);
    if optimizer.enabled() {
        optimizer.prepare(&files)?;
        files.retain(|file| optimizer.should_package(&file.rel_path));
    }
    let file_count = files.len();
    let pb = packaging_progress(args, file_count);
    let mut count = 0;

    for file in files {
        update_packaging_progress(pb.as_ref(), &file.rel_path);
        zip.start_file(file.rel_path.clone(), options)?;
        let content = if optimizer.enabled() {
            optimizer.transform_file(&file)?
        } else {
            file.content
        };
        zip.write_all(&content)?;
        count += 1;
        if let Some(p) = &pb {
            p.set_position(count);
        }
    }
    if let Some(p) = pb {
        p.finish_with_message(format!("✓ Packaged {count} files"));
    }
    Ok(())
}

fn effective_package_args(args: &BuildArgs, kam_toml: &KamToml) -> BuildArgs {
    let mut effective = args.clone();
    if let Some(build) = &kam_toml.kam.build {
        effective.trim_shell |= build.trim_shell.unwrap_or(false);
        effective.trim_shell_functions |= build.trim_shell_functions.unwrap_or(false);
        effective.obfuscate_shell |= build.obfuscate_shell.unwrap_or(false);
    }
    if effective.trim_shell_functions {
        effective.trim_shell = true;
    }
    effective
}

fn collect_module_files(
    src_dir: &Path,
    filter: &PackageFilter,
) -> Result<Vec<PackageFile>, KamError> {
    let walker = ignore::WalkBuilder::new(src_dir)
        .git_ignore(false)
        .hidden(false)
        .build();
    let mut files = Vec::new();

    for result in walker {
        let entry = result.map_err(|e| KamError::Io(std::io::Error::other(e)))?;
        let path = entry.path();
        if path == src_dir || path.is_dir() {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let rel_path = path
            .strip_prefix(src_dir)
            .map_err(|e| KamError::InvalidDirectory(format!("strip_prefix failed: {e}")))?;
        let rel_str = rel_path.to_string_lossy().to_string();
        let file_name_opt = path.file_name().and_then(|s| s.to_str());
        if filter.should_skip_file(&rel_str, file_name_opt) {
            continue;
        }
        let mut f = File::open(path).map_err(|e| {
            KamError::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to open source file '{}': {e}", path.display()),
            ))
        })?;
        let mut content = Vec::new();
        f.read_to_end(&mut content)?;
        files.push(PackageFile {
            rel_path: rel_str,
            content,
        });
    }

    Ok(files)
}

fn add_mmrl_files(
    kam_toml: &KamToml,
    args: &BuildArgs,
    zip: &mut ZipWriter<File>,
    options: FileOptions<()>,
    project_path: &Path,
) -> Result<(), KamError> {
    let Some(mmrl) = &kam_toml.mmrl else {
        return Ok(());
    };
    let Some(repo) = &mmrl.repo else {
        return Ok(());
    };

    let mut candidates = Vec::new();
    push_non_empty(&mut candidates, repo.readme.as_ref());
    push_non_empty(&mut candidates, repo.license.as_ref());
    push_non_empty(&mut candidates, repo.changelog.as_ref());
    push_non_empty(&mut candidates, repo.icon.as_ref());

    for file_name in candidates {
        let file_path = project_path.join(&file_name);
        if file_path.exists() {
            zip.start_file(&file_name, options)?;
            let mut file = File::open(&file_path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            if !args.quiet {
                Utils::info(&file_name);
            }
        }
    }
    Ok(())
}

fn push_non_empty(candidates: &mut Vec<String>, value: Option<&String>) {
    if let Some(value) = value
        && !value.trim().is_empty()
    {
        candidates.push(value.clone());
    }
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

pub(super) fn packaging_progress(args: &BuildArgs, file_count: usize) -> Option<ProgressBar> {
    if args.quiet {
        return None;
    }
    let pb = if file_count > 0 {
        ProgressBar::new(file_count as u64)
    } else {
        ProgressBar::new_spinner()
    };
    let style = if file_count > 0 {
        ProgressStyle::with_template(
            "  {spinner:.cyan.bold} {msg:.dim} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("█▉▊▋▌▍▎▏  ")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
    } else {
        ProgressStyle::with_template("  {spinner:.cyan.bold} {msg:.dim}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
    };
    pb.set_style(style);
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb.set_message(crate::i18n::tr("packaging.files"));
    Some(pb)
}

pub(super) fn update_packaging_progress(pb: Option<&ProgressBar>, rel_str: &str) {
    let display_name = if rel_str.len() > 45 {
        format!("...{}", &rel_str[rel_str.len() - 42..])
    } else {
        rel_str.to_string()
    };
    if let Some(p) = pb {
        p.set_message(format!(
            "{} {}",
            crate::i18n::tr("packaging.packaging"),
            display_name
        ));
    }
}

#[cfg(test)]
mod tests;
