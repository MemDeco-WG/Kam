//! Detect insecure (HTTP) URLs and recommend using HTTPS instead.
//
//! Small builtin rule for `kam check` that scans file content for `http://`
//! URLs and emits a warning for each occurrence. This helps catch insecure
//! references that should generally use `https://`.
//
//! Conventions for rule modules:
//! - Export a `pub fn create() -> Box<dyn crate::rules::Rule>` factory function.
//! - Implement `crate::rules::Rule` (id, description, run).

use regex::Regex;
use std::path::Path;

use crate::cmds::check::file::FileResult;

/// Rule type (stateless, zero-sized).
pub struct HttpUrlsRule;

impl HttpUrlsRule {
    fn pattern() -> &'static Regex {
        // Match http:// followed by non-whitespace characters (a rough URL match).
        // Use a word boundary before the scheme to avoid accidental matches in
        // concatenated words while still matching typical usages.
        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(r"\bhttp://\S+").unwrap();
        }
        &RE
    }
}

impl crate::rules::Rule for HttpUrlsRule {
    fn id(&self) -> &'static str {
        "http_urls"
    }

    fn description(&self) -> &'static str {
        "Detect insecure HTTP URLs (recommend HTTPS)"
    }

    fn run(&self, _path: &Path, content: &str, fr: &mut FileResult) {
        for (i, line) in content.lines().enumerate() {
            for mat in Self::pattern().find_iter(line) {
                let url = mat.as_str();
                fr.warnings.push(format!(
                    "insecure http URL detected (line {}): {}",
                    i + 1,
                    url
                ));
            }
        }
    }
}

/// Factory for the loader.
pub fn create() -> Box<dyn crate::rules::Rule> {
    Box::new(HttpUrlsRule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmds::check::file::FileResult;
    use crate::rules::Rule;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn detects_basic_http_url() {
        let content =
            "Please see http://example.com for details.\nAnd secure: https://secure.example/\n";
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("doc.md");
        fs::write(&p, content).unwrap();

        let mut fr = FileResult {
            path: p.to_string_lossy().to_string(),
            kind: "markdown".to_string(),
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            fixed: false,
        };

        let r = HttpUrlsRule;
        r.run(&p, content, &mut fr);

        assert!(
            fr.warnings
                .iter()
                .any(|w| w.contains("insecure http URL detected")),
            "expected insecure http URL warning"
        );
        // Ensure https is not flagged
        assert_eq!(
            fr.warnings.len(),
            1,
            "expected exactly one warning for the single http:// URL"
        );
    }

    #[test]
    fn detects_multiple_http_urls_on_lines() {
        let content = "first http://a\nsecond http://b http://c\n";
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("many.txt");
        fs::write(&p, content).unwrap();

        let mut fr = FileResult {
            path: p.to_string_lossy().to_string(),
            kind: "text".to_string(),
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            fixed: false,
        };

        let r = HttpUrlsRule;
        r.run(&p, content, &mut fr);

        assert_eq!(fr.warnings.len(), 3, "expected three http:// matches");
    }

    #[test]
    fn no_false_positive_for_https_or_text() {
        let content = "This has https://secure.example and plain text 'httpish' and http://ok\n";
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("mix.txt");
        fs::write(&p, content).unwrap();

        let mut fr = FileResult {
            path: p.to_string_lossy().to_string(),
            kind: "text".to_string(),
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            fixed: false,
        };

        let r = HttpUrlsRule;
        r.run(&p, content, &mut fr);

        // Only one real http:// should be flagged
        assert_eq!(fr.warnings.len(), 1);
        assert!(fr.warnings[0].contains("http://ok"));
    }
}
