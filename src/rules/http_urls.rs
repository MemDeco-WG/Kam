//! Detect insecure (HTTP) URLs and recommend using HTTPS instead.
//
//!
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
#[must_use]
pub fn create() -> Box<dyn crate::rules::Rule> {
    Box::new(HttpUrlsRule)
}
