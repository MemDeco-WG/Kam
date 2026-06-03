use crate::types::kam_toml::enums::ModuleType;

use comfy_table::{Cell, Table};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::IsTerminal;
use std::path::Path;
use std::time::Instant;

use super::args::BuildArgs;
use super::hooks::{run_post_build_hooks, run_pre_build_hooks};
pub use super::output::{determine_basename, determine_output_dir};
pub use super::package_module::create_kam_module_zip;
pub use super::package_template::create_template_archive;
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

/// # Errors
/// Returns `KamError` when build steps, I/O operations, or hooks fail.
pub fn build_project(
    project_path: &Path,
    args: &BuildArgs,
    preloaded_kam_toml: Option<KamToml>,
) -> Result<(), KamError> {
    let project_root = canonical_project_root(project_path)?;
    let project_path = project_root.as_path();
    let kam_toml = preloaded_kam_toml.map_or_else(|| KamToml::load_from_dir(project_path), Ok)?;
    let module_id = &kam_toml.prop.id;
    let display_version = display_version(&kam_toml.prop.version);
    let output_dir = determine_output_dir(&project_root, args, &kam_toml)?;

    print_build_header(args, module_id, &display_version, &output_dir);

    let is_template_build = kam_toml.kam.module_type == ModuleType::Template;
    let build_pb = build_progress(args, is_template_build, module_id, &display_version);

    run_pre_hooks_if_needed(
        args,
        build_pb.as_ref(),
        is_template_build,
        project_path,
        &kam_toml,
        &output_dir,
    )?;
    announce_packaging(args, build_pb.as_ref());

    let basename = determine_basename(&kam_toml)?;
    let start_time = Instant::now();
    let output_file = match kam_toml.kam.module_type {
        ModuleType::Kam => create_kam_module_zip(
            &kam_toml,
            &output_dir,
            &basename,
            module_id,
            project_path,
            args,
        )?,
        ModuleType::Template => {
            create_template_archive(&kam_toml, &output_dir, &basename, project_path, args)?
        }
    };
    let build_duration = start_time.elapsed();

    print_build_stats(args, &output_file, build_duration);
    run_post_hooks_if_needed(
        args,
        build_pb.as_ref(),
        is_template_build,
        project_path,
        &kam_toml,
        &output_dir,
    )?;

    Ok(())
}

fn canonical_project_root(project_path: &Path) -> Result<std::path::PathBuf, KamError> {
    project_path.canonicalize().map_err(|e| {
        KamError::InvalidDirectory(format!(
            "Failed to resolve project path '{}': {}",
            project_path.display(),
            e
        ))
    })
}

fn display_version(version: &str) -> String {
    if version.to_lowercase().starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    }
}

fn print_build_header(args: &BuildArgs, module_id: &str, display_version: &str, output_dir: &Path) {
    if args.quiet {
        return;
    }
    Utils::section(&trf!(
        "build.building_module_version",
        module_id,
        display_version
    ));

    let mut info_table = Table::new();
    info_table
        .load_preset(comfy_table::presets::UTF8_FULL)
        .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new(crate::i18n::tr("table.header.stat"))
                .fg(comfy_table::Color::Cyan)
                .add_attribute(comfy_table::Attribute::Bold),
            Cell::new(crate::i18n::tr("table.header.value"))
                .fg(comfy_table::Color::Cyan)
                .add_attribute(comfy_table::Attribute::Bold),
        ])
        .add_row(vec![
            Cell::new(crate::i18n::tr("table.header.module")).fg(comfy_table::Color::Cyan),
            Cell::new(format!("{module_id} {display_version}")).fg(comfy_table::Color::Green),
        ])
        .add_row(vec![
            Cell::new(crate::i18n::tr("project.output_directory")).fg(comfy_table::Color::Cyan),
            Cell::new(output_dir.display().to_string()).fg(comfy_table::Color::White),
        ]);

    println!("{info_table}");
    println!();
}

