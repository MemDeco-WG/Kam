use super::cache::KamCache;
use super::errors::KamError;
use colored::{Color, Colorize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub struct Utils;

/// File system and printing operations used by commands.
///
/// The functions in this module aim to be small and single-purpose so other
/// modules can reuse them (reduces duplication across the repository).
#[derive(Debug)]
pub enum PrintOp {
    Create { is_dir: bool },
    Update,
    Delete,
    Copy { from: String, to: String },
    Symlink { target: String, link_type: LinkType },
}

#[derive(Debug)]
pub enum LinkType {
    Soft,
    Hard,
}

impl Utils {
    /// Print a human-friendly status line for file operations.
    ///
    /// This is presentation-only and intentionally has no side effects other
    /// than printing. It's single-purpose so callers can reuse it consistently.
    pub fn print_status(path: &Path, rel: &str, op: PrintOp, force: bool) {
        if force || !path.exists() || matches!(op, PrintOp::Delete) {
            match op {
                PrintOp::Create { is_dir } => {
                    let color = if is_dir { Color::Blue } else { Color::Green };
                    println!("{}", format!("+ {}", rel).color(color));
                }
                PrintOp::Update => {
                    println!("{}", format!("~ {}", rel).color(Color::Yellow));
                }
                PrintOp::Delete => {
                    println!("{}", format!("- {}", rel).color(Color::Red));
                }
                PrintOp::Copy { from, to } => {
                    println!(
                        "{}",
                        format!("{} -> {} (copy)", from, to).color(Color::Cyan)
                    );
                }
                PrintOp::Symlink { target, link_type } => {
                    let symbol = match link_type {
                        LinkType::Soft => "-->",
                        LinkType::Hard => "==>",
                    };
                    println!(
                        "{}",
                        format!("{} {} {} (symlink)", rel, symbol, target).color(Color::Magenta)
                    );
                }
            }
        } else {
            // For existing files without force, keep the noise minimal by printing a dim "update".
            println!("{}", format!("~ {}", rel).color(Color::Yellow));
        }
    }
}

/// Ensure the parent directory for `path` exists.
///
/// If `path` has no parent (e.g., root), this is a no-op.
pub fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

/// Atomically write `data` to `path`.
///
/// Implementation writes to a temporary file in the same directory and renames
/// it into place. This reduces the chance of partial writes when the process
/// is interrupted.
pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Use a simple tmp extension; if absolute uniqueness is required, callers
    // should provide further guarantees.
    let tmp = path.with_extension("kamtmp");
    let mut f = fs::File::create(&tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Recursively copy a directory from `src` -> `dst`.
///
/// - Directories are created as needed
/// - Regular files are copied
/// - For symlinks we attempt to follow the link and copy the target content
///   where possible; if the target doesn't exist we try to recreate the
///   symlink on platforms where that's possible.
///
/// This implementation mirrors the copy semantics used in other modules, but
/// centralizes the logic to avoid repeated re-implementations.
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
            // Try to follow the symlink and copy target contents.
            if let Ok(target) = fs::read_link(&src_path) {
                if target.is_dir() {
                    copy_dir_all(&target, &dst_path)?;
                } else if target.is_file() {
                    fs::copy(&target, &dst_path)?;
                } else {
                    // Fallback: try to recreate the symlink. On many systems this
                    // requires privileges (especially on Windows).
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
                // If we can't read the link target, fallback to copying the item
                // as regular file to preserve content where possible.
                fs::copy(&src_path, &dst_path)?;
            }
        } else {
            // Unknown type -> attempt to copy
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Compute index path based on module name (similar to cargo's index structure)
pub fn compute_index_path(index_base: &Path, module_name: &str) -> PathBuf {
    let name_lower = module_name.to_lowercase();
    let chars: Vec<char> = name_lower.chars().collect();

    match chars.len() {
        0 => index_base.to_path_buf(),
        1 => index_base.join("1").join(&name_lower),
        2 => index_base.join("2").join(&name_lower),
        3 => index_base
            .join("3")
            .join(&chars[0].to_string())
            .join(&name_lower),
        _ => {
            let prefix1 = chars[0..2].iter().collect::<String>();
            let prefix2 = chars[2..4].iter().collect::<String>();
            index_base.join(&prefix1).join(&prefix2).join(&name_lower)
        }
    }
}

/// Extract package archive (zip or tar.gz)
pub fn extract_package(source: &Path, dest: &Path) -> Result<(), crate::errors::kam::KamError> {
    let s = source.to_string_lossy().to_lowercase();

    if s.ends_with(".zip") {
        let file = fs::File::open(source)?;
        let mut archive = zip::ZipArchive::new(file)?;
        archive.extract(dest)?;
    } else if s.ends_with(".tar.gz") || s.ends_with(".tgz") || s.ends_with(".tar") {
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

/// Install library artifacts to cache (lib, lib64, bin)
///
/// This central utility installs files from a temporary module extraction
/// directory into the global cache. It mirrors the logic used in various
/// commands and avoids duplicate implementations.
pub fn install_library_artifacts(temp_path: &Path, cache: &KamCache) -> Result<(), KamError> {
    // Copy lib to cache/lib
    let src_lib = temp_path.join("lib");
    if src_lib.exists() {
        copy_dir_all(&src_lib, &cache.lib_dir())?;
    }

    // Copy lib64 to cache/lib64
    let src_lib64 = temp_path.join("lib64");
    if src_lib64.exists() {
        copy_dir_all(&src_lib64, &cache.lib64_dir())?;
    }

    // Copy bin to cache/bin
    let src_bin = temp_path.join("bin");
    if src_bin.exists() {
        copy_dir_all(&src_bin, &cache.bin_dir())?;
    }

    Ok(())
}

/// Create symlinks for all files in `src` inside `dst` recursively (where
/// supported). On platforms where creating symlinks is not permitted, the
/// function will fall back to copying files into place.
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
