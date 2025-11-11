/// # Kam Build Command
///
/// Build and package modules according to `kam.toml` configuration.
///
/// ## Functionality
///
/// - Reads build configuration from `kam.toml`
/// - Packages source code from `src/` directory
/// - Outputs module archive to `dist/` directory
/// - Supports pre-build and post-build hooks
///
/// ## Example
///
/// ```bash
/// # Build the current project
/// kam build
///
/// # Build a specific project
/// kam build ./my-project
/// ```

use clap::Args;
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{Write, Read};
use zip::ZipWriter;
use zip::write::FileOptions;
use flate2::write::GzEncoder;
use flate2::Compression;
use tar::Builder as TarBuilder;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::sections::module::ModuleType;
use crate::errors::KamError;

/// Arguments for the build command
#[derive(Args, Debug)]
pub struct BuildArgs {
    /// Path to the project (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Output directory (default: dist)
    #[arg(short, long)]
    pub output: Option<String>,
}

/// Run the build command
///
/// ## Steps
///
/// 1. Load `kam.toml` configuration
/// 2. Run pre-build hook (if specified)
/// 3. Package source files
/// 4. Run post-build hook (if specified)
/// 5. Output to dist directory
pub fn run(args: BuildArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);
    // Resolve project root to an absolute path when possible so relative
    // `target_dir` values in kam.toml are interpreted relative to the
    // project directory (not the current working directory).
    let project_root = match project_path.canonicalize() {
        Ok(p) => p,
        Err(_) => project_path.to_path_buf(),
    };

    println!("{}", "Building module...".bold().cyan());
    println!();

    // Load kam.toml
    let kam_toml = KamToml::load_from_dir(project_path)?;
    let module_id = &kam_toml.prop.id;
    let version = &kam_toml.prop.version;

    println!("  {} Module: {}", "•".cyan(), format!("{} v{}", module_id, version).bold());

    // Determine output directory (prefer explicit CLI, then kam.build.target_dir, else default)
    // Helper: if a configured path is relative, resolve it against the
    // project's root directory. If absolute, keep as-is.
    fn resolve_against_project(project_root: &Path, p: PathBuf) -> PathBuf {
        if p.is_absolute() {
            p
        } else {
            project_root.join(p)
        }
    }

    let output_dir: PathBuf = if let Some(out_cli) = &args.output {
        let pb = PathBuf::from(out_cli);
        resolve_against_project(&project_root, pb)
    } else if let Some(build_cfg) = &kam_toml.kam.build {
        if let Some(t) = &build_cfg.target_dir {
            resolve_against_project(&project_root, PathBuf::from(t))
        } else {
            project_root.join("dist")
        }
    } else {
        project_root.join("dist")
    };

    fs::create_dir_all(&output_dir)?;
    println!("  {} Output: {}", "•".cyan(), output_dir.display().to_string().dimmed());
    println!();

    // Run pre-build hook (if provided and non-empty)
    if let Some(pre_build) = kam_toml
        .kam
        .build
        .as_ref()
        .and_then(|b| b.pre_build.as_ref())
        .filter(|s| !s.trim().is_empty())
    {
        println!("{}", "Running pre-build hook...".yellow());
        run_command(pre_build, project_path)?;
        println!();
    }

    // Package artifacts: produce two outputs
    // 1) module zip: a module archive (zip) containing kam.toml and module sources (if present) + mmrl files
    // 2) source tar.gz: a source archive (tar.gz) containing kam.toml and full source tree (if present)
    println!("{}", "Packaging artifacts...".bold());

    let src_dir = project_path.join("src").join(module_id);

    // If `src/<module_id>` does not exist but the template contains a
    // placeholder directory named "{{id}}", render it into a temporary
    // project directory so packaging can proceed without mutating the
    // working tree. We only perform a minimal render: copy `kam.toml`
    // and expand `src/{{id}}` -> `src/<module_id>`, replacing occurrences
    // of "{{id}}" in file contents.
    let mut effective_project_path = project_path.to_path_buf();
    let mut _temp_project: Option<tempfile::TempDir> = None;
    if !src_dir.exists() {
        let placeholder_dir = project_path.join("src").join("{{id}}");
        if placeholder_dir.exists() {
            println!("  {} {}", "•".cyan(), "Found template placeholder src/{{id}} — rendering into temporary project for packaging".dimmed());
            let td = tempfile::TempDir::new()?;
            // copy kam.toml
            let kam_content = std::fs::read_to_string(project_path.join("kam.toml"))?;
            std::fs::write(td.path().join("kam.toml"), kam_content.as_bytes())?;

            // ensure target src/<module_id>
            let target_src = td.path().join("src").join(module_id);
            std::fs::create_dir_all(&target_src)?;

            // recursively copy files from placeholder_dir into target_src
            // Support arbitrary placeholders like {{id}}/{{version}}/{{author}} by
            // building a replacements map from `kam.toml` and applying it to
            // both filenames and file contents.
            fn copy_and_replace(src: &std::path::Path, dst: &std::path::Path, kt: &KamToml) -> Result<(), KamError> {
                let mut replacements: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
                replacements.insert("id", kt.prop.id.clone());
                replacements.insert("version", kt.prop.version.clone());
                replacements.insert("versionCode", kt.prop.versionCode.to_string());
                replacements.insert("author", kt.prop.author.clone());

                for entry in std::fs::read_dir(src)? {
                    let entry = entry?;
                    let p = entry.path();
                    let file_name = entry.file_name();

                    // Render filename placeholders (e.g. "{{id}}.txt")
                    let file_name_str = file_name.to_string_lossy().to_string();
                    let mut rendered_name = file_name_str.clone();
                    for (k, v) in &replacements {
                        let placeholder = format!("{{{{{}}}}}", k);
                        if rendered_name.contains(&placeholder) {
                            rendered_name = rendered_name.replace(&placeholder, v);
                        }
                    }

                    let dst_path = dst.join(&rendered_name);
                    if p.is_dir() {
                        std::fs::create_dir_all(&dst_path)?;
                        copy_and_replace(&p, &dst_path, kt)?;
                    } else {
                        // read file, replace occurrences of any known placeholders in contents
                        let buf = std::fs::read(&p)?;
                        // treat as text replace; if binary, replacement is harmless if not present
                        if let Ok(s) = String::from_utf8(buf.clone()) {
                            let mut replaced = s;
                            for (k, v) in &replacements {
                                let placeholder = format!("{{{{{}}}}}", k);
                                if replaced.contains(&placeholder) {
                                    replaced = replaced.replace(&placeholder, v);
                                }
                            }
                            std::fs::write(&dst_path, replaced.as_bytes())?;
                        } else {
                            // binary file: copy as-is (filenames already rendered)
                            std::fs::copy(&p, &dst_path)?;
                        }
                    }
                }
                Ok(())
            }

            copy_and_replace(&placeholder_dir, &target_src, &kam_toml)?;

            effective_project_path = td.path().to_path_buf();
            _temp_project = Some(td);
        }
    }

    // If we rendered a template placeholder into a temporary project, treat
    // this build as a template packaging: only create the source archive
    // (no module zip) and name it without the `-src` suffix.
    let is_rendered_template = _temp_project.is_some();

    // Determine module output basename. Default is `{{id}}-{{versionCode}}` as requested.
    let default_basename = format!("{}-{}", module_id, kam_toml.prop.versionCode);

    // Helper: render simple template placeholders
    fn render_output_template(tpl: &str, kt: &KamToml) -> String {
        let mut s = tpl.to_string();
        s = s.replace("{{id}}", &kt.prop.id);
        s = s.replace("{{version}}", &kt.prop.version);
        s = s.replace("{{versionCode}}", &kt.prop.versionCode.to_string());
        s = s.replace("{{author}}", &kt.prop.author);
        s
    }

    // Read configured output_file (if any). The configured value must be a
    // filename WITHOUT extension. If an extension is present we warn and
    // ignore it. Placeholders like {{id}} are supported. The resolved basename
    // will be used for both module zip and source tar names.
    let (basename, output_file_provided) = if let Some(build_cfg) = &kam_toml.kam.build {
        if let Some(of) = &build_cfg.output_file {
            let trimmed = of.trim();
            if trimmed.is_empty() {
                (default_basename.clone(), false)
            } else {
                let rendered = render_output_template(trimmed, &kam_toml);
                let p = std::path::Path::new(&rendered);
                if p.extension().is_some() {
                    // Warn the user that extensions are not allowed in output_file
                    println!("{} {} {}", "Warning:".yellow().bold(), "kam.build.output_file should be a filename without extension; extension will be ignored:".yellow(), p.extension().unwrap().to_string_lossy().yellow());
                }
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(&rendered).to_string();
                (stem, true)
            }
        } else {
            (default_basename.clone(), false)
        }
    } else {
        (default_basename.clone(), false)
    };

    let module_output_file = output_dir.join(format!("{}.zip", &basename));

    // Only create a module zip when module_type == Kam. Other module types
    // must not be packaged as module zips even if `kam.build.output_file`
    // is provided.

    if kam_toml.kam.module_type == ModuleType::Kam && !is_rendered_template {
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

        // Add source files if present (module dir: src/<module_id>)
        // Use the effective project path for src lookup (may be a temp rendered project)
        let effective_src_dir = effective_project_path.join("src").join(module_id);
        if effective_src_dir.exists() {
            add_directory_to_zip(&mut zip, &effective_src_dir, &format!("src/{}", module_id), &effective_src_dir)?;
        } else {
            println!("  {} {}", "•".cyan(), "No src/<module_id> directory found; module zip will include kam.toml and repo files only".dimmed());
        }

        // Add other files if they exist
        // Include files referenced in kam.toml (mmrl.repo): readme, license, changelog
        if let Some(mmrl) = &kam_toml.mmrl {
            if let Some(repo) = &mmrl.repo {
                let mut candidates: Vec<String> = vec![];
                if let Some(r) = &repo.readme { if !r.trim().is_empty() { candidates.push(r.clone()) } }
                if let Some(l) = &repo.license { if !l.trim().is_empty() { candidates.push(l.clone()) } }
                if let Some(c) = &repo.changelog { if !c.trim().is_empty() { candidates.push(c.clone()) } }

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
        println!("{} Built module archive: {}", "✓".green().bold(), module_output_file.display().to_string().green());
    } else {
        println!("  {} {}", "•".cyan(), "Module type is not 'kam' — skipping module zip, only creating source archive".dimmed());
    }

    // --- Create source tar.gz archive ---
    let source_filename = format!("{}.tar.gz", &basename);

    let source_output_file = output_dir.join(&source_filename);
    let tar_gz = File::create(&source_output_file)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = TarBuilder::new(enc);

    // Add kam.toml to tar (preserve as top-level file) from effective project path
    tar.append_path_with_name(effective_project_path.join("kam.toml"), "kam.toml")?;

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
                if p.exists() { tar.append_path_with_name(p, r)?; }
            }
            if let Some(l) = &repo.license {
                let p = project_path.join(l);
                if p.exists() { tar.append_path_with_name(p, l)?; }
            }
            if let Some(c) = &repo.changelog {
                let p = project_path.join(c);
                if p.exists() { tar.append_path_with_name(p, c)?; }
            }
        }
    }

    // Finish tar (dropping will finish and flush)
    tar.finish()?;

    println!("{} Built source archive: {}", "✓".green().bold(), source_output_file.display().to_string().green());

    // Run post-build hook
    if let Some(build_config) = &kam_toml.kam.build {
        if let Some(post_build) = &build_config.post_build {
            println!();
            println!("{}", "Running post-build hook...".yellow());
            run_command(post_build, project_path)?;
        }
    }

    Ok(())
}

/// Add a directory to the zip archive recursively
fn add_directory_to_zip<W: Write + std::io::Seek>(
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
    let name = path.strip_prefix(base).map_err(|e| KamError::Other(format!("failed to strip prefix {}: {}", base.display(), e)))?;
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
fn run_command(cmd: &str, working_dir: &Path) -> Result<(), KamError> {
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
