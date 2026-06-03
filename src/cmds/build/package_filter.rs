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
        || rel_path == ".git"
        || rel_path.starts_with(".git/")
        || rel_path == ".github"
        || rel_path.starts_with(".github/")
}

#[cfg(test)]
mod tests {
    use super::should_skip_file;

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
}
