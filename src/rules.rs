//! Rules system for `kam check`.
//!
//! This module defines the `Rule` trait and provides a small built-in rule
//! loader which includes rule implementations placed under `rules.d/`.
//!
//! Conventions:
//! - A rule is an implementor of `crate::rules::Rule`.
//! - Built-in rule modules under `rules.d/` SHOULD export a
//!   `pub fn create() -> Box<dyn crate::rules::Rule>` factory function.
//!
//! Note: this is intentionally small and opinionated for now. Future work can
//! add dynamic discovery, per-project enable/disable config, rule severities,
//! rule categories, or caching of instantiated rules.

use std::path::Path;

/// A single lint/check rule.
///
/// Implementors should be small, stateless objects implementing the behavior in
/// `run`. The `id()` and `description()` methods provide a stable identifier and
/// a short human-friendly description for reporting and UI.
pub trait Rule: Send + Sync {
    /// Unique static identifier for this rule (e.g. "chmod_777").
    fn id(&self) -> &'static str;

    /// Short description shown in lists / help.
    fn description(&self) -> &'static str;

    /// Execute the rule against the given file `path` and full file `content`.
    /// Results are appended to the provided `FileResult`.
    fn run(&self, path: &Path, content: &str, fr: &mut crate::cmds::check::file::FileResult);
}

/// Builtin rule implementations are located in the source tree as `crate::rules::<name>`
/// modules (for example: `src/rules/chmod_777.rs`).
/// Add new built-in rules under `src/rules/` as modules and expose them with a
/// `pub fn create() -> Box<dyn crate::rules::Rule>` factory function.
pub mod chmod_777;

/// Load the set of built-in rules.
///
/// Currently this simply returns instances of the statically-included builtin rules.
/// In the future this can be extended to support dynamic discovery, configuration,
/// or feature-gated rule sets.
pub fn load_builtin_rules() -> Vec<Box<dyn Rule>> {
    // Currently only one builtin rule exists; list them here.
    vec![chmod_777::create()]
}

/// Convenience wrapper that returns all rules to be applied.
///
/// This is a single place to aggregate builtin + (future) external rules.
pub fn load_all_rules() -> Vec<Box<dyn Rule>> {
    load_builtin_rules()
}

/// Apply all available rules to the given file content and append results into `fr`.
pub fn apply_all_rules(path: &Path, content: &str, fr: &mut crate::cmds::check::file::FileResult) {
    for rule in load_all_rules().into_iter() {
        rule.run(path, content, fr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmds::check::file::FileResult;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn builtin_rules_load_and_run() {
        // Prepare a temporary shell script containing a bad chmod invocation
        let tmp = TempDir::new().expect("tmpdir");
        let p = tmp.path().join("bad.sh");
        let content = "#!/bin/sh\nchmod 777 /tmp/some\n";
        fs::write(&p, content).expect("write script");

        // Basic file result struct (mimic check_file behavior)
        let mut fr = FileResult {
            path: p.to_string_lossy().to_string(),
            kind: "sh".to_string(),
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            fixed: false,
        };

        // Ensure the builtin rule set contains our sample rule
        let ids: Vec<_> = load_builtin_rules().iter().map(|r| r.id()).collect();
        assert!(
            ids.contains(&"chmod_777"),
            "expected builtin chmod_777 rule to be present"
        );

        // Apply rules and assert the expected warning is produced
        apply_all_rules(&p, content, &mut fr);
        assert!(
            fr.warnings.iter().any(|w| w.contains("chmod 777 detected")),
            "expected chmod_777 rule to emit a warning"
        );
    }
}
