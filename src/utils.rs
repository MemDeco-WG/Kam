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

impl Utils {
    /// Print the status of a file operation (create, update, delete, etc).
    ///
    /// This helper is display-only and does not perform file system changes.
    pub fn print_status(_path: &Path, rel: &str, op: PrintOp, _force: bool) {
        match op {
            PrintOp::Skip => {
                // Show skipped files in dim gray
                println!("{}", format!("- {rel}").dimmed());
            }
            PrintOp::Create { is_dir } => {
                let color = if is_dir { Color::Blue } else { Color::Green };
                println!("{}", format!("+ {rel}").color(color));
            }
            PrintOp::Update => {
                println!("{}", format!("~ {rel}").color(Color::Yellow));
            }
            PrintOp::Delete => {
                println!("{}", format!("- {rel}").color(Color::Red));
            }
            PrintOp::Copy { from, to } => {
                println!("{}", format!("{from} -> {to} (copy)").color(Color::Cyan));
            }
            PrintOp::Symlink { target, link_type } => {
                let symbol = match link_type {
                    LinkType::Soft => "-->",
                    LinkType::Hard => "==>",
                };
                println!(
                    "{}",
                    format!("{rel} {symbol} {target} (symlink)").color(Color::Magenta)
                );
            }
        }
    }

    /// Print a modern, compact banner (icon + colored title).
    ///
    /// Replaces the old boxed banner with a lightweight, Cargo/Starship-like style:
    /// - Icon accent (✨)
    /// - Bold cyan title
    /// - No heavy box drawing characters
    pub fn banner<S: AsRef<str>>(title: S) {
        let title_src = title.as_ref();
        let title_text = crate::i18n::tr(title_src);
        let title_trim = title_text.trim();

        // Skip empty or obvious placeholder titles.
        if title_trim.is_empty() || title_trim.eq_ignore_ascii_case("title") {
            return;
        }

        // If the caller passed a dotted translation key and the translated text
        // is identical to the key, the translation is missing — skip the banner.
        if title_src.contains('.') && title_text == title_src {
            return;
        }

        // Modern, lightweight banner: icon + colored bold title (no box)
        println!();
        println!("{} {}", "✨".yellow().bold(), title_text.bold().cyan());
        println!();
    }

    /// Print a "key: value" pair with a clear bullet icon and subtle value styling.
    pub fn kv<K: AsRef<str>, V: AsRef<str>>(key: K, value: V) {
        let key_translated = crate::i18n::tr(key.as_ref());
        println!(
            "  {} {}: {}",
            "•".cyan(),
            key_translated.bold(),
            value.as_ref().dimmed()
        );
    }

    /// Print a compact section header with a modern style (icon + colored title).
    ///
    /// Uses a lightweight layout instead of boxed ASCII art:
    /// - Icon accent (»)
    /// - Bold cyan title
    /// - No heavy box drawing characters
    pub fn section<S: AsRef<str>>(title: S) {
        let title_ref = title.as_ref();
        if title_ref.is_empty() {
            return;
        }

        // Translate and check for placeholders/missing translations. If the
        // translation is empty, literally "Title", or (when a dotted key was
        // passed) equals the key itself, we skip printing the section header.
        let title_text = crate::i18n::tr(title_ref);
        let title_trim = title_text.trim();
        if title_trim.is_empty() || title_trim.eq_ignore_ascii_case("title") {
            return;
        }
        if title_ref.contains('.') && title_text == title_ref {
            return;
        }

        // Modern lightweight section header (icon + bold cyan text)
        println!();
        println!("{} {}", "»".cyan().bold(), title_text.bold().cyan());
        println!();
    }

    /// Print a generic informational line.
    pub fn info<S: AsRef<str>>(msg: S) {
        // Attempt to translate the message (if we can map it). Useful for static
        // phrases and common messages. For complex templates consider using `trf!`.
        let translated = crate::i18n::tr(msg.as_ref());
        println!("  {} {}", "•".cyan(), translated);
    }

    /// Print an executing line for tasks such as scripts or commands being run.
    pub fn executing<S: AsRef<str>>(msg: S) {
        let translated = crate::i18n::tr(msg.as_ref());
        println!("  {} {}", "→".blue(), translated);
    }

    /// Print a success line with a prominent green check.
    ///
    /// Modern look: green check + neutral (uncolored) message text. Durations or
    /// secondary details should be printed in gray by callers when needed.
    pub fn success<S: AsRef<str>>(msg: S) {
        let translated = crate::i18n::tr(msg.as_ref());
        println!("{} {}", "✔".green().bold(), translated);
    }

    /// Print a warning line with yellow emphasis.
    /// Print a warning message in yellow.
    pub fn warn<S: AsRef<str>>(msg: S) {
        let translated = crate::i18n::tr(msg.as_ref());
        println!("  {} {}", "!".yellow(), translated.yellow());
    }

