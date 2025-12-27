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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmds::check::file::FileResult;
    use crate::rules::Rule;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn basic_tab_detection() {
        let content = "#!/bin/sh\nalpha\tbeta\nno_tabs\n\tleading_tab\n";
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

        let r = TabIndentationRule;
        r.run(&p, content, &mut fr);

        assert!(
            fr.warnings
                .iter()
                .any(|w| w.contains("tab character detected")),
            "expected at least one tab-character warning"
        );
        // Two lines contain tabs in the sample content above.
        assert_eq!(
            fr.warnings.len(),
            2,
            "expected two warnings for two lines containing tabs"
        );
    }

    #[test]
    fn no_tab_no_warning() {
        let content = "one two\nthree four\n";
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

        let r = TabIndentationRule;
        r.run(&p, content, &mut fr);

        assert!(
            fr.warnings.is_empty(),
            "expected no warnings for a clean file"
        );
    }

    #[test]
    fn detects_multiple_tab_positions() {
        let content = "start\tmiddle\tend\n\tleading\ntrailing\t\n";
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("multi_tab.sh");
        fs::write(&p, content).unwrap();

        let mut fr = FileResult {
            path: p.to_string_lossy().to_string(),
            kind: "sh".to_string(),
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            fixed: false,
        };

        let r = TabIndentationRule;
        r.run(&p, content, &mut fr);

        assert_eq!(
            fr.warnings.len(),
            3,
            "expected three warnings for three lines containing tabs"
        );
    }
}
