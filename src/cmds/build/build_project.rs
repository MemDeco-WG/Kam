use crate::types::kam_toml::enums::ModuleType;

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

    // Use a high-level progress bar for the main build phases rather than a large ASCII banner
    // We'll create the main build progress bar after we know the module and template info.

    // Load kam.toml
    let kam_toml = if let Some(kt) = preloaded_kam_toml {
        kt
    } else {
        KamToml::load_from_dir(project_path)?
    };
    let module_id = &kam_toml.prop.id;
    let version = &kam_toml.prop.version;

    if !args.quiet {
        Utils::section(&format!("Building module: {} v{}", module_id, version));
        Utils::kv("Module", &format!("{} v{}", module_id, version));
    }

    let output_dir = determine_output_dir(&project_root, args, &kam_toml)?;
    if !args.quiet {
        Utils::kv("Output", &output_dir.display().to_string());
    }
    if !args.quiet {
        println!();
    }

    // Templates are exported as tar.gz and normally do not require build hooks executed.
    // Skip pre/post build hooks when packaging a template archive.
    let is_template_build = kam_toml.kam.module_type == ModuleType::Template;

    let total_steps = if is_template_build { 1 } else { 3 };
    let build_pb = if !args.quiet && std::io::stdout().is_terminal() {
        let pb = ProgressBar::new(total_steps as u64);
        let style = ProgressStyle::with_template("{spinner:.green} {msg} {pos}/{len}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner());
        pb.set_style(style);
        pb.set_message(format!("Building {} v{}", module_id, version));
        Some(pb)
    } else {
        None
    };

    // Run pre-build hooks only for non-template modules
    if !is_template_build {
        if let Some(pb) = &build_pb {
            pb.set_message("Running pre-build hooks");
        }
        run_pre_build_hooks(project_path, &kam_toml, &output_dir, args)?;
        if let Some(pb) = &build_pb {
            pb.inc(1);
        }
    } else {
        if !args.quiet {
            Utils::info(&format!("Skipping build hooks for template packaging"));
        }
    }

    // For non-interactive environments, print a section separator for packaging; if we have a top-level build progress bar show its message
    let show_top_build_progress = build_pb.is_some();
    if !show_top_build_progress && !args.quiet {
        Utils::section("Packaging artifacts...");
    }
    if let Some(pb) = &build_pb {
        pb.set_message("Packaging artifacts...");
    }

    let main_spinner = if !args.quiet {
        let pb = ProgressBar::new_spinner();
        let style = ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner());
        pb.set_style(style);
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        pb.set_message("Packaging artifacts...");
        Some(pb)
    } else {
        None
    };

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

    // Display build statistics
    if let Ok(metadata) = fs::metadata(&output_file) {
        let size_kb = metadata.len() as f64 / 1024.0;
        let size_str = if size_kb < 1024.0 {
            format!("{:.1} KB", size_kb)
        } else {
            format!("{:.1} MB", size_kb / 1024.0)
        };

        println!();
        Utils::kv(
            "Build time",
            &format!("{:.2}s", build_duration.as_secs_f64()),
        );
        Utils::kv("Package size", &size_str);
    }

    // Finish main spinner before running post-build hooks
    if let Some(pb) = main_spinner {
        pb.finish_and_clear();
    }

    // Run post-build hooks only for non-template modules
    if !is_template_build {
        if let Some(pb) = &build_pb {
            pb.set_message("Running post-build hooks");
        }
        run_post_build_hooks(project_path, &kam_toml, &output_dir, args)?;
        if let Some(pb) = &build_pb {
            pb.inc(1);
            pb.finish_with_message("Build complete");
        }
    }

    Ok(())
}

