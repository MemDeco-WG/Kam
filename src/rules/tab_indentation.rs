/*!
A small builtin rule that detects tab characters in files.

Conventions:
- Export a `pub fn create() -> Box<dyn crate::rules::Rule>` factory function.
- Implement the `crate::rules::Rule` trait.

This rule flags any line that contains a tab character (`\t`) and emits a
warning with a short snippet showing where the tab was found.
*/

use std::path::Path;

use crate::cmds::check::file::FileResult;

/// Zero-sized rule type (no state required).
pub struct TabIndentationRule;

impl TabIndentationRule {
    fn has_tab(line: &str) -> bool {
        line.contains('\t')
    }
}

impl crate::rules::Rule for TabIndentationRule {
    fn id(&self) -> &'static str {
        "tab_indentation"
    }

    fn description(&self) -> &'static str {
        "Detect tab characters and recommend using spaces for indentation"
    }

    fn run(&self, _path: &Path, content: &str, fr: &mut FileResult) {
        for (i, line) in content.lines().enumerate() {
            if Self::has_tab(line) {
                // Replace actual tabs with a visible escape so snippets are readable.
                let snippet = line.replace('\t', "\\t");
                fr.warnings.push(format!(
                    "tab character detected (line {}): {}",
                    i + 1,
                    snippet
                ));
            }
        }
    }
}

/// Factory for the rule loader.
pub fn create() -> Box<dyn crate::rules::Rule> {
    Box::new(TabIndentationRule)
}
