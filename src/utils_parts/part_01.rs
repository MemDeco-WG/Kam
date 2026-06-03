use super::errors::KamError;
use colored::{Color, Colorize};

use indicatif::ProgressBar;
use regex::Regex;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Collection of small helper utilities used across the CLI and commands.
///
/// This type acts as a namespace for printing helpers, formatters, and other
/// lightweight utilities that do not carry state.
pub struct Utils;

/// Default folders to search for project templates.
pub const PROJECT_TEMPLATE_DIRS: &[&str; 2] = &["tmpl", "templates"];

/// Supported archive file extensions (used when inspecting archive files).
pub const DEFAULT_ARCHIVE_EXTS: &[&str; 4] = &[".tar.gz", ".tgz", ".zip", ".tar"];

fn path_has_ext(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(expected))
}

fn path_has_tar_gz_suffix(path: &Path) -> bool {
    path.to_string_lossy()
        .to_ascii_lowercase()
        .ends_with(".tar.gz")
}

/// Return the default list of directory names that should be excluded.
///
/// This list is computed from the build section defaults and extended with a
/// few commonly excluded directories (e.g., `dist`, `templates`, `tmpl`) so
/// that callers can efficiently check whether a top-level directory should be
/// ignored when packaging or scanning a project.
#[must_use]
pub fn default_exclude_dir_names() -> Vec<String> {
    // 从BuildSection的默认值里拿排除列表
    // 这里主要是为了性能，避免每次都做完整的glob匹配
    let exclude_list = crate::types::kam_toml::sections::build::BuildSection::default()
        .exclude
        .unwrap_or_default();

    let mut names: Vec<String> = Vec::new();
    for pattern in exclude_list {
        let s_trim = pattern.trim_end_matches('/');
        if s_trim.is_empty() {
            continue;
        }
        if let Some(first) = s_trim.split('/').next() {
            let first = first.to_string();
            if !names.contains(&first) {
                names.push(first);
            }
        }
    }

    // 再加几个常见的目录，这些通常也不应该被打包
    // 虽然理论上应该在build section里配置，但很多人会忘记
    for common in &["dist", "templates", "tmpl"] {
        if !names.iter().any(|n| n == common) {
            names.push(common.to_string());
        }
    }

    names
}

// 匹配include/exclude模式
// 支持的格式：
// - 以'/'结尾的当作目录前缀匹配
// - 包含'*'或'?'的转成正则表达式
// - '*.ext'这种后缀模式直接匹配文件名后缀

fn matches_directory_prefix(pattern: &str, path: &str) -> bool {
    let prefix = pattern.trim_end_matches('/');
    let path_norm = path.strip_prefix("./").map_or(path, |stripped| stripped);
    path_norm == prefix || path_norm.starts_with(&format!("{prefix}/"))
}

fn matches_suffix_wildcard(pattern: &str, file_name: &str) -> bool {
    if pattern.starts_with("*.") {
        file_name.ends_with(&pattern[1..])
    } else {
        false
    }
}

fn matches_wildcard(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') && !pattern.contains('?') {
        return false;
    }
    let mut regex_str = regex::escape(pattern);
    regex_str = regex_str.replace("\\*", ".*").replace("\\?", ".");
    let final_regex = format!("^{regex_str}$");
    Regex::new(&final_regex).is_ok_and(|re| re.is_match(text))
}

fn matches_exact(pattern: &str, text: &str) -> bool {
    text == pattern
}

/// Determine whether a pattern matches a relative path or optional file name.
///
/// Supported pattern formats:
/// - Directory prefix (ends with `/`)
/// - Suffix patterns like `*.ext`
/// - Glob-like patterns containing `*` or `?`
/// - Exact matches
#[must_use]
pub fn pattern_matches(pattern: &str, rel_path: &str, file_name: Option<&str>) -> bool {
    let patt = pattern.trim();
    let rel = rel_path.trim();

    // 目录前缀匹配
    if patt.ends_with('/') && matches_directory_prefix(patt, rel) {
        return true;
    }

    // 后缀通配符匹配
    if let Some(fname) = file_name
        && matches_suffix_wildcard(patt, fname)
    {
        return true;
    }

    // 通配符正则匹配
    if matches_wildcard(patt, rel) {
        return true;
    }
    if let Some(fname) = file_name
        && matches_wildcard(patt, fname)
    {
        return true;
    }

    // 精确匹配
    if matches_exact(patt, rel) {
        return true;
    }
    if let Some(fname) = file_name
        && matches_exact(patt, fname)
    {
        return true;
    }

    false
}

/// Return true if a command with the given `cmd` name exists in the PATH and is executable.
///
/// This simple helper checks each entry from the `PATH` environment variable.
/// On Unix-like platforms it additionally verifies the file is executable by inspecting
/// the permission bits. On non-Unix platforms existence is considered sufficient.
/// Return true if a command with the given `cmd` name exists in the PATH and is executable.
///
/// On Unix-like platforms the file's execute permission is checked; on non-Unix
/// platforms existence is considered sufficient.
#[must_use]
pub fn command_exists(cmd: &str) -> bool {
    if cmd.trim().is_empty() {
        return false;
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(cmd);
            if candidate.exists() {
                #[cfg(unix)]
                {
                    if let Ok(md) = candidate.metadata()
                        && md.permissions().mode() & 0o111 != 0
                    {
                        return true;
                    }
                }
                #[cfg(not(unix))]
                {
                    // On non-Unix platforms, existence is considered sufficient.
                    return true;
                }
            }
        }
    }

    false
}

/// Represents a file operation used for printing status messages to the user.
///
/// This enum is used by helpers to render what happened to a file or directory
/// (created, updated, deleted, copied, symlinked, or skipped).
#[derive(Debug)]
pub enum PrintOp {
    /// Creation of a file or directory.
    ///
    /// `is_dir` is true when the created entry is a directory (as opposed to a file).
    Create {
        /// Whether the created entry is a directory.
        is_dir: bool,
    },
    /// File or directory content updated.
    Update,
    /// File or directory deleted.
    Delete,
    /// File or directory copied from one location to another.
    Copy {
        /// The source path to copy from.
        from: String,
        /// The destination path to copy to.
        to: String,
    },
    /// A symbolic or hard link was created.
    Symlink {
        /// The target path the symlink points to.
        target: String,
        /// The type of link created (soft or hard).
        link_type: LinkType,
    },
    /// The file was skipped (no change).
    Skip,
}

/// Type of link used when creating a link on the filesystem.
#[derive(Debug)]
pub enum LinkType {
    /// A symbolic (soft) link.
    Soft,
    /// A hard link.
    Hard,
}

/// Internal enum for log line classification
enum LogLevel<'a> {
    Warn(&'a str),
    Error(&'a str),
    Info(&'a str),
    Empty,
}

