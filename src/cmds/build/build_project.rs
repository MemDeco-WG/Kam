use crate::types::kam_toml::enums::ModuleType;

use comfy_table::{Cell, Table};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::IsTerminal;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tar::Builder as TarBuilder;
use zip::{ZipWriter, write::FileOptions};

use super::args::BuildArgs;
use super::hooks::{run_post_build_hooks, run_pre_build_hooks};
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

/// Check if a file should be skipped based on exclude/include patterns.
/// Returns true if the file should be skipped (excluded and not included).
fn should_skip_file(
    rel_path: &str,
    file_name: Option<&str>,
    exclude_patterns: &[String],
    include_patterns: &[String],
) -> bool {
    // Check if file matches any exclude pattern
    let is_excluded = exclude_patterns
        .iter()
        .any(|pat| crate::utils::pattern_matches(pat, rel_path, file_name));

    if is_excluded {
        // If excluded, check if it's also included (include overrides exclude)
        let is_included = include_patterns
            .iter()
            .any(|pat| crate::utils::pattern_matches(pat, rel_path, file_name));
        // Skip if excluded and not included
        !is_included
    } else {
        false
    }
}

pub fn determine_output_dir(
    project_root: &Path,
    _args: &BuildArgs,
    _kam_toml: &KamToml,
) -> Result<PathBuf, KamError> {
    let target_dir = _kam_toml
        .kam
        .build
        .as_ref()
        .and_then(|b| b.target_dir.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("dist");

    let output_dir = if Path::new(target_dir).is_absolute() {
        PathBuf::from(target_dir)
    } else {
        project_root.join(target_dir)
    };

    fs::create_dir_all(&output_dir)?;
    Ok(output_dir.canonicalize().unwrap_or(output_dir))
}

pub fn build_project(
    project_path: &Path,
    args: &BuildArgs,
    preloaded_kam_toml: Option<KamToml>,
) -> Result<(), KamError> {
    // Use project path as-is
    let project_root = project_path.canonicalize().map_err(|e| {
        KamError::InvalidDirectory(format!(
            "Failed to resolve project path '{}': {}",
            project_path.display(),
            e
        ))
    })?;
    let project_path = project_root.as_path();

    // 用进度条显示构建过程，比打印一堆ASCII字符好看多了
    // 等拿到模块和模板信息后再创建进度条
    // 虽然进度条有时候会卡住，但至少看起来专业一点（笑）

    // Load kam.toml
    let kam_toml = if let Some(kt) = preloaded_kam_toml {
        kt
    } else {
        KamToml::load_from_dir(project_path)?
    };
    let module_id = &kam_toml.prop.id;
    let version = &kam_toml.prop.version;

    let output_dir = determine_output_dir(&project_root, args, &kam_toml)?;

    if !args.quiet {
        Utils::section(&trf!("build.building_module_version", module_id, version));

        // Use a beautiful table to display build information
        let mut info_table = Table::new();
        info_table
            .load_preset(comfy_table::presets::UTF8_FULL)
            .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new(crate::i18n::tr_key("table.header.stat"))
                    .fg(comfy_table::Color::Cyan)
                    .add_attribute(comfy_table::Attribute::Bold),
                Cell::new(crate::i18n::tr_key("table.header.value"))
                    .fg(comfy_table::Color::Cyan)
                    .add_attribute(comfy_table::Attribute::Bold),
            ])
            .add_row(vec![
                Cell::new(crate::i18n::tr_key("table.header.module")).fg(comfy_table::Color::Cyan),
                Cell::new(format!("{} v{}", module_id, version)).fg(comfy_table::Color::Green),
            ])
            .add_row(vec![
                Cell::new(crate::i18n::tr_key("project.output_directory"))
                    .fg(comfy_table::Color::Cyan),
                Cell::new(output_dir.display().to_string()).fg(comfy_table::Color::White),
            ]);

        println!("{}", info_table);
        println!();
    }

    // 模板打包成tar.gz，通常不需要执行构建钩子
    // 所以如果是模板类型，就跳过pre/post build hooks
    let is_template_build = kam_toml.kam.module_type == ModuleType::Template;

    let total_steps = if is_template_build { 1 } else { 3 };
    let build_pb = if !args.quiet && std::io::stdout().is_terminal() {
        let pb = ProgressBar::new(total_steps as u64);
        let style = ProgressStyle::with_template(
            "{spinner:.green.bold} {msg:.bold} [{bar:40.cyan/blue}] {pos}/{len} {elapsed_precise}",
        )
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
        .progress_chars("█▉▊▋▌▍▎▏  ")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");
        pb.set_style(style);
        pb.set_message(trf!("build.building_module_version", module_id, version));
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    // Run pre-build hooks only for non-template modules
    if !is_template_build {
        if let Some(pb) = &build_pb {
            pb.set_message(crate::i18n::tr_key("hooks.running_pre"));
        }
        // Suspend the top-level progress bar while running pre-build hooks so any
        // interactive prompts or command output are not overwritten.
        Utils::suspend_progressbar(build_pb.as_ref(), || {
            run_pre_build_hooks(project_path, &kam_toml, &output_dir, args)
        })?;
        if let Some(pb) = &build_pb {
            pb.inc(1);
        }
    } else if !args.quiet {
        Utils::info(crate::i18n::tr_key("hooks.skipping_template_packaging"));
    }

    // For non-interactive environments, print a section separator for packaging; if we have a top-level build progress bar show its message
    let show_top_build_progress = build_pb.is_some();
    if !show_top_build_progress && !args.quiet {
        Utils::section(crate::i18n::tr_key("build.packaging_artifacts"));
    }
    if let Some(pb) = &build_pb {
        pb.set_message(crate::i18n::tr_key("build.packaging_artifacts"));
    }

    let basename = determine_basename(&kam_toml)?;

    let start_time = Instant::now();
    let output_file = match kam_toml.kam.module_type {
        ModuleType::Kam => create_kam_module_zip(
            &kam_toml,
            &output_dir,
            &basename,
            project_path,
            module_id,
            args,
        )?,
        ModuleType::Template => {
            create_template_archive(&kam_toml, &output_dir, &basename, project_path, args)?
        }
    };
    let build_duration = start_time.elapsed();

    // Display build statistics in a beautiful table
    if !args.quiet
        && let Ok(metadata) = fs::metadata(&output_file)
    {
        let size_kb = metadata.len() as f64 / 1024.0;
        let size_str = if size_kb < 1024.0 {
            format!("{:.1} KB", size_kb)
        } else {
            format!("{:.1} MB", size_kb / 1024.0)
        };

        println!();
        let mut table = Table::new();
        table
            .load_preset(comfy_table::presets::UTF8_FULL)
            .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new(crate::i18n::tr_key("project.header"))
                    .fg(comfy_table::Color::Cyan)
                    .add_attribute(comfy_table::Attribute::Bold),
                Cell::new(crate::i18n::tr_key("table.header.value"))
                    .fg(comfy_table::Color::Cyan)
                    .add_attribute(comfy_table::Attribute::Bold),
            ])
            .add_row(vec![
                Cell::new(crate::i18n::tr_key("project.build_time")).fg(comfy_table::Color::Cyan),
                Cell::new(format!("{:.2}s", build_duration.as_secs_f64()))
                    .fg(comfy_table::Color::Green)
                    .add_attribute(comfy_table::Attribute::Bold),
            ])
            .add_row(vec![
                Cell::new(crate::i18n::tr_key("project.package_size")).fg(comfy_table::Color::Cyan),
                Cell::new(&size_str)
                    .fg(comfy_table::Color::Green)
                    .add_attribute(comfy_table::Attribute::Bold),
            ])
            .add_row(vec![
                Cell::new(crate::i18n::tr_key("project.output_file")).fg(comfy_table::Color::Cyan),
                Cell::new(output_file.display().to_string()).fg(comfy_table::Color::White),
            ]);
        println!("{}", table);
        println!();
    }

    // Run post-build hooks only for non-template modules
    if !is_template_build {
        if let Some(pb) = &build_pb {
            pb.set_message(crate::i18n::tr_key("hooks.running_post"));
        }
        // Suspend the top-level progress bar while running post-build hooks so that
        // any interactive prompts or subtree command outputs remain visible to the user.
        Utils::suspend_progressbar(build_pb.as_ref(), || {
            run_post_build_hooks(project_path, &kam_toml, &output_dir, args)
        })?;
        if let Some(pb) = &build_pb {
            pb.inc(1);
            pb.finish_with_message(crate::i18n::tr_key("build.complete"));
        }
    }

    Ok(())
}

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

