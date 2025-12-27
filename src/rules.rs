//! Rules system for `kam check`.
//!
//! This module provides the `Rule` trait and a small built-in rule loader.
//! Built-in rule implementations are placed under `rules.d/`.
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

    /// Run the rule and optionally perform an in-memory fix.
    ///
    /// When `do_fix` is true a rule MAY return `Some(String)` containing the
    /// modified file content; the caller will then use the returned content for
    /// subsequent rules (and may write it back to disk). The default
    /// implementation delegates to `run` and returns `None` (no modification).
    fn run_with_fix(
        &self,
        path: &Path,
        content: &str,
        fr: &mut crate::cmds::check::file::FileResult,
        _do_fix: bool,
    ) -> Option<String> {
        // Default behaviour: run normal analysis and don't change content.
        self.run(path, content, fr);
        None
    }
}

/// Builtin rule implementations are located in `crate::rules::<name>`.
///
/// For example `src/rules/chmod_777.rs`. Add new built-in rules under `src/rules/`
/// and expose them with a `pub fn create() -> Box<dyn crate::rules::Rule>` factory
/// function.
pub mod chmod_777;
pub mod http_urls;
pub mod line_endings;
pub mod tab_indentation;
pub mod trailing_whitespace;

/// Load the set of built-in rules.
///
/// Currently this simply returns instances of the statically-included builtin rules.
/// In the future this can be extended to support dynamic discovery, configuration,
/// or feature-gated rule sets.
pub fn load_builtin_rules() -> Vec<Box<dyn Rule>> {
    // List builtin rules here. Add new rules as additional calls to `create()`.
    vec![
        chmod_777::create(),
        trailing_whitespace::create(),
        tab_indentation::create(),
        http_urls::create(),
        line_endings::create(),
    ]
}

/// Convenience wrapper that returns all rules to be applied.
///
/// This is a single place to aggregate builtin + (future) external rules.
pub fn load_all_rules() -> Vec<Box<dyn Rule>> {
    load_builtin_rules()
}

/// Apply all available rules to the given file content and append results into `fr`.
///
/// This is a backwards-compatible wrapper around `apply_all_rules_with_fix`
/// that runs rules in analysis-only mode (no in-memory fixes applied).
pub fn apply_all_rules(path: &Path, content: &str, fr: &mut crate::cmds::check::file::FileResult) {
    let _ = apply_all_rules_with_fix(path, content, fr, false, None);
}

/// Apply all available rules and optionally allow rules to perform in-memory fixes.
///
/// When `do_fix` is true, rules may return `Some(new_content)` from `run_with_fix`
/// to replace the file's content. The returned (possibly modified) content is
/// returned to the caller so it can be written back to disk if desired. If a
/// modification was applied and `do_fix` is true, this function will set
/// `fr.fixed = true`.
pub fn apply_all_rules_with_fix(
    path: &Path,
    content: &str,
    fr: &mut crate::cmds::check::file::FileResult,
    do_fix: bool,
    rules_cfg: Option<&std::collections::HashMap<String, crate::types::kam_toml::RuleConfig>>,
) -> String {
    let mut cur = content.to_string();

    for rule in load_all_rules().into_iter() {
        let id = rule.id();

        // Determine per-rule configuration (enabled / allow fix) if provided by project config.
        let mut enabled: bool = true;
        let mut allow_fix: bool = true;
        if let Some(cfg_map) = rules_cfg
            && let Some(cfg) = cfg_map.get(id) {
                if let Some(e) = cfg.enabled {
                    enabled = e;
                }
                if let Some(f) = cfg.fix {
                    allow_fix = f;
                }
            }

        if !enabled {
            // Rule explicitly disabled in project config, skip it entirely.
            continue;
        }

        // Decide whether we should allow the rule to attempt fixes.
        let should_apply_fix = do_fix && allow_fix;

        if should_apply_fix {
            // Let the rule perform its analysis + optional fix and return modified content.
            if let Some(new_content) = rule.run_with_fix(path, &cur, fr, true)
                && new_content != cur {
                    cur = new_content;
                    fr.fixed = true;
                }
        } else {
            // Analysis-only run (no fixes allowed for this rule). Default run_with_fix
            // delegates to run() when do_fix is false, so this also covers rules that
            // only implement the non-fixing `run` method.
            let _ = rule.run_with_fix(path, &cur, fr, false);
        }
    }

    cur
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
