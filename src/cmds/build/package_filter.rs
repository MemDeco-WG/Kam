use std::fs;
use std::path::Path;

pub(super) struct PackageFilter {
    exclude_patterns: Vec<String>,
    include_patterns: Vec<String>,
}

impl PackageFilter {
    pub(super) fn from_root(
        root: &Path,
        config_excludes: Vec<String>,
        config_includes: Vec<String>,
    ) -> Self {
        let mut exclude_patterns = config_excludes;
        let mut include_patterns = Vec::new();
        for rule in read_kamignore(root) {
            if let Some(include) = rule.strip_prefix('!') {
                let include = include.trim();
                if !include.is_empty() {
                    include_patterns.push(include.to_string());
                }
            } else {
                exclude_patterns.push(rule);
            }
        }
        include_patterns.extend(config_includes);

        Self {
            exclude_patterns,
            include_patterns,
        }
    }

    pub(super) fn should_skip_file(&self, rel_path: &str, file_name: Option<&str>) -> bool {
        should_skip_file(
            rel_path,
            file_name,
            &self.exclude_patterns,
            &self.include_patterns,
        )
    }
}

fn read_kamignore(root: &Path) -> Vec<String> {
    let path = root.join(".kamignore");
    let Ok(contents) = fs::read_to_string(path) else {
        return Vec::new();
    };

    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToString::to_string)
        .collect()
}

/// Check if a file should be skipped based on exclude/include patterns.
pub(super) fn should_skip_file(
    rel_path: &str,
    file_name: Option<&str>,
    exclude_patterns: &[String],
    include_patterns: &[String],
) -> bool {
    let is_included = include_patterns
        .iter()
        .any(|pat| crate::utils::pattern_matches(pat, rel_path, file_name));

    if !is_included && is_project_metadata_path(rel_path, file_name) {
        return true;
    }

    let is_excluded = exclude_patterns
        .iter()
        .any(|pat| crate::utils::pattern_matches(pat, rel_path, file_name));

    is_excluded && !is_included
}

fn is_project_metadata_path(rel_path: &str, file_name: Option<&str>) -> bool {
    file_name == Some(".gitignore")
        || file_name == Some(".git")
        || rel_path == ".git"
        || rel_path.starts_with(".git/")
        || rel_path == ".github"
        || rel_path.starts_with(".github/")
}

#[cfg(test)]
mod tests {
    use super::should_skip_file;

    #[test]
    fn nested_git_metadata_file_is_skipped() {
        let exclude_patterns: Vec<String> = vec![];
        let include_patterns: Vec<String> = vec![];
        assert!(should_skip_file(
            "lib/kamfw/.git",
            Some(".git"),
            &exclude_patterns,
            &include_patterns
        ));
    }

    #[test]
    fn gitignore_skipped_by_default() {
        let exclude_patterns: Vec<String> = vec![];
        let include_patterns: Vec<String> = vec![];
        assert!(should_skip_file(
            ".gitignore",
            Some(".gitignore"),
            &exclude_patterns,
            &include_patterns
        ));
    }

    #[test]
    fn include_overrides_metadata_skip() {
        let exclude_patterns: Vec<String> = vec![];
        let include_patterns = vec![".gitignore".to_string()];
        assert!(!should_skip_file(
            ".gitignore",
            Some(".gitignore"),
            &exclude_patterns,
            &include_patterns
        ));
    }

    #[test]
    fn include_overrides_exclude() {
        let exclude_patterns = vec!["foo.txt".to_string()];
        let include_patterns = vec!["foo.txt".to_string()];
        assert!(!should_skip_file(
            "foo.txt",
            Some("foo.txt"),
            &exclude_patterns,
            &include_patterns
        ));
    }

    #[test]
    fn excluded_file_is_skipped() {
        let exclude_patterns = vec!["bar.txt".to_string()];
        let include_patterns: Vec<String> = vec![];
        assert!(should_skip_file(
            "bar.txt",
            Some("bar.txt"),
            &exclude_patterns,
            &include_patterns
        ));
    }

    #[test]
    fn kamignore_include_overrides_kamignore_exclude() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(".kamignore"), "*.log\n!important.log\n").unwrap();
        let filter = super::PackageFilter::from_root(temp.path(), Vec::new(), Vec::new());

        assert!(filter.should_skip_file("debug.log", Some("debug.log")));
        assert!(!filter.should_skip_file("important.log", Some("important.log")));
    }

    #[test]
    fn config_include_overrides_kamignore_exclude() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(".kamignore"), "*.log\n").unwrap();
        let filter = super::PackageFilter::from_root(
            temp.path(),
            Vec::new(),
            vec!["important.log".to_string()],
        );

        assert!(!filter.should_skip_file("important.log", Some("important.log")));
    }
}
