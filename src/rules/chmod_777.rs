//! A sample rule implementation for rules.d demonstrating how to implement a
//! simple, built-in Rust rule for kam.
//!
//! Conventions for rule modules in `rules.d/`:
//! - Each rule module SHOULD export a `pub fn create() -> Box<dyn crate::rules::Rule>`
//!   factory function which returns a boxed instance of the rule.
//! - The rule type should implement the `crate::rules::Rule` trait, which has
//!   the signature:
//!     fn id(&self) -> &'static str;
//!     fn description(&self) -> &'static str;
//!     fn run(&self, path: &std::path::Path, content: &str, fr: &mut crate::cmds::check::file::FileResult);
//!
//! The `chmod_777` rule below detects uses of `chmod 777` (and `chmod -R 777`) and
//! emits a warning with a short snippet of the offending line.

use regex::Regex;
use std::path::Path;

use crate::cmds::check::file::FileResult;

/// The rule type. Keep it zero-sized if no state is required.
pub struct Chmod777Rule;

impl Chmod777Rule {
    fn pattern() -> &'static Regex {
        // Matches:
        //  chmod 777 ...
        //  chmod -R 777 ...
        // Allows arbitrary spacing and optional -R.
        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(r"\bchmod\s+(?:-R\s+)?7{3}\b").unwrap();
        }
        &RE
    }
}

impl crate::rules::Rule for Chmod777Rule {
    fn id(&self) -> &'static str {
        "chmod_777"
    }

    fn description(&self) -> &'static str {
        "Detect usages of `chmod 777` and recommend more restrictive permissions"
    }

    fn run(&self, _path: &Path, content: &str, fr: &mut FileResult) {
        // Scan line by line and report each match once (with the line snippet).
        for (i, line) in content.lines().enumerate() {
            if Self::pattern().is_match(line) {
                let msg = format!("chmod 777 detected (line {}): {}", i + 1, line.trim());
                fr.warnings.push(msg);
            }
        }
    }
}

/// Factory used by the rules loader to instantiate this rule.
/// The `rules` module will call this when building the active ruleset.
pub fn create() -> Box<dyn crate::rules::Rule> {
    Box::new(Chmod777Rule)
}
