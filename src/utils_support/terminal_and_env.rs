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

