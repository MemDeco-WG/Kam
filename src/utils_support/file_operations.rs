/// Normalize a free-form root manager string into a canonical manager name.
///
/// Returns one of: "Magisk", "KernelSU", "APatchSU", or "Unknown".
/// Recognizes common aliases and variants (case-insensitive): magisk, ksu, kernel,
/// apatch, apd, apu, etc.
#[must_use]
pub fn normalize_root_manager(raw: &str) -> String {
    let low = raw.trim().to_lowercase();
    if low.contains("magisk") {
        "Magisk".to_string()
    } else if low.contains("kernel") || low.contains("ksu") {
        "KernelSU".to_string()
    } else if low.contains("apatch") || low.contains("ap") || low.contains("apu") {
        "APatchSU".to_string()
    } else {
        "Unknown".to_string()
    }
}

// 确保path的父目录存在
/// Ensure the parent directory of `path` exists.
///
/// If `path` has no parent (for example when it is a root path) this function
/// is a no-op. Creates missing parent directories using `create_dir_all`.
///
/// # Errors
/// Returns an I/O error when the parent directory cannot be created.
pub fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Atomically write `data` to `path`.
///
/// Data is written to a temporary file in the same directory (with an added
/// `.kamtmp` extension) and then renamed into place. This provides a best-effort
/// atomic write so that an interrupted write does not leave a partially written
/// destination file. Note: on some filesystems or when renaming across devices,
/// true atomicity is not guaranteed.
///
/// # Errors
/// Returns an I/O error when the parent directory, temporary file, write,
/// sync, or rename operation fails.
pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Use a `.kamtmp` extension for the temporary file. Callers requiring
    // stronger guarantees can implement their own temporary-file strategy.
    let tmp = path.with_extension("kamtmp");
    let mut f = fs::File::create(&tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Recursively copy all entries from `src` to `dst`.
///
/// - Directories are created as needed.
/// - Regular files are copied.
/// - Symlink entries are handled by attempting to follow and copy the target;
///   if the target cannot be resolved the function will try to recreate the
///   symlink or fall back to copying file contents where appropriate.
///
/// This centralizes recursive copy logic for use across the codebase. Note that
/// file permissions and ownership are attempted to be preserved where the OS
/// APIs support it; callers that require exact metadata preservation should
/// perform additional copy steps.
///
/// # Errors
/// Returns an I/O error when reading, creating, copying, or linking an entry
/// fails.
pub fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path)?;
        } else if file_type.is_symlink() {
            // 尝试跟随符号链接并复制目标内容
            if let Ok(target) = fs::read_link(&src_path) {
                if target.is_dir() {
                    copy_dir_all(&target, &dst_path)?;
                } else if target.is_file() {
                    fs::copy(&target, &dst_path)?;
                } else {
                    // 回退方案：尝试重新创建符号链接
                    // 在很多系统上这需要权限（特别是Windows）
                    #[cfg(unix)]
                    {
                        std::os::unix::fs::symlink(&target, &dst_path)?;
                    }
                    #[cfg(windows)]
                    {
                        if target.is_dir() {
                            std::os::windows::fs::symlink_dir(&target, &dst_path)?;
                        } else {
                            std::os::windows::fs::symlink_file(&target, &dst_path)?;
                        }
                    }
                }
            } else {
                // 如果读不到链接目标，就当作普通文件复制
                // 至少能保留一些内容
                fs::copy(&src_path, &dst_path)?;
            }
        } else {
            // 未知类型，尝试复制（虽然可能失败）
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Compute an index path for `module_name` under `index_base`.
///
/// The layout is similar to Cargo's index structure and disperses modules
/// across subdirectories to avoid excessive files in a single directory.
/// Returns a PathBuf relative to `index_base`.
#[must_use]
pub fn compute_index_path(index_base: &Path, module_name: &str) -> PathBuf {
    let name_lower = module_name.to_lowercase();
    let chars: Vec<char> = name_lower.chars().collect();

    match chars.len() {
        0 => index_base.to_path_buf(),
        1 => index_base.join("1").join(&name_lower),
        2 => index_base.join("2").join(&name_lower),
        3 => index_base
            .join("3")
            .join(chars[0].to_string())
            .join(&name_lower),
        _ => {
            let prefix1 = chars[0..2].iter().collect::<String>();
            let prefix2 = chars[2..4].iter().collect::<String>();
            index_base.join(&prefix1).join(&prefix2).join(&name_lower)
        }
    }
}

/// Extract a package archive into `dest`.
///
/// Supported formats:
/// - `.zip`
/// - `.tar.gz`, `.tgz`, `.tar`
///
/// # Errors
/// Returns `KamError::UnsupportedFormat` if the archive type is unsupported,
/// or a filesystem/archive error when opening or extracting fails.
pub fn extract_package(source: &Path, dest: &Path) -> Result<(), crate::errors::kam::KamError> {
    if path_has_ext(source, "zip") {
        let file = fs::File::open(source)?;
        let mut archive = zip::ZipArchive::new(file)?;
        archive.extract(dest)?;
    } else if path_has_tar_gz_suffix(source)
        || path_has_ext(source, "tgz")
        || path_has_ext(source, "tar")
    {
        let file = fs::File::open(source)?;
        let dec = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(dec);
        archive
            .unpack(dest)
            .map_err(|e| KamError::ExtractFailed(e.to_string()))?;
    } else {
        return Err(KamError::UnsupportedFormat(
            "Package must be .zip or .tar.gz format".to_string(),
        ));
    }
    Ok(())
}

/// Create symbolic links in `dst` for all entries under `src`, recursively.
///
/// On platforms or environments where symlink creation is not supported the
/// function falls back to copying files so that the intended content is
/// available in `dst`.
///
/// Commonly used to install library files into a cache (e.g. `lib`, `lib64`,
/// `bin`) while preserving symlinks when possible.
///
/// # Errors
/// Returns an I/O error when directory creation, symlink creation, or fallback
/// copying fails.
pub fn symlink_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let ty = entry.file_type()?;
        if ty.is_dir() {
            // Try to create a directory symlink, fallback to recursion if not supported.
            #[cfg(unix)]
            {
                if std::os::unix::fs::symlink(&src_path, &dst_path).is_err() {
                    symlink_dir_all(&src_path, &dst_path)?;
                }
            }
            #[cfg(windows)]
            {
                if std::os::windows::fs::symlink_dir(&src_path, &dst_path).is_err() {
                    symlink_dir_all(&src_path, &dst_path)?;
                }
            }
        } else if ty.is_file() {
            #[cfg(unix)]
            {
                if std::os::unix::fs::symlink(&src_path, &dst_path).is_err() {
                    fs::copy(&src_path, &dst_path)?;
                }
            }
            #[cfg(windows)]
            {
                if std::os::windows::fs::symlink_file(&src_path, &dst_path).is_err() {
                    fs::copy(&src_path, &dst_path)?;
                }
            }
        } else if ty.is_symlink() {
            // Attempt to preserve existing symlinks when possible.
            let target = fs::read_link(&src_path)?;
            #[cfg(unix)]
            {
                if std::os::unix::fs::symlink(&target, &dst_path).is_err() {
                    // Fall back to copying
                    if target.is_dir() {
                        copy_dir_all(&target, &dst_path)?;
                    } else if target.is_file() {
                        fs::copy(&target, &dst_path)?;
                    }
                }
            }
            #[cfg(windows)]
            {
                if target.is_dir() {
                    if std::os::windows::fs::symlink_dir(&target, &dst_path).is_err() {
                        copy_dir_all(&target, &dst_path)?;
                    }
                } else {
                    if std::os::windows::fs::symlink_file(&target, &dst_path).is_err() {
                        fs::copy(&target, &dst_path)?;
                    }
                }
            }
        }
    }
    Ok(())
}
