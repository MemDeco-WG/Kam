use crate::types::kam_toml::enums::ModuleType;
use colored::*;
use glob::Pattern;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tar::Builder as TarBuilder;
use zip::{ZipWriter, write::FileOptions};

use super::args::BuildArgs;
use super::hooks::{run_post_build_hooks, run_pre_build_hooks};
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;

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

    println!("{}", "Building module...".bold().cyan());
    println!();

    // Load kam.toml
    let kam_toml = if let Some(kt) = preloaded_kam_toml {
        kt
    } else {
        KamToml::load_from_dir(project_path)?
    };
    let module_id = &kam_toml.prop.id;
    let version = &kam_toml.prop.version;

    println!("  {} Module: {} v{}", "•".cyan(), module_id, version);

    let output_dir = determine_output_dir(&project_root, args, &kam_toml)?;
    println!(
        "  {} Output: {}",
        "•".cyan(),
        output_dir.display().to_string().dimmed()
    );
    println!();

    run_pre_build_hooks(project_path, &kam_toml, &output_dir, args)?;

    println!("{}", "Packaging artifacts...".bold());

    let basename = determine_basename(&kam_toml)?;

    let start_time = Instant::now();
    let output_file = match kam_toml.kam.module_type {
        ModuleType::Kam => {
            create_kam_module_zip(&kam_toml, &output_dir, &basename, project_path, module_id)?
        }
        ModuleType::Template => {
            create_template_archive(&kam_toml, &output_dir, &basename, project_path)?
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
        println!(
            "  {} Build time: {:.2}s",
            "•".cyan(),
            build_duration.as_secs_f64()
        );
        println!("  {} Package size: {}", "•".cyan(), size_str);
    }

    run_post_build_hooks(project_path, &kam_toml, &output_dir, args)?;

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
        "{}-{}",
        kam_toml.prop.id, kam_toml.prop.versionCode
    ))
}

pub fn create_kam_module_zip(
    kam_toml: &KamToml,
    output_dir: &Path,
    basename: &str,
    project_path: &Path,
    module_id: &str,
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
        println!(
            "  {} Source directory not found: {}",
            "!".yellow(),
            src_dir.display()
        );
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

    // Generate module.prop
    println!("  {} Generating module.prop", "+".green());
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
    zip.start_file("module.prop", options)?;
    zip.write_all(prop_content.as_bytes())?;

    // Add source files (module dir: src/<module_id>) flattened to root
    if src_dir.exists() {
        // Compile exclude and include patterns
        let exclude_patterns: Vec<Pattern> = if let Some(build) = kam_toml.kam.build.as_ref() {
            if let Some(excludes) = &build.exclude {
                excludes
                    .iter()
                    .filter_map(|p| Pattern::new(p).ok())
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let include_patterns: Vec<Pattern> = if let Some(build) = kam_toml.kam.build.as_ref() {
            if let Some(includes) = &build.include {
                includes
                    .iter()
                    .filter_map(|p| Pattern::new(p).ok())
                    .collect()
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

        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("  {spinner:.cyan} Packaging files: {pos} - {msg}")
                .unwrap(),
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

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

            // Check patterns
            let mut should_exclude = false;
            for pat in &exclude_patterns {
                if pat.matches(&rel_str) {
                    should_exclude = true;
                    break;
                }
            }
            if should_exclude {
                let mut should_include = false;
                for pat in &include_patterns {
                    if pat.matches(&rel_str) {
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
                pb.set_message(display_name);

                zip.start_file(rel_str.to_string(), options)?;
                let mut f = File::open(path).map_err(|e| {
                    KamError::Io(std::io::Error::new(
                        e.kind(),
                        format!("Failed to open source file '{}': {}", path.display(), e),
                    ))
                })?;
                std::io::copy(&mut f, &mut zip)?;

                count += 1;
                pb.set_position(count);
            }
        }
        pb.finish_with_message(format!("{} (Done)", count));
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
                    println!("  {} {}", "+".green(), file_name);
                }
            }
        }
    }

    zip.finish()?;

    println!();
    println!(
        "{} Built Kam module: {}",
        "✓".green().bold(),
        module_output_file.display().to_string().green()
    );
    Ok(module_output_file)
}

pub fn create_template_archive(
    _kam_toml: &KamToml,
    output_dir: &Path,
    basename: &str,
    project_root: &Path,
) -> Result<PathBuf, KamError> {
    let source_filename = format!("{}.tar.gz", basename);
    let source_output_file = output_dir.join(&source_filename);
    let tar_gz = File::create(&source_output_file)?;
    let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
    let mut tar = TarBuilder::new(enc);

    // Compile exclude and include patterns
    let exclude_patterns: Vec<Pattern> = if let Some(build) = _kam_toml.kam.build.as_ref() {
        if let Some(excludes) = &build.exclude {
            excludes
                .iter()
                .filter_map(|p| Pattern::new(p).ok())
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let include_patterns: Vec<Pattern> = if let Some(build) = _kam_toml.kam.build.as_ref() {
        if let Some(includes) = &build.include {
            includes
                .iter()
                .filter_map(|p| Pattern::new(p).ok())
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Use ignore::WalkBuilder to traverse all files, respecting .gitignore
    // For templates, we generally want to include hidden files (like .gitignore itself if needed, though ignore crate handles it)
    // But we should probably include everything that is not ignored by git.
    let walker = ignore::WalkBuilder::new(project_root)
        .git_ignore(true)
        .hidden(false) // Templates might have hidden files
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            // Explicitly exclude heavy directories that shouldn't be in templates
            name != ".git" && name != "node_modules" && name != "target" && name != ".kam"
        })
        .build();

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("  {spinner:.cyan} Packaging files: {pos} - {msg}")
            .unwrap(),
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

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

        // Check custom exclude/include
        let rel_str = rel_path.to_string_lossy();
        let mut should_exclude = false;
        for pat in &exclude_patterns {
            if pat.matches(&rel_str) {
                should_exclude = true;
                break;
            }
        }
        if should_exclude {
            let mut should_include = false;
            for pat in &include_patterns {
                if pat.matches(&rel_str) {
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
            pb.set_message(display_name);

            tar.append_path_with_name(path, rel_path)?;

            count += 1;
            pb.set_position(count);
        }
    }
    pb.finish_with_message(format!("{} (Done)", count));

    tar.finish()?;

    println!(
        "{} Built template archive: {}",
        "✓".green().bold(),
        source_output_file.display().to_string().green()
    );
    Ok(source_output_file)
}
