//! Detect and optionally fix CRLF / CR line endings (normalize to LF).
//!
//! - `run` will emit warnings for lines that contain CR (`\r`) characters,
//!   reporting the offending line numbers.
//! - `run_with_fix(..., do_fix = true)` will return a normalized String where
//!   all `\r\n` and standalone `\r` are converted to `\n`. The caller is
//!   expected to write the returned content back to disk when appropriate.
//!
//! This rule is safe to run on any text file; it only inspects textual
//! content and does not modify files by itself (the rules engine will apply
//! in-memory fixes and `check` will write changes when `--fix` is specified).

use std::path::Path;

use crate::cmds::check::file::FileResult;

/// Zero-sized rule type (stateless).
pub struct LineEndingsRule;

impl LineEndingsRule {
    fn has_cr(content: &str) -> bool {
        content.as_bytes().contains(&b'\r')
    }
}

impl crate::rules::Rule for LineEndingsRule {
    fn id(&self) -> &'static str {
        "line_endings"
    }

    fn description(&self) -> &'static str {
        "Detect CRLF / CR line endings and optionally normalize them to LF"
    }

    fn run(&self, path: &Path, content: &str, fr: &mut FileResult) {
        // Scan by splitting on '\n' so CR characters (if present) remain part of the line
        // (CRLF lines end with '\r' before the split).
        for (i, line) in content.split('\n').enumerate() {
            if line.ends_with('\r') || line.contains('\r') {
                // Show a concise hint rather than the whole line content.
                fr.warnings.push(format!(
                    "CR/LF line ending detected ({}: line {}): consider converting to LF",
                    path.display(),
                    i + 1
                ));
            }
        }

        // Basic quick check: the file contains CR anywhere (useful for files without per-line context)
        if Self::has_cr(content) && fr.warnings.is_empty() {
            fr.warnings.push(format!(
                "CRLF/CR line endings detected in {}: consider converting to LF",
                path.display()
            ));
        }
    }

    fn run_with_fix(
        &self,
        path: &Path,
        content: &str,
        fr: &mut FileResult,
        do_fix: bool,
    ) -> Option<String> {
        // First, emit the same kind of warnings as `run` so user sees the issue when not fixing.
        // Note: callers usually call run_with_fix directly (the default delegates to run).
        let mut saw = false;
        for (i, line) in content.split('\n').enumerate() {
            if line.ends_with('\r') || line.contains('\r') {
                fr.warnings.push(format!(
                    "CR/LF line ending detected ({}: line {}): consider converting to LF",
                    path.display(),
                    i + 1
                ));
                saw = true;
            }
        }
        if !saw && Self::has_cr(content) {
            fr.warnings.push(format!(
                "CRLF/CR line endings detected in {}: consider converting to LF",
                path.display()
            ));
            saw = true;
        }

        if !saw {
            return None;
        }

        if do_fix {
            // Normalize CRLF and any stray CR to LF.
            let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
            // If normalization produced different content, return it for the caller to persist.
            if normalized != content {
                // Optionally add an info-style warning to indicate an automatic fix was performed.
                fr.warnings.push(format!(
                    "line endings normalized to LF in {} (in-memory change applied)",
                    path.display()
                ));
                return Some(normalized);
            }
        }

        None
    }
}

/// Factory used by the rules loader.
pub fn create() -> Box<dyn crate::rules::Rule> {
    Box::new(LineEndingsRule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmds::check::file::FileResult;
    use crate::rules::Rule;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn detects_crlf_lines() {
        let content = "first line\r\nsecond line\r\n";
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("crlf.txt");
        fs::write(&p, content).unwrap();

        let mut fr = FileResult {
            path: p.to_string_lossy().to_string(),
            kind: "text".to_string(),
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            fixed: false,
        };

        let r = LineEndingsRule;
        r.run(&p, content, &mut fr);
        assert!(
            fr.warnings
                .iter()
                .any(|w| w.contains("CR/LF") || w.contains("CRLF")),
            "expected a warning about CRLF"
        );
    }

    #[test]
    fn auto_fix_crlf_to_lf() {
        let content = "alpha\r\nbeta\r\n";
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("fix_me.txt");
        fs::write(&p, content).unwrap();

        let mut fr = FileResult {
            path: p.to_string_lossy().to_string(),
            kind: "text".to_string(),
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            fixed: false,
        };

        let r = LineEndingsRule;
        let fixed_opt = r.run_with_fix(&p, content, &mut fr, true);
        assert!(
            fixed_opt.is_some(),
            "expected the rule to produce a fixed content"
        );

        let fixed = fixed_opt.unwrap();
        assert!(
            !fixed.contains('\r'),
            "expected all CR characters to be removed"
        );
        assert_eq!(fixed, "alpha\nbeta\n");
    }

    #[test]
    fn no_warning_for_clean_lf() {
        let content = "one\ntwo\n";
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("clean.txt");
        fs::write(&p, content).unwrap();

        let mut fr = FileResult {
            path: p.to_string_lossy().to_string(),
            kind: "text".to_string(),
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            fixed: false,
        };

        let r = LineEndingsRule;
        r.run(&p, content, &mut fr);
        assert!(
            fr.warnings.is_empty(),
            "expected no warnings for clean LF content"
        );

        // run_with_fix should also return None when there's nothing to fix
        let fixed_opt = r.run_with_fix(&p, content, &mut fr, true);
        assert!(
            fixed_opt.is_none(),
            "expected no fix for already-LF content"
        );
    }
}
