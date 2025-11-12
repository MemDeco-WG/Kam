use colored::*;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use tar::Builder as TarBuilder;
use zip::{ZipWriter, write::FileOptions};

use super::args::BuildArgs;
use super::post_build::handle_post_build_hook;
use super::pre_build::handle_pre_build_hook;
use crate::errors::kam::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::sections::ModuleType;

fn resolve_against_project(project_root: &Path, p: PathBuf) -> PathBuf {
    if p.is_absolute() {
        p
    } else {
        project_root.join(p)
    }
}

pub fn determine_output_dir(
    project_root: &Path,
    args: &BuildArgs,
    kam_toml: &KamToml,
) -> Result<PathBuf, KamError> {
    let output_dir: PathBuf = if let Some(out_cli) = &args.output {
        let pb = PathBuf::from(out_cli);
        resolve_against_project(project_root, pb)
    } else if let Some(build_cfg) = &kam_toml.kam.build {
        if let Some(t) = &build_cfg.target_dir {
            resolve_against_project(project_root, PathBuf::from(t))
        } else {
            project_root.join("dist")
        }
    } else {
        project_root.join("dist")
    };

    fs::create_dir_all(&output_dir)?;
    Ok(output_dir)
}

/// Build a single project
pub fn build_project(
    project_path: &Path,
    args: &BuildArgs,
    preloaded_kam_toml: Option<KamToml>,
) -> Result<(), KamError> {
    // Use project path as-is to avoid extended length paths on Windows
    let project_root = project_path.to_path_buf();

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

    handle_pre_build_hook(&kam_toml, project_path)?;

    // Package artifacts: produce two outputs
    // 1) module zip: a module archive (zip) containing kam.toml and module sources (if present) + mmrl files
    // 2) source tar.gz: a source archive (tar.gz) containing kam.toml and full source tree (if present)
    println!("{}", "Packaging artifacts...".bold());

    let (effective_project_path, is_rendered_template) =
        prepare_effective_project(project_path, &kam_toml, module_id, &output_dir)?;

    let basename = determine_basename(&kam_toml)?;

    create_module_zip_if_needed(
        &kam_toml,
        &output_dir,
        &basename,
        &effective_project_path,
        project_path,
        module_id,
        is_rendered_template,
    )?;

    create_source_archive(
        &kam_toml,
        &output_dir,
        &basename,
        &effective_project_path,
        project_path,
    )?;

    handle_post_build_hook(&kam_toml, project_path)?;

    Ok(())
}

pub fn prepare_effective_project(
    project_path: &Path,
    _kam_toml: &KamToml,
    _module_id: &str,
    _output_dir: &Path,
) -> Result<(PathBuf, bool), KamError> {
    let _src_dir = project_path.join("src").join(_module_id);

    // Build should not perform template rendering or variable replacement.
    // If src/<module_id> does not exist, proceed without it.
    let effective_project_path = project_path.to_path_buf();
    let is_rendered_template = false;
    Ok((effective_project_path, is_rendered_template))
}

pub fn determine_basename(kam_toml: &KamToml) -> Result<String, KamError> {
    // Determine module output basename. Default is `{{id}}-{{versionCode}}` as requested.
    let default_basename = format!("{}-{}", kam_toml.prop.id, kam_toml.prop.versionCode);

    // Read configured output_file (if any). The configured value must be a
    // filename WITHOUT extension. If an extension is present we warn and
    // ignore it. Placeholders like {{id}} are supported. The resolved basename
    // will be used for both module zip and source tar names.
    let basename = if let Some(build_cfg) = &kam_toml.kam.build {
        if let Some(of) = &build_cfg.output_file {
            let trimmed = of.trim();
            if trimmed.is_empty() {
                default_basename
            } else {
                let rendered = render_output_template(trimmed, kam_toml);
                let p = std::path::Path::new(&rendered);
                if p.extension().is_some() {
                    // Warn the user that extensions are not allowed in output_file
                    println!("{} {} {}", "Warning:".yellow().bold(), "kam.build.output_file should be a filename without extension; extension will be ignored:".yellow(), p.extension().unwrap().to_string_lossy().yellow());
                }
                let stem = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&rendered)
                    .to_string();
                stem
            }
        } else {
            default_basename
        }
    } else {
        default_basename
    };
    Ok(basename)
}

pub fn render_output_template(tpl: &str, kt: &KamToml) -> String {
    let mut s = tpl.to_string();
    s = s.replace("{{id}}", &kt.prop.id);
    s = s.replace("{{version}}", &kt.prop.version);
    s = s.replace("{{versionCode}}", &kt.prop.versionCode.to_string());
    s = s.replace("{{author}}", &kt.prop.author);
    s
}

