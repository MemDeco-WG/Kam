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
use crate::types::kam_toml::KamToml;

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
pub fn run(args: BuildArgs) -> Result<(), Box<dyn std::error::Error>> {
    let project_path = Path::new(&args.path);
    
    println!("{}", "Building module...".bold().cyan());
    println!();
    
    // Load kam.toml
    let kam_toml = KamToml::load_from_dir(project_path)?;
    let module_id = &kam_toml.prop.id;
    let version = &kam_toml.prop.version;
    
    println!("  {} Module: {}", "•".cyan(), format!("{} v{}", module_id, version).bold());
    
    // Determine output directory
    let output_dir = if let Some(output) = args.output {
        PathBuf::from(output)
    } else if let Some(build_config) = &kam_toml.kam.build {
        if let Some(target_dir) = &build_config.target_dir {
            PathBuf::from(target_dir)
        } else {
            project_path.join("dist")
        }
    } else {
        project_path.join("dist")
    };
    
    fs::create_dir_all(&output_dir)?;
    println!("  {} Output: {}", "•".cyan(), output_dir.display().to_string().dimmed());
    println!();
    
    // Run pre-build hook
    if let Some(build_config) = &kam_toml.kam.build {
        if let Some(pre_build) = &build_config.pre_build {
            println!("{}", "Running pre-build hook...".yellow());
            run_command(pre_build, project_path)?;
            println!();
        }
    }
    
    // Package source files
    println!("{}", "Packaging source files...".bold());
    
    let src_dir = project_path.join("src").join(module_id);
    if !src_dir.exists() {
        return Err(format!("Source directory not found: {}", src_dir.display()).into());
    }
    
    // Determine output filename
    let default_filename = format!("{}-{}.zip", module_id, version);
    let output_filename = if let Some(build_config) = &kam_toml.kam.build {
        build_config.output_file.as_deref().unwrap_or(&default_filename)
    } else {
        &default_filename
    };
    
    let output_file = output_dir.join(output_filename);
    
    // Create zip archive
    let zip_file = File::create(&output_file)?;
    let mut zip = ZipWriter::new(zip_file);
    let options: FileOptions<()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);
    
    // Add kam.toml
    zip.start_file("kam.toml", options)?;
    let kam_toml_content = fs::read_to_string(project_path.join("kam.toml"))?;
    zip.write_all(kam_toml_content.as_bytes())?;
    println!("  {} {}", "+".green(), "kam.toml");
    
    // Add source files
    add_directory_to_zip(&mut zip, &src_dir, &format!("src/{}", module_id), &src_dir)?;
    
    // Add other files if they exist
    let additional_files = vec!["README.md", "LICENSE"];
    for file_name in additional_files {
        let file_path = project_path.join(file_name);
        if file_path.exists() {
            zip.start_file(file_name, options)?;
            let mut file = File::open(&file_path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            println!("  {} {}", "+".green(), file_name);
        }
    }
    
    zip.finish()?;
    
    println!();
    println!("{} Built: {}", "✓".green().bold(), output_file.display().to_string().green());
    
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
) -> Result<(), Box<dyn std::error::Error>> {
    let options: FileOptions<()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);
    
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.strip_prefix(base)?;
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
fn run_command(cmd: &str, working_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;
    
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&["/C", cmd])
            .current_dir(working_dir)
            .output()?
    } else {
        Command::new("sh")
            .args(&["-c", cmd])
            .current_dir(working_dir)
            .output()?
    };
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Command failed: {}", stderr).into());
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        println!("{}", stdout);
    }
    
    Ok(())
}