    /// Return the configured error color from the global theme.
    pub(crate) fn error_color() -> Color {
        crate::colors::get_theme().error
    }

    /// Print an error message using the configured error color from the theme.
    ///
    /// Message is printed to stderr with a leading colored '✗' marker.
    pub fn error<S: AsRef<str>>(msg: S) {
        let translated = crate::i18n::tr(msg.as_ref());
        let c = Self::error_color();
        eprintln!("{} {}", "✗".color(c).bold(), translated.color(c));
    }

    /// Classify a log line and return its log level type.
    /// This centralizes the classification logic used across multiple functions.
    fn classify_log_line(line: &str) -> LogLevel<'_> {
        let l = line.trim();
        if l.is_empty() {
            return LogLevel::Empty;
        }
        let upper = l.to_ascii_uppercase();
        if upper.contains("[WARN]") || upper.starts_with("WARN") || upper.contains("WARNING") {
            LogLevel::Warn(l)
        } else if upper.contains("[ERROR]")
            || upper.starts_with("ERROR")
            || upper.contains("FAIL")
            || upper.contains("[ERR]")
        {
            LogLevel::Error(l)
        } else {
            LogLevel::Info(l)
        }
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

        // Print stdout lines (map common prefixes to structured outputs)
        for line in s_out.lines() {
            match Self::classify_log_line(line) {
                LogLevel::Warn(msg) => Self::warn(msg),
                LogLevel::Error(msg) => Self::error(msg),
                LogLevel::Info(msg) => Self::info(msg),
                LogLevel::Empty => {}
            }
        }

        // Print stderr lines in 日系暖橙 (warm orange) to visually distinguish them from stdout.
        // Use an orange header and print each stderr line to the stderr stream in warm orange.
        if !s_err.is_empty() {
            let c = Self::error_color();
            eprintln!("{}", "\n--- stderr ---".color(c).bold());
            for line in s_err.lines() {
                eprintln!("{}", line.color(c));
            }
        }
    }

    /// Print a single stdout/stderr line using the same classification
    /// rules used by `print_cmd_output`. This is useful for streaming
    /// log consumers that read output line-by-line.
    pub fn print_cmd_line<S: AsRef<str>>(line: S) {
        let l = line.as_ref();
        match Self::classify_log_line(l) {
            LogLevel::Warn(msg) => Self::warn(msg),
            LogLevel::Error(msg) => Self::error(msg),
            LogLevel::Info(msg) => Self::info(msg),
            LogLevel::Empty => {}
        }
    }

    /// Return a colored and formatted log line for streaming output
    ///
    /// This replicates the classification logic used by `print_cmd_line` but
    /// returns a colored string rather than printing it directly. It's useful
    /// for streaming log consumers that want to print through a progress bar
    /// or a logging queue while still preserving the same classification and color.
    #[must_use]
    pub fn format_cmd_line(line: &str) -> String {
        match Self::classify_log_line(line) {
            LogLevel::Warn(msg) => format!("  {} {msg}", "!".yellow()),
            LogLevel::Error(msg) => {
                let c = Self::error_color();
                format!("{} {msg}", "✗".color(c).bold())
            }
            LogLevel::Info(msg) => format!("  {} {msg}", "•".cyan()),
            LogLevel::Empty => String::new(),
        }
    }

    /// Run a closure while suspending a progress bar if provided.
    ///
    /// This ensures CLI output (including interactive prompts) produced while the
    /// closure executes won't be overwritten by an active progress bar.
    /// The closure's result is returned unchanged.
    pub fn suspend_progressbar<F, R>(pb: Option<&ProgressBar>, op: F) -> R
    where
        F: FnOnce() -> R,
    {
        if let Some(pb) = pb {
            // Temporarily disable steady tick while we run the action to avoid
            // background updates interfering with output.
            pb.disable_steady_tick();
            let res = pb.suspend(op);
            pb.enable_steady_tick(Duration::from_millis(120));
            return res;
        }
        op()
    }

    /// Spawn a command with stdout/stderr piped and stream its output live.
    ///
    /// `cmd` should have stdin configured by the caller (e.g., inherit when
    /// interactive input is required). This helper will forcibly set stdout
    /// and stderr to piped and then spawn the process, streaming stdout lines
    /// (via `Utils::print_cmd_line`) and stderr lines (printed in red to the
    /// stderr stream). Returns the child's exit status when it finishes.
    ///
    /// # Errors
    /// Returns any I/O error raised while spawning the process, waiting for it,
    /// or configuring its stdout/stderr pipes.
    pub fn run_and_stream(mut cmd: std::process::Command) -> io::Result<std::process::ExitStatus> {
        // Ensure we have pipes for reading
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn()?;

        // Take pipes
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // stdout reader thread
        let out_handle = std::thread::spawn(move || {
            if let Some(out) = stdout {
                let mut reader = BufReader::new(out);
                let mut buf: Vec<u8> = Vec::new();
                loop {
                    buf.clear();
                    match reader.read_until(b'\n', &mut buf) {
                        Ok(0) | Err(_) => break, // EOF or read failure
                        Ok(_) => {
                            let s = String::from_utf8_lossy(&buf);
                            // Trim trailing newline for consistent formatting
                            let s_trim = s.trim_end_matches('\n');
                            if !s_trim.is_empty() {
                                Self::print_cmd_line(s_trim);
                            }
                        }
                    }
                }
            }
        });

        // stderr reader thread (prints in warm orange)
        let err_color = Self::error_color();
        let err_handle = std::thread::spawn(move || {
            if let Some(err) = stderr {
                let mut reader = BufReader::new(err);
                let mut buf: Vec<u8> = Vec::new();
                loop {
                    buf.clear();
                    match reader.read_until(b'\n', &mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            let s = String::from_utf8_lossy(&buf);
                            let s_trim = s.trim_end_matches('\n');
                            if !s_trim.is_empty() {
                                eprintln!("{}", s_trim.color(err_color));
                            }
                        }
                    }
                }
            }
        });

        // Wait for child to finish and for readers to complete
        let status = child.wait()?;
        let _ = out_handle.join();
        let _ = err_handle.join();
        Ok(status)
    }

    /// Convenience wrapper to preserve compatibility with callers that explicitly
    /// request no stderr header when streaming child processes. Historically some
    /// callers referenced a `run_and_stream_no_stderr_header` helper; add a thin
    /// delegating wrapper so those call sites compile and keep behavior stable.
    ///
    /// Note: the current `run_and_stream` implementation already streams stderr
    /// lines without printing a `--- stderr ---` separator, so this wrapper simply
    /// delegates to it.
    ///
    /// # Errors
    /// Returns any I/O error raised by `run_and_stream`.
    pub fn run_and_stream_no_stderr_header(
        cmd: std::process::Command,
    ) -> io::Result<std::process::ExitStatus> {
        Self::run_and_stream(cmd)
    }
}

