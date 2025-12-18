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
use terminal_size::{Width, terminal_size};

pub struct Utils;

// 默认的模板目录，到处都用这个
pub const PROJECT_TEMPLATE_DIRS: &[&str] = &["tmpl", "templates"];

// 支持的压缩包格式
pub const DEFAULT_ARCHIVE_EXTS: &[&str] = &[".tar.gz", ".tgz", ".zip", ".tar"];

// 返回默认的排除目录名列表
// 就是把那些带斜杠的路径模式转换成顶层目录名，方便检查
pub fn default_exclude_dir_names() -> Vec<String> {
    // 从BuildSection的默认值里拿排除列表
    // 这里主要是为了性能，避免每次都做完整的glob匹配
    let exclude_list = crate::types::kam_toml::sections::build::BuildSection::default()
        .exclude
        .unwrap_or_default();

    let mut names: Vec<String> = Vec::new();
    for pattern in exclude_list.into_iter() {
        let s_trim = pattern.trim_end_matches('/');
        if s_trim.is_empty() {
            continue;
        }
        let first = s_trim.split('/').next().unwrap().to_string();
        if !names.contains(&first) {
            names.push(first);
        }
    }

    // 再加几个常见的目录，这些通常也不应该被打包
    // 虽然理论上应该在build section里配置，但很多人会忘记
    for common in ["dist", "templates", "tmpl"].iter() {
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
    let path_norm = if let Some(stripped) = path.strip_prefix("./") {
        stripped
    } else {
        path
    };
    path_norm == prefix || path_norm.starts_with(&format!("{}/", prefix))
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
    let final_regex = format!("^{}$", regex_str);
    Regex::new(&final_regex)
        .map(|re| re.is_match(text))
        .unwrap_or(false)
}

fn matches_exact(pattern: &str, text: &str) -> bool {
    text == pattern
}

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

// 文件操作和打印相关的枚举
// 设计成小函数是为了复用，避免到处重复代码
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

/// Internal enum for log line classification
enum LogLevel<'a> {
    Warn(&'a str),
    Error(&'a str),
    Info(&'a str),
    Empty,
}

impl Utils {
    // 打印文件操作的状态信息
    // 纯展示用的，没有副作用，就是打印一下
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

    /// Helper function to format a centered title with decorative separators.
    /// Returns the formatted string and terminal width.
    fn format_centered_title(title: &str) -> (String, usize) {
        let width: usize = terminal_size()
            .map(|(Width(w), _)| w as usize)
            .unwrap_or(80);
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
        let formatted = format!("{}{}{}", left, title_text, right);
        (formatted, width)
    }

    /// Print a bold, centered banner to visually separate a logical operation.
    ///
    /// Uses a flower "✿" as a visual accent (梅花) and draws a simple separator.
    /// The banner attempt to center the title within an 80-column width; if the
    /// title is longer than the width it'll simply be printed without additional
    /// padding.
    pub fn banner(title: &str) {
        // Translate section/banners where possible before centering
        let (formatted, _) = Self::format_centered_title(crate::i18n::tr_key(title));
        println!("{}", formatted.cyan().bold());
        println!();
    }

    /// Print a "key: value" pair with a clear bullet icon and subtle value styling.
    pub fn kv(key: &str, value: &str) {
        let key_translated = crate::i18n::tr_key(key);
        println!(
            "  {} {}: {}",
            "•".cyan(),
            key_translated.bold(),
            value.dimmed()
        );
    }

    /// Print a compact section header with a horizontal separator below.
    ///
    /// This is intended for grouping output; it prints the title in bold cyan
    /// and a cyan horizontal line across the terminal width for readability.
    pub fn section(title: &str) {
        if title.is_empty() {
            return;
        }
        // Translate section titles (if we have mapping), then format/center them.
        let (formatted, _) = Self::format_centered_title(crate::i18n::tr_key(title));
        println!();
        println!("{}", formatted.cyan().bold());
        println!();
    }

    /// Print a generic informational line.
    pub fn info(msg: &str) {
        // Attempt to translate the message (if we can map it). Useful for static
        // phrases and common messages. For complex templates consider using `trf!`.
        let translated = crate::i18n::tr(msg);
        println!("  {} {}", "•".cyan(), translated);
    }

    /// Print an executing line for tasks such as scripts or commands being run.
    pub fn executing(msg: &str) {
        let translated = crate::i18n::tr(msg);
        println!("  {} {}", "→".blue(), translated);
    }

    /// Print a success line with a prominent green check.
    pub fn success(msg: &str) {
        let translated = crate::i18n::tr(msg);
        println!("{} {}", "✓".green().bold(), translated.green());
    }

    /// Print a warning line with yellow emphasis.
    /// Print a warning message in yellow.
    pub fn warn(msg: &str) {
        let translated = crate::i18n::tr(msg);
        println!("  {} {}", "!".yellow(), translated.yellow());
    }

    /// Return the configured error color from the global theme.
    pub(crate) fn error_color() -> Color {
        crate::colors::get_theme().error
    }

    pub fn error(msg: &str) {
        let translated = crate::i18n::tr(msg);
        let c = Self::error_color();
        eprintln!("{} {}", "✗".color(c).bold(), translated.color(c));
    }

    /// Classify a log line and return its log level type.
    /// This centralizes the classification logic used across multiple functions.
    fn classify_log_line<'a>(line: &'a str) -> LogLevel<'a> {
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
                LogLevel::Warn(msg) => Utils::warn(msg),
                LogLevel::Error(msg) => Utils::error(msg),
                LogLevel::Info(msg) => Utils::info(msg),
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
    pub fn print_cmd_line(line: &str) {
        match Self::classify_log_line(line) {
            LogLevel::Warn(msg) => Utils::warn(msg),
            LogLevel::Error(msg) => Utils::error(msg),
            LogLevel::Info(msg) => Utils::info(msg),
            LogLevel::Empty => {}
        }
    }

    /// Return a colored and formatted log line for streaming output
    ///
    /// This replicates the classification logic used by `print_cmd_line` but
    /// returns a colored string rather than printing it directly. It's useful
    /// for streaming log consumers that want to print through a progress bar
    /// or a logging queue while still preserving the same classification and color.
    pub fn format_cmd_line(line: &str) -> String {
        match Self::classify_log_line(line) {
            LogLevel::Warn(msg) => format!("  {} {}", "!".yellow(), msg.yellow()),
            LogLevel::Error(msg) => {
                let c = Self::error_color();
                format!("{} {}", "✗".color(c).bold(), msg.color(c))
            }
            LogLevel::Info(msg) => format!("  {} {}", "•".cyan(), msg),
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
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let s = String::from_utf8_lossy(&buf);
                            // Trim trailing newline for consistent formatting
                            let s_trim = s.trim_end_matches('\n');
                            if !s_trim.is_empty() {
                                Utils::print_cmd_line(s_trim);
                            }
                        }
                        Err(_) => break,
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
                // Only print header once before the first stderr line
                let mut printed_header = false;
                loop {
                    buf.clear();
                    match reader.read_until(b'\n', &mut buf) {
                        Ok(0) => break,
                        Ok(_) => {
                            let s = String::from_utf8_lossy(&buf);
                            let s_trim = s.trim_end_matches('\n');
                            if !s_trim.is_empty() {
                                if !printed_header {
                                    eprintln!("{}", "\n--- stderr ---".color(err_color).bold());
                                    printed_header = true;
                                }
                                eprintln!("{}", s_trim.color(err_color));
                            }
                        }
                        Err(_) => break,
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
}

/// Normalize a key into an environment-variable-friendly string.
///
/// - Upper-cases the input.
/// - Replaces '.' and '-' with underscores.
///   This helper centralizes the normalization logic used across the codebase
///   when converting kam.toml keys (e.g. `prop.id`) to environment variable fragments
///   (e.g. `PROP_ID`).
pub fn normalize_env_key(key: &str) -> String {
    key.to_ascii_uppercase().replace(['.', '-'], "_")
}

/// Convert a Kam-style key (e.g. `prop.id`) into a full `KAM_` environment
/// variable name (e.g. `KAM_PROP_ID`).
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
                } else {
                    return Err(KamError::InvalidDirectory(
                        "Cannot resolve home directory to expand KAM_HOME".to_string(),
                    ));
                }
            } else {
                return Ok(PathBuf::from(s));
            }
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
pub fn normalize_root_manager(raw: &str) -> String {
    let low = raw.trim().to_lowercase();
    if low.contains("magisk") {
        "Magisk".to_string()
    } else if low.contains("kernel") || low.contains("ksu") {
        "KernelSU".to_string()
    } else if low.contains("apatch") || low.contains("apd") || low.contains("apu") {
        "APatchSU".to_string()
    } else {
        "Unknown".to_string()
    }
}

// 确保path的父目录存在
// 如果path没有父目录（比如是根目录），就什么都做
pub fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

// 原子性地写入数据到path
// 先写到临时文件，然后rename，这样即使进程被中断也不会留下部分写入的文件
pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // 用简单的tmp扩展名，如果需要绝对唯一性，调用者应该自己保证
    let tmp = path.with_extension("kamtmp");
    let mut f = fs::File::create(&tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    fs::rename(&tmp, path)?;
    Ok(())
}

// 递归复制目录（从src到dst）
// - 目录会按需创建
// - 普通文件直接复制
// - 符号链接会尝试跟随并复制目标内容，如果目标不存在就尝试重新创建链接
//   这个函数主要是为了统一复制逻辑，避免到处重复实现
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

// 根据模块名计算索引路径（类似cargo的索引结构）
// 这个函数主要是为了分散文件，避免单个目录里文件太多
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

// 解压包（zip或tar.gz）
// 根据文件扩展名判断格式
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

// 为src目录里的所有文件在dst里创建符号链接（递归）
// 如果不支持创建符号链接，就回退到复制文件
// 这个函数主要是用来安装库文件到缓存（lib, lib64, bin等）
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use std::fs;

    #[test]
    fn test_normalize_root_manager() {
        assert_eq!(normalize_root_manager("magisk"), "Magisk");
        assert_eq!(normalize_root_manager("MagIsK"), "Magisk");
        assert_eq!(normalize_root_manager("ksu"), "KernelSU");
        assert_eq!(normalize_root_manager("KSU"), "KernelSU");
        assert_eq!(normalize_root_manager("apatch"), "APatchSU");
        assert_eq!(normalize_root_manager("apd"), "APatchSU");
        assert_eq!(normalize_root_manager("apu"), "APatchSU");
        assert_eq!(normalize_root_manager("APU"), "APatchSU");
        assert_eq!(normalize_root_manager("Iapu"), "APatchSU");
        assert_eq!(normalize_root_manager("something-else"), "Unknown");
    }

    #[test]
    #[serial]
    fn test_kam_home_dir_prefers_kam_home_env() {
        // Preserve original KAM_HOME then set a test value
        let orig = env::var("KAM_HOME").ok();
        let tmp = env::temp_dir().join("kam_home_test_env");
        unsafe { env::set_var("KAM_HOME", tmp.to_str().unwrap()) };
        let got = kam_home_dir().unwrap();
        assert_eq!(got, tmp);
        // restore original
        if let Some(v) = orig {
            unsafe { env::set_var("KAM_HOME", v) };
        } else {
            unsafe { env::remove_var("KAM_HOME") };
        }
    }

    #[test]
    #[serial]
    fn test_kam_home_dir_defaults_to_home_dot_kam() {
        // Ensure KAM_HOME is not set and the default is $HOME/.kam
        let orig = env::var("KAM_HOME").ok();
        unsafe { env::remove_var("KAM_HOME") };
        if let Some(home) = dirs::home_dir() {
            let expected = home.join(".kam");
            let got = kam_home_dir().unwrap();
            assert_eq!(got, expected);
        } else {
            // If home_dir() is not available on this platform, skip the assertion
        }
        if let Some(v) = orig {
            unsafe { env::set_var("KAM_HOME", v) };
        }
    }

    #[test]
    #[serial]
    fn test_kam_home_dir_expands_tilde() {
        // Save original envs
        let orig_kam = env::var("KAM_HOME").ok();
        let orig_home = env::var("HOME").ok();

        // Prepare a fake HOME so tilde expansion can be validated
        let fake_home = env::temp_dir().join("kam_home_tilde_test");
        fs::create_dir_all(&fake_home).unwrap();
        unsafe { env::set_var("HOME", fake_home.to_str().unwrap()) };
        unsafe { env::set_var("KAM_HOME", "~/kam_tilde_test") };

        // Only run the meaningful assertion if dirs::home_dir() actually reflects our fake HOME
        if dirs::home_dir().as_deref() == Some(fake_home.as_path()) {
            let expected = fake_home.join("kam_tilde_test");
            let got = kam_home_dir().unwrap();
            assert_eq!(got, expected);
        }

        // restore envs
        if let Some(v) = orig_home {
            unsafe { env::set_var("HOME", v) };
        } else {
            unsafe { env::remove_var("HOME") };
        }
        if let Some(v) = orig_kam {
            unsafe { env::set_var("KAM_HOME", v) };
        } else {
            unsafe { env::remove_var("KAM_HOME") };
        }
    }

    #[test]
    fn test_run_and_stream_basic() {
        // Prepare a small script that writes to stdout, stderr, and exits with code 3.
        let tmp = tempfile::tempdir().unwrap();
        let script_path = tmp.path().join("echo_both.sh");

        // Write script content in one shot to avoid requiring std::io::Write in scope.
        fs::write(
            &script_path,
            "#!/bin/sh\n\
             echo out\n\
             echo err >&2\n\
             exit 3\n",
        )
        .unwrap();

        // Ensure executable on Unix platforms
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        let mut cmd = std::process::Command::new(&script_path);
        // Keep stdin inherited so interactive commands continue to work when needed.
        cmd.stdin(std::process::Stdio::inherit());
        let status = Utils::run_and_stream(cmd).unwrap();
        assert_eq!(status.code(), Some(3));
    }

    #[test]
    fn test_format_cmd_line_includes_error_marker_and_theme_color() {
        // Ensure ANSI colors are enabled for this test so Colorize emits escape sequences.
        colored::control::set_override(true);
        let s = Utils::format_cmd_line("ERROR something");

        // Basic sanity: has the error marker and the original message
        assert!(s.contains("✗"), "expected error marker in formatted line");
        assert!(
            s.contains("something"),
            "expected original message in formatted line"
        );

        // The repository is a workspace that can run crates/tests in parallel and the
        // theme may be configured in workspace members. To make this test robust,
        // check that the produced line contains the ANSI color fragment that
        // corresponds to the currently configured theme color (whatever it is).
        let c = crate::colors::get_theme().error.clone();
        match c {
            colored::Color::TrueColor { r, g, b } => {
                // Match TrueColor SGR fragment like "38;2;R;G;B"
                let frag = format!("38;2;{};{};{}", r, g, b);
                assert!(
                    s.contains(&frag),
                    "expected theme truecolor fragment {} in output: {}",
                    frag,
                    s
                );
            }
            // For palette colors ensure at least some ANSI SGR escape is present.
            _ => {
                assert!(
                    s.contains("\x1b["),
                    "expected some ANSI escape sequence in output: {}",
                    s
                );
            }
        }
    }
}
