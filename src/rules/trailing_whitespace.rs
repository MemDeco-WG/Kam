/*!
A simple rule that detects trailing whitespace at the end of lines.

Conventions:
- Export a `pub fn create() -> Box<dyn crate::rules::Rule>` factory function.
- Implement the `crate::rules::Rule` trait:

  fn id(&self) -> &'static str;
  fn description(&self) -> &'static str;
  fn run(&self, path: &std::path::Path, content: &str, fr: &mut crate::cmds::check::file::FileResult);

This rule flags any line that ends with one or more spaces or tabs.
*/

use regex::Regex;
use std::path::Path;

use crate::cmds::check::file::FileResult;

/// Zero-sized rule type (no state required).
pub struct TrailingWhitespaceRule;

impl TrailingWhitespaceRule {
    fn pattern() -> &'static Regex {
        // Matches one or more spaces or tabs at the end of a line.
        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(r"[ \t]+$").unwrap();
        }
        &RE
    }
}

impl crate::rules::Rule for TrailingWhitespaceRule {
    fn id(&self) -> &'static str {
        "trailing_whitespace"
    }

    fn description(&self) -> &'static str {
        "Detect trailing spaces/tabs at the end of lines"
    }

    fn run(&self, _path: &Path, content: &str, fr: &mut FileResult) {
        for (i, line) in content.lines().enumerate() {
            if Self::pattern().is_match(line) {
                // Show the line content trimmed of trailing whitespace so the
                // snippet is readable in diagnostics.
                let snippet = line.trim_end_matches([' ', '\t']).to_string();
                let msg = format!("trailing whitespace detected (line {}): {}", i + 1, snippet);
                fr.warnings.push(msg);
            }
        }
    }

    /// Optional auto-fix: when `do_fix` is true, return a modified version
    /// of the file content with trailing spaces/tabs removed from each line.
    /// If no changes are necessary, return `None`.
    fn run_with_fix(
        &self,
        _path: &Path,
        content: &str,
        _fr: &mut FileResult,
        do_fix: bool,
    ) -> Option<String> {
        if !do_fix {
            return None;
        }

        // Normalize each line by trimming trailing spaces and tabs.
        // Preserve whether the original content ended with a newline.
        let ends_with_newline = content.ends_with('\n');
        let fixed_lines: Vec<String> = content
            .split('\n')
            .map(|line| line.trim_end_matches([' ', '\t']).to_string())
            .collect();
        let mut fixed = fixed_lines.join("\n");
        if ends_with_newline && !fixed.ends_with('\n') {
            fixed.push('\n');
        }

        if fixed != content { Some(fixed) } else { None }
    }
}

pub fn create() -> Box<dyn crate::rules::Rule> {
    Box::new(TrailingWhitespaceRule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmds::check::file::FileResult;
    use crate::rules::Rule;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn basic_trailing_whitespace_detection() {
        let content = "#!/bin/sh\nfoo \nbar\t\nbaz\n   \n";
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("t.sh");
        fs::write(&p, content).unwrap();

        let mut fr = FileResult {
            path: p.to_string_lossy().to_string(),
            kind: "sh".to_string(),
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            fixed: false,
        };

        let r = TrailingWhitespaceRule;
        r.run(&p, content, &mut fr);

        // Expect at least three warnings for the three trailing-whitespace lines.
        assert!(
            fr.warnings
                .iter()
                .any(|w| w.contains("trailing whitespace detected")),
            "expected at least one trailing whitespace warning"
        );
        assert!(
            fr.warnings.len() >= 3,
            "expected warnings for each line with trailing whitespace"
        );
    }

    #[test]
    fn no_trailing_whitespace_no_warning() {
        let content = "#!/bin/sh\nalpha\nbeta\ngamma\n";
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("clean.sh");
        fs::write(&p, content).unwrap();

        let mut fr = FileResult {
            path: p.to_string_lossy().to_string(),
            kind: "sh".to_string(),
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            fixed: false,
        };

        let r = TrailingWhitespaceRule;
        r.run(&p, content, &mut fr);

        assert!(
            fr.warnings.is_empty(),
            "expected no warnings for clean file"
        );
    }

    #[test]
    fn detects_space_and_tab_trailing() {
        let content = "one \ntwo\t\nthree \n";
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("mixed.sh");
        fs::write(&p, content).unwrap();

        let mut fr = FileResult {
            path: p.to_string_lossy().to_string(),
            kind: "sh".to_string(),
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            fixed: false,
        };

        let r = TrailingWhitespaceRule;
        r.run(&p, content, &mut fr);

        assert_eq!(
            fr.warnings.len(),
            3,
            "expected three warnings for three lines with trailing whitespace"
        );
    }

    #[test]
    fn auto_fix_trailing_whitespace() {
        // Ensure trait method is in scope so run_with_fix can be called on the concrete type.
        use crate::rules::Rule;

        let content = "alpha \nbeta\t\nok\n";
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("fix.sh");
        fs::write(&p, content).unwrap();

        let mut fr = FileResult {
            path: p.to_string_lossy().to_string(),
            kind: "sh".to_string(),
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            fixed: false,
        };

        let r = TrailingWhitespaceRule;
        let fixed_opt = r.run_with_fix(&p, content, &mut fr, true);
        assert!(
            fixed_opt.is_some(),
            "expected rule to produce a fixed content"
        );

        let fixed_content = fixed_opt.unwrap();
        assert!(
            !fixed_content
                .lines()
                .any(|l| l.ends_with(' ') || l.ends_with('\t')),
            "expected all trailing spaces/tabs to be removed"
        );
    }
}