pub fn create_kam_module_zip(
    kam_toml: &KamToml,
    output_dir: &Path,
    basename: &str,
    project_path: &Path,
    module_id: &str,
    args: &BuildArgs,
) -> Result<PathBuf, KamError> {
    let module_output_file = output_dir.join(format!("{}.zip", basename));

    let src_dir = if let Some(build) = &kam_toml.kam.build {
        if let Some(custom_src) = &build.source_dir {
            project_path.join(custom_src)
        } else {
            project_path.join("src").join(module_id)
        }
    } else {
        project_path.join("src").join(module_id)
    };

    if !src_dir.exists() && !args.quiet {
        Utils::warn(&trf!(
            "packaging.source_directory_not_found",
            src_dir.display()
        ));
    }
    // We allow building even if src dir is missing, but it might be empty

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

    // Check if module.prop already exists in src_dir (generated by pre-build hook)
    let module_prop_path = src_dir.join("module.prop");
    let module_prop_exists = module_prop_path.exists();

    if !module_prop_exists {
        // Generate module.prop if it doesn't exist
        if !args.quiet {
            Utils::info(crate::i18n::tr_key("packaging.generating_module_prop"));
        }
        let mut prop_content = String::new();
        prop_content.push_str(&format!("id={}\n", kam_toml.prop.id));
        prop_content.push_str(&format!("name={}\n", kam_toml.prop.get_name()));
        prop_content.push_str(&format!("version={}\n", kam_toml.prop.version));
        prop_content.push_str(&format!("versionCode={}\n", kam_toml.prop.versionCode));
        // author是可选的，有的话才写进去
        if let Some(author) = &kam_toml.prop.author {
            prop_content.push_str(&format!("author={}\n", author));
        }
        prop_content.push_str(&format!(
            "description={}\n",
            kam_toml.prop.get_description()
        ));
        if let Some(uj) = &kam_toml.prop.updateJson {
            prop_content.push_str(&format!("updateJson={}\n", uj));
        }
        prop_content.push_str(&format!("metamodule={}\n", kam_toml.prop.metamodule));
        zip.start_file("module.prop", options)?;
        zip.write_all(prop_content.as_bytes())?;
    } else if !args.quiet {
        Utils::info(crate::i18n::tr_key(
            "packaging.using_existing_module_prop_from_hook",
        ));
    }

    // Add source files (module dir: src/<module_id>) flattened to root
    if src_dir.exists() {
        // Compile exclude and include patterns as raw strings and use central matcher at runtime
        let exclude_patterns: Vec<String> = if let Some(build) = kam_toml.kam.build.as_ref() {
            if let Some(excludes) = &build.exclude {
                excludes.clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let include_patterns: Vec<String> = if let Some(build) = kam_toml.kam.build.as_ref() {
            if let Some(includes) = &build.include {
                includes.clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // First, count files to create a proper progress bar
        let file_count = {
            let temp_walker = ignore::WalkBuilder::new(&src_dir)
                .git_ignore(true)
                .hidden(false)
                .build();
            temp_walker
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let path = e.path();
                    if path == src_dir || path.is_dir() {
                        return false;
                    }
                    let rel_path = match path.strip_prefix(&src_dir) {
                        Ok(p) => p,
                        Err(_) => return false,
                    };
                    let rel_str = rel_path.to_string_lossy();
                    let file_name_opt = path.file_name().and_then(|s| s.to_str());
                    !should_skip_file(
                        &rel_str,
                        file_name_opt,
                        &exclude_patterns,
                        &include_patterns,
                    )
                })
                .count()
        };

        let walker = ignore::WalkBuilder::new(&src_dir)
            .git_ignore(true)
            .hidden(false)
            .build();

        let pb = if args.quiet {
            None
        } else {
            let pb = if file_count > 0 {
                ProgressBar::new(file_count as u64)
            } else {
                ProgressBar::new_spinner()
            };
            let style = if file_count > 0 {
                ProgressStyle::with_template(
                    "  {spinner:.cyan.bold} {msg:.dim} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)"
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
            pb.set_message(crate::i18n::tr_key("packaging.files"));
            Some(pb)
        };
        // Progress bar created only if not quiet

        let mut count = 0;
        for result in walker {
            let entry = result.map_err(|e| KamError::Io(std::io::Error::other(e)))?;
            let path = entry.path();

            if path == src_dir {
                continue;
            }

            let rel_path = path
                .strip_prefix(&src_dir)
                .map_err(|e| KamError::InvalidDirectory(format!("strip_prefix failed: {}", e)))?;

            let rel_str = rel_path.to_string_lossy();

            // Check patterns using central matcher
            let file_name_opt = path.file_name().and_then(|s| s.to_str());

            if should_skip_file(
                &rel_str,
                file_name_opt,
                &exclude_patterns,
                &include_patterns,
            ) {
                continue;
            }

            if path.is_dir() {
                zip.add_directory(rel_str.to_string(), options)?;
            } else if path.is_file() {
                // Update progress bar with current file (truncate if too long)
                let display_name = if rel_str.len() > 45 {
                    format!("...{}", &rel_str[rel_str.len() - 42..])
                } else {
                    rel_str.to_string()
                };
                if let Some(p) = &pb {
                    p.set_message(format!(
                        "{} {}",
                        crate::i18n::tr_key("packaging.packaging"),
                        display_name
                    ));
                }

                zip.start_file(rel_str.to_string(), options)?;
                let mut f = File::open(path).map_err(|e| {
                    KamError::Io(std::io::Error::new(
                        e.kind(),
                        format!("Failed to open source file '{}': {}", path.display(), e),
                    ))
                })?;
                std::io::copy(&mut f, &mut zip)?;

                count += 1;
                if let Some(p) = &pb {
                    p.set_position(count);
                }
            }
        }
        if let Some(p) = pb {
            p.finish_with_message(format!("✓ Packaged {} files", count));
        }
    }

    // Add other files if they exist (readme, license, changelog)
    if let Some(mmrl) = &kam_toml.mmrl
        && let Some(repo) = &mmrl.repo
    {
        let mut candidates: Vec<String> = Vec::new();
        if let Some(r) = &repo.readme
            && !r.trim().is_empty()
        {
            candidates.push(r.clone());
        }
        if let Some(l) = &repo.license
            && !l.trim().is_empty()
        {
            candidates.push(l.clone());
        }
        if let Some(c) = &repo.changelog
            && !c.trim().is_empty()
        {
            candidates.push(c.clone());
        }
        // Also check for icon
        if let Some(i) = &repo.icon
            && !i.trim().is_empty()
        {
            candidates.push(i.clone());
        }

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
    }

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

pub fn create_template_archive(
    _kam_toml: &KamToml,
    output_dir: &Path,
    basename: &str,
    project_root: &Path,
    args: &BuildArgs,
) -> Result<PathBuf, KamError> {
    let source_filename = format!("{}.tar.gz", basename);
    let source_output_file = output_dir.join(&source_filename);
    let tar_gz = File::create(&source_output_file)?;
    let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
    let mut tar = TarBuilder::new(enc);

    // Compile exclude and include patterns as raw strings and use the central matcher at runtime
    let exclude_patterns: Vec<String> = if let Some(build) = _kam_toml.kam.build.as_ref() {
        build.exclude.clone().unwrap_or_default()
    } else {
        Vec::new()
    };

    let include_patterns: Vec<String> = if let Some(build) = _kam_toml.kam.build.as_ref() {
        build.include.clone().unwrap_or_default()
    } else {
        Vec::new()
    };

    // Use ignore::WalkBuilder to traverse all files, respecting .gitignore
    // For templates, we generally want to include hidden files (like .gitignore itself if needed, though ignore crate handles it)
    // But we should probably include everything that is not ignored by git.
    let exclude_dir_names = crate::utils::default_exclude_dir_names();

    // Count files first for proper progress bar
    let file_count = {
        let exclude_dir_names_clone = exclude_dir_names.clone();
        let temp_walker = ignore::WalkBuilder::new(project_root)
            .git_ignore(true)
            .hidden(false)
            .filter_entry(move |entry| {
                let name = entry.file_name().to_string_lossy();
                !exclude_dir_names_clone.iter().any(|d| d == name.as_ref())
            })
            .build();
        temp_walker
            .filter_map(|e| e.ok())
            .filter(|e| {
                let path = e.path();
                if path == project_root || path.is_dir() {
                    return false;
                }
                let rel_path = match path.strip_prefix(project_root) {
                    Ok(p) => p,
                    Err(_) => return false,
                };
                if path.starts_with(output_dir) {
                    return false;
                }
                let rel_str = rel_path.to_string_lossy();
                let file_name_opt = e.file_name().to_str();
                !should_skip_file(
                    &rel_str,
                    file_name_opt,
                    &exclude_patterns,
                    &include_patterns,
                )
            })
            .count()
    };

    let walker = ignore::WalkBuilder::new(project_root)
        .git_ignore(true)
        .hidden(false)
        .filter_entry(move |entry| {
            let name = entry.file_name().to_string_lossy();
            !exclude_dir_names.iter().any(|d| d == name.as_ref())
        })
        .build();

    let pb = if args.quiet {
        None
    } else {
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
        pb.set_message(crate::i18n::tr_key("packaging.files"));
        Some(pb)
    };

    let mut count = 0;
    for result in walker {
        let entry = result.map_err(|e| KamError::Io(std::io::Error::other(e)))?;
        let path = entry.path();

        if path == project_root {
            continue;
        }

        // Calculate relative path from project_root
        let rel_path = path
            .strip_prefix(project_root)
            .map_err(|e| KamError::InvalidDirectory(format!("strip_prefix failed: {}", e)))?;

        // Skip output directory
        if path.starts_with(output_dir) {
            continue;
        }

        // Check custom exclude/include using the central matcher
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
            // Update progress bar with current file (truncate if too long)
            let display_name = if rel_str.len() > 45 {
                format!("...{}", &rel_str[rel_str.len() - 42..])
            } else {
                rel_str.to_string()
            };
            if let Some(p) = &pb {
                p.set_message(format!(
                    "{} {}",
                    crate::i18n::tr_key("packaging.packaging"),
                    display_name
                ));
            }

            tar.append_path_with_name(path, rel_path)?;

            count += 1;
            if let Some(p) = &pb {
                p.set_position(count);
            }
        }
    }
    if let Some(p) = pb {
        p.finish_with_message(format!("✓ Packaged {} files", count));
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