fn build_progress(
    args: &BuildArgs,
    is_template_build: bool,
    module_id: &str,
    display_version: &str,
) -> Option<ProgressBar> {
    if args.quiet || !std::io::stdout().is_terminal() {
        return None;
    }
    let total_steps = if is_template_build { 1 } else { 3 };
    let pb = ProgressBar::new(total_steps);
    let style = ProgressStyle::with_template(
        "{spinner:.green.bold} {msg:.bold} [{bar:40.cyan/blue}] {pos}/{len} {elapsed_precise}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_spinner())
    .progress_chars("█▉▊▋▌▍▎▏  ")
    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");
    pb.set_style(style);
    pb.set_message(trf!(
        "build.building_module_version",
        module_id,
        display_version
    ));
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    Some(pb)
}

fn run_pre_hooks_if_needed(
    args: &BuildArgs,
    build_pb: Option<&ProgressBar>,
    is_template_build: bool,
    project_path: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
) -> Result<(), KamError> {
    if is_template_build {
        if !args.quiet {
            Utils::info(crate::i18n::tr("hooks.skipping_template_packaging"));
        }
        return Ok(());
    }
    if let Some(pb) = build_pb {
        pb.set_message(crate::i18n::tr("hooks.running_pre"));
    }
    Utils::suspend_progressbar(build_pb, || {
        run_pre_build_hooks(project_path, kam_toml, output_dir, args)
    })?;
    if let Some(pb) = build_pb {
        pb.inc(1);
    }
    Ok(())
}

fn announce_packaging(args: &BuildArgs, build_pb: Option<&ProgressBar>) {
    if build_pb.is_none() && !args.quiet {
        Utils::section(crate::i18n::tr("build.packaging_artifacts"));
    }
    if let Some(pb) = build_pb {
        pb.set_message(crate::i18n::tr("build.packaging_artifacts"));
    }
}

fn print_build_stats(args: &BuildArgs, output_file: &Path, build_duration: std::time::Duration) {
    if args.quiet {
        return;
    }
    let Ok(metadata) = fs::metadata(output_file) else {
        return;
    };

    #[allow(clippy::cast_precision_loss)]
    let size_kilobytes = metadata.len() as f64 / 1024.0;
    let size_megabytes = size_kilobytes / 1024.0;
    let size_str = if size_kilobytes < 1024.0 {
        format!("{size_kilobytes:.1} KB")
    } else {
        format!("{size_megabytes:.1} MB")
    };

    println!();
    let mut table = Table::new();
    table
        .load_preset(comfy_table::presets::UTF8_FULL)
        .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new(crate::i18n::tr("project.header"))
                .fg(comfy_table::Color::Cyan)
                .add_attribute(comfy_table::Attribute::Bold),
            Cell::new(crate::i18n::tr("table.header.value"))
                .fg(comfy_table::Color::Cyan)
                .add_attribute(comfy_table::Attribute::Bold),
        ])
        .add_row(vec![
            Cell::new(crate::i18n::tr("project.build_time")).fg(comfy_table::Color::Cyan),
            Cell::new(format!("{:.2}s", build_duration.as_secs_f64()))
                .fg(comfy_table::Color::Green)
                .add_attribute(comfy_table::Attribute::Bold),
        ])
        .add_row(vec![
            Cell::new(crate::i18n::tr("project.package_size")).fg(comfy_table::Color::Cyan),
            Cell::new(&size_str)
                .fg(comfy_table::Color::Green)
                .add_attribute(comfy_table::Attribute::Bold),
        ])
        .add_row(vec![
            Cell::new(crate::i18n::tr("project.output_file")).fg(comfy_table::Color::Cyan),
            Cell::new(output_file.display().to_string()).fg(comfy_table::Color::White),
        ]);
    println!("{table}");
    println!();
}

fn run_post_hooks_if_needed(
    args: &BuildArgs,
    build_pb: Option<&ProgressBar>,
    is_template_build: bool,
    project_path: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
) -> Result<(), KamError> {
    if is_template_build {
        return Ok(());
    }
    if let Some(pb) = build_pb {
        pb.set_message(crate::i18n::tr("hooks.running_post"));
    }
    Utils::suspend_progressbar(build_pb, || {
        run_post_build_hooks(project_path, kam_toml, output_dir, args)
    })?;
    if let Some(pb) = build_pb {
        pb.inc(1);
        pb.finish_with_message(crate::i18n::tr("build.complete"));
    }
    Ok(())
}
