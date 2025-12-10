use super::errors::KamError;
use colored::{Color, Colorize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use terminal_size::{terminal_size, Width};

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
    Skip,
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
    pub fn print_status(_path: &Path, rel: &str, op: PrintOp, _force: bool) {
        match op {
            PrintOp::Skip => {
                // Show skipped files in dim gray
                println!("{}", format!("- {}", rel).dimmed());
            }
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
    }

    /// Print a bold, centered banner to visually separate a logical operation.
    ///
    /// Uses a flower "✿" as a visual accent (梅花) and draws a simple separator.
    /// The banner attempt to center the title within an 80-column width; if the
    /// title is longer than the width it'll simply be printed without additional
    /// padding.
    pub fn banner(title: &str) {
        // Use the terminal size to render a centered banner dynamically.
        let width: usize = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);
        let title_text = format!(" ✿ {} ✿ ", title);
        let title_len = title_text.chars().count();

        let left_len = if width > title_len {
            (width - title_len) / 2
        } else {
            0
        };
        let right_len = if width > title_len {
            width - title_len - left_len
        } else {
            0
        };

        let left = "─".repeat(left_len);
        let right = "─".repeat(right_len);

        println!(
            "{}",
            format!("{}{}{}", left, title_text, right).cyan().bold()
        );
        println!();
    }

    /// Print a "key: value" pair with a clear bullet icon and subtle value styling.
    pub fn kv(key: &str, value: &str) {
        println!("  {} {}: {}", "•".cyan(), key.bold(), value.dimmed());
    }

    /// Print a compact section header with a horizontal separator below.
    ///
    /// This is intended for grouping output; it prints the title in bold cyan
    /// and a cyan horizontal line across the terminal width for readability.
    pub fn section(title: &str) {
        if title.is_empty() {
            return;
        }
        let width: usize = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);
        // We create a centered title with small decorative flower
        let title_text = format!(" ✿ {} ✿ ", title);
        let title_len = title_text.chars().count();
        let left_len = if width > title_len { (width - title_len) / 2 } else { 0 };
        let right_len = if width > title_len { width - title_len - left_len } else { 0 };
        let left = "─".repeat(left_len);
        let right = "─".repeat(right_len);
        println!("");
        println!("{}", format!("{}{}{}", left.cyan(), title_text.cyan().bold(), right.cyan()).bold());
        println!("");
    }

    /// Print a generic informational line.
    pub fn info(msg: &str) {
        println!("  {} {}", "•".cyan(), msg);
    }

    /// Print an executing line for tasks such as scripts or commands being run.
    pub fn executing(msg: &str) {
        println!("  {} {}", "→".blue(), msg);
    }

    /// Print a success line with a prominent green check.
    pub fn success(msg: &str) {
        println!("{} {}", "✓".green().bold(), msg.green());
    }

    /// Print a warning line with yellow emphasis.
    /// Print a warning message in yellow.
    pub fn warn(msg: &str) {
        println!("  {} {}", "!".yellow(), msg.yellow());
    }

    /// Print an error message in bold red with context.
    pub fn error(msg: &str) {
        eprintln!("{} {}", "✗".red().bold(), msg.red());
    }

    /// Print stdout/stderr from a command execution in a readable form.
    ///
    /// Both `stdout` and `stderr` are accepted as byte slices to match the types
    /// returned by `std::process::Output`. They are printed lossily to avoid
    /// panics on non-UTF-8 bytes and to remain resilient across platforms.
    pub fn print_cmd_output(stdout: &[u8], stderr: &[u8]) {
        // Convert to string lossily to handle non-UTF8 bytes gracefully
        let s_out = String::from_utf8_lossy(stdout);
        let s_err = String::from_utf8_lossy(stderr);

        // Helper to classify and print a single line
        fn print_line<S: AsRef<str>>(line: S) {
            let l = line.as_ref().trim();
            if l.is_empty() {
                return;
            }
            let upper = l.to_ascii_uppercase();
            // Prefer mapping to structured prints: WARN -> warn, ERROR/FAIL -> error, otherwise info.
            if upper.contains("[WARN]") || upper.starts_with("WARN") || upper.contains("WARNING") {
                Utils::warn(l);
            } else if upper.contains("[ERROR]")
                || upper.starts_with("ERROR")
                || upper.contains("FAIL")
                || upper.contains("[ERR]")
            {
                Utils::error(l);
            } else {
                Utils::info(l);
            }
        }

        // Print stdout lines (map common prefixes to structured outputs)
        for line in s_out.lines() {
            print_line(line);
        }

        // Print stderr lines (treat as warnings/errors when applicable)
        for line in s_err.lines() {
            print_line(line);
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