pub fn determine_basename(kam_toml: &KamToml) -> Result<String, KamError> {
    if let Some(build) = &kam_toml.kam.build {
        if let Some(output_file) = &build.output_file {
            if !output_file.is_empty() {
                let mut name = output_file.clone();
                name = name.replace("{{id}}", &kam_toml.prop.id);
                name = name.replace("{{version}}", &kam_toml.prop.version);
                name = name.replace("{{versionCode}}", &kam_toml.prop.versionCode.to_string());
                name = name.replace("{{name}}", &kam_toml.prop.get_name());
                return Ok(name);
            }
        }
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

    if !src_dir.exists() {
        if !args.quiet {
            Utils::warn(&format!(
                "Source directory not found: {}",
                src_dir.display()
            ));
        }
        // We allow building even if src dir is missing, but it might be empty
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

    // Check if module.prop already exists in src_dir (generated by pre-build hook)
    let module_prop_path = src_dir.join("module.prop");
    let module_prop_exists = module_prop_path.exists();

    if !module_prop_exists {
        // Generate module.prop if it doesn't exist
        if !args.quiet {
            Utils::info("Generating module.prop");
        }
        let mut prop_content = String::new();
        prop_content.push_str(&format!("id={}\n", kam_toml.prop.id));
        prop_content.push_str(&format!("name={}\n", kam_toml.prop.get_name()));
        prop_content.push_str(&format!("version={}\n", kam_toml.prop.version));
        prop_content.push_str(&format!("versionCode={}\n", kam_toml.prop.versionCode));
        prop_content.push_str(&format!("author={}\n", kam_toml.prop.author));
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
        Utils::info("Using existing module.prop (from pre-build hook)");
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

        let walker = ignore::WalkBuilder::new(&src_dir)
            .git_ignore(true)
            .hidden(false)
            .build();

        let pb = if args.quiet {
            None
        } else {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("  {spinner:.cyan} Packaging files: {pos} - {msg}")
                    .unwrap(),
            );
            pb.enable_steady_tick(std::time::Duration::from_millis(100));
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
                .map_err(|e| KamError::StripPrefixFailed(format!("strip_prefix: {}", e)))?;

            let rel_str = rel_path.to_string_lossy();

            // Check patterns using central matcher
            let file_name_opt = path.file_name().and_then(|s| s.to_str());

            let mut should_exclude = false;
            for pat in &exclude_patterns {
                if crate::utils::pattern_matches(pat, &rel_str, file_name_opt) {
                    should_exclude = true;
                    break;
                }
            }
            if should_exclude {
                let mut should_include = false;
                for pat in &include_patterns {
                    if crate::utils::pattern_matches(pat, &rel_str, file_name_opt) {
                        should_include = true;
                        break;
                    }
                }
                if !should_include {
                    continue;
                }
            }

            if path.is_dir() {
                zip.add_directory(rel_str.to_string(), options)?;
            } else if path.is_file() {
                // Update progress bar with current file (truncate if too long)
                let display_name = if rel_str.len() > 50 {
                    format!("...{}", &rel_str[rel_str.len() - 47..])
                } else {
                    rel_str.to_string()
                };
                if let Some(p) = &pb {
                    p.set_message(display_name);
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
            p.finish_with_message(format!("{} (Done)", count));
        }
    }

    // Add other files if they exist (readme, license, changelog)
    if let Some(mmrl) = &kam_toml.mmrl {
        if let Some(repo) = &mmrl.repo {
            let mut candidates: Vec<String> = Vec::new();
            if let Some(r) = &repo.readme {
                if !r.trim().is_empty() {
                    candidates.push(r.clone());
                }
            }
            if let Some(l) = &repo.license {
                if !l.trim().is_empty() {
                    candidates.push(l.clone());
                }
            }
            if let Some(c) = &repo.changelog {
                if !c.trim().is_empty() {
                    candidates.push(c.clone());
                }
            }
            // Also check for icon
            if let Some(i) = &repo.icon {
                if !i.trim().is_empty() {
                    candidates.push(i.clone());
                }
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
    }

    zip.finish()?;

    if !args.quiet {
        Utils::success(&format!(
            "Built Kam module: {}",
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
        build.exclude.clone().unwrap_or_else(|| Vec::new())
    } else {
        Vec::new()
    };

    let include_patterns: Vec<String> = if let Some(build) = _kam_toml.kam.build.as_ref() {
        build.include.clone().unwrap_or_else(|| Vec::new())
    } else {
        Vec::new()
    };

    // Use ignore::WalkBuilder to traverse all files, respecting .gitignore
    // For templates, we generally want to include hidden files (like .gitignore itself if needed, though ignore crate handles it)
    // But we should probably include everything that is not ignored by git.
    let exclude_dir_names = crate::utils::default_exclude_dir_names();

    let walker = ignore::WalkBuilder::new(project_root)
        .git_ignore(true)
        .hidden(false) // Templates might have hidden files
        .filter_entry(move |entry| {
            let name = entry.file_name().to_string_lossy();
            // Derived excluded directory names (from BuildSection defaults). Skip those.
            !exclude_dir_names.iter().any(|d| d == name.as_ref())
        })
        .build();

    let pb = if args.quiet {
        None
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("  {spinner:.cyan} Packaging files: {pos} - {msg}")
                .unwrap(),
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
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
            .map_err(|e| KamError::StripPrefixFailed(format!("strip_prefix: {}", e)))?;

        // Skip output directory
        if path.starts_with(output_dir) {
            continue;
        }

        // Check custom exclude/include using the central matcher
        let rel_str = rel_path.to_string_lossy();
        let file_name_opt = entry.file_name().to_str();
        let mut should_exclude = false;
        for pat in &exclude_patterns {
            if crate::utils::pattern_matches(pat, &rel_str, file_name_opt) {
                should_exclude = true;
                break;
            }
        }
        if should_exclude {
            let mut should_include = false;
            for pat in &include_patterns {
                if crate::utils::pattern_matches(pat, &rel_str, file_name_opt) {
                    should_include = true;
                    break;
                }
            }
            if !should_include {
                continue;
            }
        }

        if path.is_dir() {
            tar.append_dir(rel_path, path)?;
        } else if path.is_file() {
            // Update progress bar with current file (truncate if too long)
            let display_name = if rel_str.len() > 50 {
                format!("...{}", &rel_str[rel_str.len() - 47..])
            } else {
                rel_str.to_string()
            };
            if let Some(p) = &pb {
                p.set_message(display_name);
            }

            tar.append_path_with_name(path, rel_path)?;

            count += 1;
            if let Some(p) = &pb {
                p.set_position(count);
            }
        }
    }
    if let Some(p) = pb {
        p.finish_with_message(format!("{} (Done)", count));
    }

    tar.finish()?;

    if !args.quiet {
        Utils::success(&format!(
            "Built template archive: {}",
            source_output_file.display()
        ));
    }
    Ok(source_output_file)
}
