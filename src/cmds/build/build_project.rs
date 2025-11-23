use crate::types::kam_toml::enums::ModuleType;
use colored::*;
use glob::Pattern;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tar::Builder as TarBuilder;
use zip::{ZipWriter, write::FileOptions};

use super::args::BuildArgs;
use super::post_build::handle_post_build_hook;
use super::pre_build::handle_pre_build_hook;
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

    let output_dir = if target_dir.starts_with('/') || target_dir.contains(':') {
        PathBuf::from(target_dir)
    } else {
        project_root.join(target_dir)
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
    // Use project path as-is
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
    let basename = if let Some(build_cfg) = &kam_toml.kam.build
        && let Some(of) = &build_cfg.output_file
    {
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
            p.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&rendered)
                .to_string()
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
    if kam_toml.kam.module_type == ModuleType::Kam
        && !is_rendered_template
        && effective_src_dir.exists()
    {
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
        println!("  {} kam.toml", "+".green());

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
    _kam_toml: &KamToml,
    output_dir: &Path,
    basename: &str,
    effective_project_path: &Path,
    _project_path: &Path,
) -> Result<(), KamError> {
    // --- Create source tar.gz archive ---
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
    let walker = ignore::WalkBuilder::new(effective_project_path)
        .git_ignore(true)
        .hidden(match _kam_toml.kam.module_type {
            ModuleType::Template => false, // include hidden files for templates
            _ => true,                     // ignore hidden files for other module types
        })
        .build();

    for result in walker {
        let entry = result.map_err(|e| KamError::Io(std::io::Error::other(e)))?;
        let path = entry.path();

        // Skip the root directory itself
        if path == effective_project_path {
            continue;
        }

        let rel_path = path
            .strip_prefix(effective_project_path)
            .map_err(|e| KamError::StripPrefixFailed(format!("strip_prefix: {}", e)))?;

        // Skip .git directory and other unwanted paths
        if rel_path.starts_with(".git") || rel_path.starts_with(".kam") {
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
            // Add directory to tar archive
            tar.append_dir(rel_path, path)?;
            println!(
                "  {} {}/",
                "+".green(),
                rel_path.display().to_string().dimmed()
            );
        } else if path.is_file() {
            tar.append_path_with_name(path, rel_path)?;
            println!(
                "  {} {}",
                "+".green(),
                rel_path.display().to_string().dimmed()
            );
        }
    }

    // Add extra includes if specified
    if let Some(build) = _kam_toml.kam.build.as_ref()
        && let Some(extra_includes) = build.extra_includes.as_ref()
    {
        for include in extra_includes {
            let source_path = effective_project_path.join(&include.source);
            if source_path.exists() && source_path.is_file() {
                tar.append_path_with_name(&source_path, &include.dest)?;
                println!("  {} {}", "+".green(), include.dest.dimmed());
            } else {
                println!(
                    "  {} Extra include not found: {}",
                    "!".yellow(),
                    include.source
                );
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

/// Run a shell command and stream stdout/stderr as it arrives.
///
/// This implementation:
/// - Streams stdout and stderr concurrently using threads and an interleaving channel,
/// - Reads raw bytes using `BufRead::read_until(b'\n')` so we can cope with non-UTF8 outputs,
/// - Colorizes stderr lines to help spot issues in logs,
/// - Collects stderr bytes so we can present a meaningful error message when the command fails,
/// - Prints output as it arrives, ensuring timely log streaming for long-running commands.
pub fn run_command(cmd: &str, working_dir: &Path) -> Result<(), KamError> {
    use colored::Colorize;
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;

    // Spawn the child process and capture stdout/stderr
    let mut child = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", cmd])
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(KamError::from)?
    } else {
        Command::new("sh")
            .args(["-c", cmd])
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(KamError::from)?
    };

    let stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| KamError::CommandFailed("Failed to capture stdout".to_string()))?;
    let stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| KamError::CommandFailed("Failed to capture stderr".to_string()))?;

    // Channel carries (is_stdout, bytes) where bytes contains the raw chunk (usually a line)
    let (tx, rx) = mpsc::channel::<(bool, Vec<u8>)>();

    // Spawn a thread to stream stdout bytes using read_until
    {
        let tx = tx.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout_pipe);
            let mut buf = Vec::with_capacity(1024);
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        let _ = tx.send((true, buf.clone()));
                    }
                    Err(_) => break,
                }
            }
        });
    }

    // Spawn a thread to stream stderr bytes using read_until
    {
        let tx = tx.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr_pipe);
            let mut buf = Vec::with_capacity(1024);
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        let _ = tx.send((false, buf.clone()));
                    }
                    Err(_) => break,
                }
            }
        });
    }

    // Drop the sender on the main thread so rx.iter() completes once both reader threads end.
    drop(tx);

    // Keep an accumulator of stderr bytes for potential failure diagnostics.
    let mut stderr_acc: Vec<u8> = Vec::new();

    // Interleave and print outputs as they arrive
    for (is_stdout, bytes) in rx.iter() {
        if is_stdout {
            // Print stdout bytes lossy (handles non-UTF8 safely).
            let s = String::from_utf8_lossy(&bytes);
            print!("{}", s);
            let _ = std::io::stdout().flush();
        } else {
            // Colorize stderr text for clarity and accumulate bytes for diagnostics.
            stderr_acc.extend_from_slice(&bytes);
            let s = String::from_utf8_lossy(&bytes);
            eprintln!("{}", s.red());
            let _ = std::io::stderr().flush();
        }
    }

    // Reap the child process and return appropriate error if it failed.
    let status = child.wait().map_err(KamError::from)?;
    if !status.success() {
        let err_msg = if !stderr_acc.is_empty() {
            String::from_utf8_lossy(&stderr_acc).to_string()
        } else {
            format!("Command failed with status: {}", status)
        };
        return Err(KamError::CommandFailed(err_msg));
    }

    Ok(())
}