/// Normalize a key into an environment-variable-friendly string.
///
/// - Upper-cases the input.
/// - Replaces '.' and '-' with underscores.
///   This helper centralizes the normalization logic used across the codebase
///   when converting kam.toml keys (e.g. `prop.id`) to environment variable fragments
///   (e.g. `PROP_ID`).
#[must_use]
pub fn normalize_env_key(key: &str) -> String {
    key.to_ascii_uppercase().replace(['.', '-'], "_")
}

/// Convert a Kam-style key (e.g. `prop.id`) into a full `KAM_` environment
/// variable name (e.g. `KAM_PROP_ID`).
#[must_use]
pub fn kam_env_var(key: &str) -> String {
    format!("KAM_{}", normalize_env_key(key))
}

/// Resolve the Kam home directory (the root directory used by Kam for global
/// configuration, caches, secrets, etc).
///
/// Behavior:
/// - If the environment variable `KAM_HOME` is set and non-empty, its value
///   is used as the Kam home directory. Leading `~` is expanded to the user's
///   home directory when possible (e.g., `~/kam` -> `/home/user/kam`).
/// - Otherwise the default is `$HOME/.kam`.
///
/// Returns `Ok(PathBuf)` on success or `Err(KamError::InvalidDirectory)` when the
/// user's home directory cannot be determined (and no KAM_HOME is set).
///
/// # Errors
/// Returns `KamError::InvalidDirectory` when no usable home directory can be
/// resolved.
pub fn kam_home_dir() -> Result<PathBuf, KamError> {
    // Prefer explicit KAM_HOME if provided
    if let Ok(val) = std::env::var("KAM_HOME") {
        let s = val.trim();
        if !s.is_empty() {
            // Expand leading `~` if present (best-effort)
            if s.starts_with('~') {
                // Handle "~" and "~/..." specially
                if let Some(home) = dirs::home_dir() {
                    if s == "~" {
                        return Ok(home);
                    }
                    // Prefer using strip_prefix("~/") so we don't manually slice the string.
                    if let Some(rest) = s.strip_prefix("~/") {
                        return Ok(home.join(rest));
                    }
                    // Fallback for cases like "~username" — treat as a literal path.
                    return Ok(PathBuf::from(s));
                }
                return Err(KamError::InvalidDirectory(
                    "Cannot resolve home directory to expand KAM_HOME".to_string(),
                ));
            }
            return Ok(PathBuf::from(s));
        }
    }

    // Fallback: $HOME/.kam
    let home = dirs::home_dir().ok_or_else(|| {
        KamError::InvalidDirectory("Could not determine home directory".to_string())
    })?;
    Ok(home.join(".kam"))
}

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
