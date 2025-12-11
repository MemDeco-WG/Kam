use crate::errors::KamError;
use crate::template::TemplateCacheManager;
use colored::Colorize;
use std::collections::HashMap;
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

    // Extract .tar.gz files and also handle directory-based templates in the zip
    // We will collect top-level directories' entries and later extract them into the template cache
    let mut dir_groups: HashMap<String, Vec<usize>> = HashMap::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| KamError::CommandFailed(format!("Failed to read ZIP entry: {}", e)))?;

        let outpath = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        let outpath_str = outpath.to_string_lossy();

        // If the entry is a .tar.gz file, treat as a single template archive
        if outpath_str.ends_with(".tar.gz") {
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
        } else {
            // Non .tar.gz entry - consider grouping by top-level directory name
            if let Some(component) = outpath.components().next() {
                if let std::path::Component::Normal(os_str) = component {
                    if let Some(top) = os_str.to_str() {
                        dir_groups
                            .entry(top.to_string())
                            .or_insert_with(Vec::new)
                            .push(i);
                    }
                }
            }
        }
    }

    // Process grouped directory entries as templates
    for (top, indices) in dir_groups.into_iter() {
        let dest_dir = cache_dir.join(&top);

        // If template exists in cache and force is not set, skip
        if dest_dir.exists() && !force {
            println!(
                "{} Template '{}' already exists, skipping",
                "⊘".yellow(),
                top
            );
            skipped_count += 1;
            continue;
        }

        // Extract grouped entries into a temporary directory and then copy into cache
        let temp_dir = tempfile::tempdir().map_err(KamError::Io)?;
        for idx in indices {
            let mut file = archive
                .by_index(idx)
                .map_err(|e| KamError::CommandFailed(format!("Failed to read ZIP entry: {}", e)))?;

            let entry_path = match file.enclosed_name() {
                Some(p) => p.to_owned(),
                None => continue,
            };

            // Compute relative subpath under the top directory
            let subpath = match entry_path.strip_prefix(&top) {
                Ok(p) => p.to_owned(),
                Err(_) => entry_path.to_owned(),
            };

            let out_target = temp_dir.path().join(&subpath);

            if file.is_dir() {
                fs::create_dir_all(&out_target).map_err(KamError::Io)?;
            } else {
                if let Some(parent) = out_target.parent() {
                    fs::create_dir_all(parent).map_err(KamError::Io)?;
                }
                let mut outfile = fs::File::create(&out_target).map_err(KamError::Io)?;
                std::io::copy(&mut file, &mut outfile).map_err(KamError::Io)?;
            }
        }

        // Install into cache: remove existing (if any), then copy from temp
        if dest_dir.exists() {
            fs::remove_dir_all(&dest_dir).map_err(KamError::Io)?;
        }
        crate::utils::copy_dir_all(temp_dir.path(), &dest_dir).map_err(KamError::Io)?;

        println!("{} Template '{}' imported", "✓".green(), top);
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::FileOptions;

    #[test]
    #[serial]
    fn test_import_directory_template_from_zip() {
        // Create a temporary directory to build the ZIP file
        let tmp = tempdir().expect("create tmp");
        let zip_path = tmp.path().join("templates.zip");

        // Prepare a directory structure test_template/README.md
        let tmpl_dir = tmp.path().join("test_template");
        std::fs::create_dir_all(&tmpl_dir).expect("create tmpl dir");
        let readme_path = tmpl_dir.join("README.md");
        let mut rf = File::create(&readme_path).expect("create readme");
        rf.write_all(b"Hello from template").expect("write readme");

        // Create a ZIP file containing the test_template/ directory and files
        let zf = File::create(&zip_path).expect("create zip file");
        let mut zip_writer = zip::ZipWriter::new(zf);
        let options: FileOptions<()> = FileOptions::default();

        // Add directory entry and file entry
        // Directory entry intentionally omitted (file entries create necessary paths)
        zip_writer
            .start_file("test_template/README.md", options)
            .expect("start file");
        zip_writer
            .write_all(b"Hello from template")
            .expect("write file");
        zip_writer.finish().expect("finish zip");

        // Isolate template cache by setting KAM_TEMPLATE_CACHE_DIR to a temporary directory
        let cache_tmp = tempdir().expect("cache tmpdir");
        let old_cache_dir = std::env::var_os("KAM_TEMPLATE_CACHE_DIR");
        unsafe {
            std::env::set_var(
                "KAM_TEMPLATE_CACHE_DIR",
                cache_tmp.path().to_str().expect("cache tmp to str"),
            );
        }

        // Run the import
        let res = import_multiple_templates(&zip_path, true);
        assert!(
            res.is_ok(),
            "import_multiple_templates returned an error: {:?}",
            res
        );

        // Verify that the template was installed in the cache
        let cache_dir = TemplateCacheManager::get_cache_dir().expect("get cache dir");
        let installed_dir = cache_dir.join("test_template");
        assert!(
            installed_dir.exists(),
            "installed_dir does not exist: {:?}",
            installed_dir
        );

        // Check extracted file content
        let installed_readme = installed_dir.join("README.md");
        assert!(installed_readme.exists(), "installed README not found");
        let content = std::fs::read_to_string(installed_readme).expect("read installed README");
        assert!(content.contains("Hello from template"));

        // Restore KAM_TEMPLATE_CACHE_DIR (best-effort)
        if let Some(orig) = old_cache_dir {
            unsafe {
                std::env::set_var("KAM_TEMPLATE_CACHE_DIR", orig);
            }
        } else {
            unsafe {
                std::env::remove_var("KAM_TEMPLATE_CACHE_DIR");
            }
        }
    }
}