pub fn create_module_zip_if_needed(
    kam_toml: &KamToml,
    output_dir: &Path,
    basename: &str,
    effective_project_path: &Path,
    project_path: &Path,
    module_id: &str,
    is_rendered_template: bool,
) -> Result<(), KamError> {
    let module_output_file = output_dir.join(format!("{}.zip", basename));

    // Only create a module zip when module_type == Kam. Other module types
    // must not be packaged as module zips even if `kam.build.output_file`
    // is provided.

    let effective_src_dir = effective_project_path.join("src").join(module_id);
    if kam_toml.kam.module_type == ModuleType::Kam && !is_rendered_template && effective_src_dir.exists() {
        // Create module zip archive
        let zip_file = File::create(&module_output_file)?;
        let mut zip = ZipWriter::new(zip_file);
        let options: FileOptions<()> = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        // Add kam.toml (from effective project path)
        zip.start_file("kam.toml", options)?;
        let kam_toml_content = fs::read_to_string(effective_project_path.join("kam.toml"))?;
        zip.write_all(kam_toml_content.as_bytes())?;
        println!("  {} {}", "+".green(), "kam.toml");

        // Add source files (module dir: src/<module_id>)
        // Since we checked effective_src_dir.exists(), we can add it directly
        add_directory_to_zip(
            &mut zip,
            &effective_src_dir,
            &format!("src/{}", module_id),
            &effective_src_dir,
        )?;

        // Add other files if they exist
        // Include files referenced in kam.toml (mmrl.repo): readme, license, changelog
        if let Some(mmrl) = &kam_toml.mmrl {
            if let Some(repo) = &mmrl.repo {
                let mut candidates: Vec<String> = vec![];
                if let Some(r) = &repo.readme {
                    if !r.trim().is_empty() {
                        candidates.push(r.clone())
                    }
                }
                if let Some(l) = &repo.license {
                    if !l.trim().is_empty() {
                        candidates.push(l.clone())
                    }
                }
                if let Some(c) = &repo.changelog {
                    if !c.trim().is_empty() {
                        candidates.push(c.clone())
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
            "{} Built module archive: {}",
            "✓".green().bold(),
            module_output_file.display().to_string().green()
        );
    } else {
        println!(
            "  {} {}",
            "•".cyan(),
            "Module type is not 'kam' — skipping module zip, only creating source archive".dimmed()
        );
    }
    Ok(())
}

pub fn create_source_archive(
    kam_toml: &KamToml,
    output_dir: &Path,
    basename: &str,
    effective_project_path: &Path,
    project_path: &Path,
) -> Result<(), KamError> {
    // --- Create source tar.gz archive ---
    let source_filename = format!("{}.tar.gz", basename);

    let source_output_file = output_dir.join(&source_filename);
    let tar_gz = File::create(&source_output_file)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = TarBuilder::new(enc);

    // Add kam.toml to tar (preserve as top-level file) from effective project path
    let mut file = File::open(effective_project_path.join("kam.toml"))?;
    let mut kam_content = Vec::new();
    file.read_to_end(&mut kam_content)?;
    let mut header = tar::Header::new_gnu();
    header.set_size(kam_content.len() as u64);
    header.set_mode(0o644);
    tar.append_data(&mut header, "kam.toml", Cursor::new(&kam_content))?;

    // Add src/ directory (if exists) - include entire src/ tree
    let full_src = effective_project_path.join("src");
    if full_src.exists() {
        tar.append_dir_all("src", &full_src)?;
    }

    // Include tmpl/ if exists
    let tmpl_dir = effective_project_path.join("tmpl");
    if tmpl_dir.exists() {
        tar.append_dir_all("tmpl", &tmpl_dir)?;
    }

    // Include mmrl repo referenced files (readme/license/changelog)
    if let Some(mmrl) = &kam_toml.mmrl {
        if let Some(repo) = &mmrl.repo {
            if let Some(r) = &repo.readme {
                let p = project_path.join(r);
                if p.exists() {
                    tar.append_path_with_name(p, r)?;
                }
            }
            if let Some(l) = &repo.license {
                let p = project_path.join(l);
                if p.exists() {
                    tar.append_path_with_name(p, l)?;
                }
            }
            if let Some(c) = &repo.changelog {
                let p = project_path.join(c);
                if p.exists() {
                    tar.append_path_with_name(p, c)?;
                }
            }
        }
    }

    // Finish tar (dropping will finish and flush)
    tar.finish()?;

    println!(
        "{} Built source archive: {}",
        "✓".green().bold(),
        source_output_file.display().to_string().green()
    );
    Ok(())
}

/// Add a directory to the zip archive recursively
pub fn add_directory_to_zip<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    dir: &Path,
    prefix: &str,
    base: &Path,
) -> Result<(), KamError> {
    let options: FileOptions<()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.strip_prefix(base).map_err(|e| {
            KamError::StripPrefixFailed(format!("failed to strip prefix {}: {}", base.display(), e))
        })?;
        let zip_path = format!("{}/{}", prefix, name.display());

        if path.is_file() {
            zip.start_file(&zip_path, options)?;
            let mut file = File::open(&path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            println!("  {} {}", "+".green(), zip_path.dimmed());
        } else if path.is_dir() {
            add_directory_to_zip(zip, &path, prefix, base)?;
        }
    }

    Ok(())
}

/// Run a shell command
pub fn run_command(cmd: &str, working_dir: &Path) -> Result<(), KamError> {
    use std::process::Command;

    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&["/C", cmd])
            .current_dir(working_dir)
            .output()
            .map_err(KamError::from)?
    } else {
        Command::new("sh")
            .args(&["-c", cmd])
            .current_dir(working_dir)
            .output()
            .map_err(KamError::from)?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(KamError::CommandFailed(stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        println!("{}", stdout);
    }

    Ok(())
}
