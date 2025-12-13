use crate::errors::KamError;
use crate::template::TemplateCacheManager;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::{self, File};

use std::path::Path;
use tar::Builder;
use walkdir::WalkDir;
use zip::ZipWriter;
use zip::write::FileOptions;

// 导出单个模板到.tar.gz文件
// 如果模板是目录就打包，如果是文件就直接复制
pub fn export_single_template(
    template_name: &str,
    output_path: &Path,
    force: bool,
) -> Result<(), KamError> {
    // 检查输出文件是否已存在
    if output_path.exists() && !force {
        return Err(KamError::CommandFailed(format!(
            "Output file already exists: {}. Use --force to overwrite.",
            output_path.display()
        )));
    }

    // 在缓存里找模板
    let template_path =
        TemplateCacheManager::resolve_template_path(template_name)?.ok_or_else(|| {
            KamError::TemplateNotFound(format!("Template '{}' not found in cache", template_name))
        })?;

    // 创建父目录（如果需要的话）
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }

    let tar_file = File::create(output_path).map_err(KamError::Io)?;
    let encoder = GzEncoder::new(tar_file, Compression::default());
    let mut archive = Builder::new(encoder);

    if template_path.is_dir() {
        // 打包目录，递归遍历所有文件
        for entry in WalkDir::new(&template_path) {
            let entry = entry.map_err(|e| KamError::Io(e.into()))?;
            let path = entry.path();

            if path == template_path {
                continue;
            }

            let relative_path = path
                .strip_prefix(&template_path)
                .map_err(|e| KamError::StripPrefixFailed(e.to_string()))?;

            if path.is_file() {
                let mut file = File::open(path).map_err(KamError::Io)?;
                archive
                    .append_file(relative_path, &mut file)
                    .map_err(KamError::Io)?;
            } else if path.is_dir() {
                archive
                    .append_dir(relative_path, path)
                    .map_err(KamError::Io)?;
            }
        }
    } else if template_path.is_file() {
        // 如果已经是压缩包了，直接复制就行（不用再打包）
        fs::copy(&template_path, output_path).map_err(KamError::Io)?;
        use crate::utils::Utils;
        Utils::success(&format!(
            "Template '{}' exported to {}",
            template_name,
            output_path.display()
        ));
        return Ok(());
    }

    archive.finish().map_err(KamError::Io)?;

    use crate::utils::Utils;
    Utils::success(&format!(
        "Template '{}' exported to {}",
        template_name,
        output_path.display()
    ));

    Ok(())
}

// 导出多个模板到一个.zip文件
// 把多个模板打包成一个ZIP，方便分发
pub fn export_multiple_templates(
    template_names: &[String],
    output_path: &Path,
    force: bool,
) -> Result<(), KamError> {
    // Check if output file exists
    if output_path.exists() && !force {
        return Err(KamError::CommandFailed(format!(
            "Output file already exists: {}. Use --force to overwrite.",
            output_path.display()
        )));
    }

    // Create parent directory if needed
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }

    let file = File::create(output_path).map_err(KamError::Io)?;
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    let temp_dir = tempfile::tempdir().map_err(KamError::Io)?;
    let mut exported_count = 0;

    for template_name in template_names {
        // Find template in cache
        let template_path = match TemplateCacheManager::resolve_template_path(template_name)? {
            Some(path) => path,
            None => {
                use crate::utils::Utils;
                Utils::warn(&format!("Template '{}' not found, skipping", template_name));
                continue;
            }
        };

        // Create a temporary tar.gz for this template
        let temp_archive_path = temp_dir.path().join(format!("{}.tar.gz", template_name));
        let tar_file = File::create(&temp_archive_path).map_err(KamError::Io)?;
        let encoder = GzEncoder::new(tar_file, Compression::default());
        let mut archive = Builder::new(encoder);

        if template_path.is_dir() {
            // Archive directory
            for entry in WalkDir::new(&template_path) {
                let entry = entry.map_err(|e| KamError::Io(e.into()))?;
                let path = entry.path();

                if path == template_path {
                    continue;
                }

                let relative_path = path
                    .strip_prefix(&template_path)
                    .map_err(|e| KamError::StripPrefixFailed(e.to_string()))?;

                if path.is_file() {
                    let mut file = File::open(path).map_err(KamError::Io)?;
                    archive
                        .append_file(relative_path, &mut file)
                        .map_err(KamError::Io)?;
                } else if path.is_dir() {
                    archive
                        .append_dir(relative_path, path)
                        .map_err(KamError::Io)?;
                }
            }
            archive.finish().map_err(KamError::Io)?;
        } else if template_path.is_file() {
            // If it's already an archive, use it directly
            fs::copy(&template_path, &temp_archive_path).map_err(KamError::Io)?;
        }

        // Add the tar.gz to the zip
        let archive_name = format!("{}.tar.gz", template_name);
        zip.start_file(&archive_name, options)
            .map_err(|e| KamError::CommandFailed(format!("Failed to add file to ZIP: {}", e)))?;

        let mut temp_file = File::open(&temp_archive_path).map_err(KamError::Io)?;
        std::io::copy(&mut temp_file, &mut zip).map_err(KamError::Io)?;

        use crate::utils::Utils;
        Utils::success(&format!("Template '{}' added to archive", template_name));
        exported_count += 1;
    }

    zip.finish()
        .map_err(|e| KamError::CommandFailed(format!("Failed to finalize ZIP: {}", e)))?;

    use crate::utils::Utils;
    if exported_count > 0 {
        println!();
        Utils::success(&format!(
            "Successfully exported {} template(s) to {}",
            exported_count,
            output_path.display()
        ));
    } else {
        return Err(KamError::CommandFailed(
            "No templates were exported".to_string(),
        ));
    }

    Ok(())
}

// 主导出函数，检测输出格式然后调用相应的处理函数
// 根据输出文件扩展名判断是单个还是多个模板
pub fn export_template(
    template_names: &[String],
    output_path: &Path,
    force: bool,
) -> Result<(), KamError> {
    if template_names.is_empty() {
        return Err(KamError::CommandFailed(
            "No templates specified for export".to_string(),
        ));
    }

    let extension = output_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let path_str = output_path.to_string_lossy();

    if template_names.len() == 1 && (path_str.ends_with(".tar.gz") || extension == "tgz") {
        // Single template export
        export_single_template(&template_names[0], output_path, force)
    } else if extension == "zip" {
        // Multiple templates export
        export_multiple_templates(template_names, output_path, force)
    } else if template_names.len() > 1 {
        Err(KamError::CommandFailed(
            "Multiple templates require .zip output format".to_string(),
        ))
    } else {
        Err(KamError::CommandFailed(format!(
            "Unsupported output format. Use .tar.gz for single template or .zip for multiple templates"
        )))
    }
}
