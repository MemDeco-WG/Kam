use crate::errors::KamError;
use crate::template::TemplateCacheManager;
use colored::Colorize;
use std::fs;
use std::path::Path;
use zip::ZipArchive;

/// Import a single template from a .tar.gz file
pub fn import_single_template(
    archive_path: &Path,
    name: Option<String>,
    force: bool,
) -> Result<(), KamError> {
    if !archive_path.exists() {
        return Err(KamError::InvalidDirectory(format!(
            "Archive file not found: {}",
            archive_path.display()
        )));
    }

    // Determine template name
    let template_name = if let Some(n) = name {
        n
    } else {
        // Extract name from filename
        let filename = archive_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| {
                KamError::InvalidDirectory("Could not determine template name".to_string())
            })?;

        // Handle .tar.gz case where file_stem gives us "template.tar"
        let clean_name = if filename.ends_with(".tar") {
            filename.strip_suffix(".tar").unwrap_or(filename)
        } else {
            filename
        };

        clean_name.to_string()
    };

    let cache_dir = TemplateCacheManager::get_cache_dir()?;
    let dest_path = cache_dir.join(format!("{}.tar.gz", template_name));

    // Check if template already exists
    if dest_path.exists() && !force {
        return Err(KamError::CommandFailed(format!(
            "Template '{}' already exists. Use --force to overwrite.",
            template_name
        )));
    }

    // Copy the archive to cache directory
    fs::copy(archive_path, &dest_path).map_err(KamError::Io)?;

    println!(
        "{} Template '{}' imported successfully",
        "✓".green(),
        template_name.bold()
    );

    Ok(())
}

/// Import multiple templates from a .zip file
pub fn import_multiple_templates(zip_path: &Path, force: bool) -> Result<(), KamError> {
    if !zip_path.exists() {
        return Err(KamError::InvalidDirectory(format!(
            "ZIP file not found: {}",
            zip_path.display()
        )));
    }

    let file = fs::File::open(zip_path).map_err(KamError::Io)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| KamError::CommandFailed(format!("Failed to open ZIP archive: {}", e)))?;

    let cache_dir = TemplateCacheManager::get_cache_dir()?;
    let mut imported_count = 0;
    let mut skipped_count = 0;

    // Extract each .tar.gz file from the zip
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| KamError::CommandFailed(format!("Failed to read ZIP entry: {}", e)))?;

        let outpath = match file.enclosed_name() {
            Some(path) => path,
            None => continue,
        };

        // Only process .tar.gz files
        if !outpath.to_string_lossy().ends_with(".tar.gz") {
            continue;
        }

        let filename = outpath
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| KamError::InvalidDirectory("Invalid filename in ZIP".to_string()))?;

        let dest_path = cache_dir.join(filename);

        // Check if exists
        if dest_path.exists() && !force {
            println!(
                "{} Template '{}' already exists, skipping",
                "⊘".yellow(),
                filename.strip_suffix(".tar.gz").unwrap_or(filename)
            );
            skipped_count += 1;
            continue;
        }

        // Extract to cache directory
        let mut outfile = fs::File::create(&dest_path).map_err(KamError::Io)?;
        std::io::copy(&mut file, &mut outfile).map_err(KamError::Io)?;

        println!(
            "{} Template '{}' imported",
            "✓".green(),
            filename.strip_suffix(".tar.gz").unwrap_or(filename)
        );
        imported_count += 1;
    }

    if imported_count > 0 {
        println!(
            "\n{} Successfully imported {} template(s)",
            "✓".green(),
            imported_count
        );
    }

    if skipped_count > 0 {
        println!(
            "{} Skipped {} template(s) (already exist)",
            "⊘".yellow(),
            skipped_count
        );
    }

    if imported_count == 0 && skipped_count == 0 {
        println!("{} No .tar.gz templates found in ZIP file", "!".yellow());
    }

    Ok(())
}

/// Main import function that detects file type and calls appropriate handler
pub fn import_template(path: &Path, name: Option<String>, force: bool) -> Result<(), KamError> {
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    let path_str = path.to_string_lossy();

    if path_str.ends_with(".tar.gz") || extension == "tgz" {
        // Single template import
        import_single_template(path, name, force)
    } else if extension == "zip" {
        // Multiple templates import
        if name.is_some() {
            println!(
                "{} Note: --name is ignored when importing from ZIP (contains multiple templates)",
                "!".yellow()
            );
        }
        import_multiple_templates(path, force)
    } else {
        Err(KamError::CommandFailed(format!(
            "Unsupported file format. Use .tar.gz for single template or .zip for multiple templates"
        )))
    }
}
