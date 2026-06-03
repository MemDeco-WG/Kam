struct GitInfo {
    author: Option<String>,
    repo_url: Option<String>,
    branch: Option<String>,
    repo_name: Option<String>,
    owner: Option<String>,
}

fn discover_git_info(current_dir: &std::path::Path) -> GitInfo {
    let mut info = GitInfo {
        author: None,
        repo_url: None,
        branch: None,
        repo_name: None,
        owner: None,
    };
    let Ok(repo) = Repository::discover(current_dir) else {
        return info;
    };

    if let Ok(cfg) = repo.config()
        && let Ok(name) = cfg.get_string("user.name")
    {
        info.author = Some(name);
    }
    info.repo_url = discover_remote_url(&repo);
    if let Some(ref url) = info.repo_url
        && let Some((owner, repo_name)) = parse_git_remote_url(url)
    {
        info.owner = Some(owner);
        info.repo_name = Some(repo_name);
    }
    info.branch = repo.head().ok().and_then(|head| {
        head.name().ok().and_then(|name| {
            name.strip_prefix("refs/heads/")
                .map(ToString::to_string)
                .or_else(|| head.shorthand().ok().map(ToString::to_string))
        })
    });
    info
}

fn discover_remote_url(repo: &Repository) -> Option<String> {
    if let Ok(remote) = repo.find_remote("origin")
        && let Ok(url) = remote.url()
    {
        return Some(url.to_string());
    }
    if let Ok(remotes) = repo.remotes()
        && let Some(name) = remotes.get(0).ok().flatten()
        && let Ok(remote) = repo.find_remote(name)
        && let Ok(url) = remote.url()
    {
        return Some(url.to_string());
    }
    None
}

fn infer_project_name(
    project_name_raw: &str,
    current_dir: &std::path::Path,
    repo_name: Option<&str>,
) -> String {
    repo_name.map_or_else(
        || {
            let raw_name = if project_name_raw == "." {
                current_dir
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Example Module Name")
            } else {
                std::path::Path::new(project_name_raw)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(project_name_raw)
            };
            prettify_project_name(raw_name)
        },
        prettify_project_name,
    )
}

fn prettify_project_name(raw: &str) -> String {
    let mut name = raw.replace(['_', '-'], " ");
    if let Some(first_char) = name.chars().next() {
        let mut chars: Vec<char> = name.chars().collect();
        chars[0] = first_char.to_uppercase().next().unwrap_or(first_char);
        name = chars.into_iter().collect();
    }
    name
}

fn fill_repo_documentation_and_license(
    repo: &mut crate::types::kam_toml::sections::RepoSection,
    owner: &str,
    repo_name: &str,
    default_branch: &str,
    current_dir: &std::path::Path,
) {
    if repo.documentation.as_ref().is_none_or(String::is_empty) {
        repo.documentation = Some(format!(
            "https://github.com/{owner}/{repo_name}/blob/{default_branch}/README.md",
        ));
    }

    if repo.license.as_ref().is_none_or(String::is_empty)
        && let Some((license, license_file)) = detect_license_info(current_dir)
    {
        repo.license = Some(license);
        repo.license_file = Some(license_file);
    }
}

fn detect_license_info(current_dir: &std::path::Path) -> Option<(String, String)> {
    let license_paths = [
        current_dir.join("LICENSE"),
        current_dir.join("LICENSE.txt"),
        current_dir.join("LICENSE.md"),
        current_dir.join("license"),
    ];

    for license_path in &license_paths {
        if let Ok(content) = std::fs::read_to_string(license_path) {
            let content_upper = content.to_uppercase();
            let license = if content_upper.contains("MIT") {
                "MIT"
            } else if content_upper.contains("APACHE") && content_upper.contains("2.0") {
                "Apache-2.0"
            } else if content_upper.contains("GPL") && content_upper.contains("3.0") {
                "GPL-3.0"
            } else if content_upper.contains("GPL") && content_upper.contains("2.0") {
                "GPL-2.0"
            } else if content_upper.contains("BSD") && content_upper.contains('3') {
                "BSD-3-Clause"
            } else {
                continue;
            };

            let license_file = license_path.file_name().and_then(|s| s.to_str()).map_or_else(
                || "LICENSE".to_string(),
                ToString::to_string,
            );
            return Some((license.to_string(), license_file));
        }
    }

    None
}

// 解析git remote URL，提取owner和repo
// 支持：
// - git@github.com:owner/repo.git
// - https://github.com/owner/repo.git
fn parse_git_remote_url(remote: &str) -> Option<(String, String)> {
    let s = remote.trim();
    let s = if s.starts_with("git@") {
        // 把 git@ 格式转成 https:// 格式
        s.find(':').map_or_else(
            || s.to_string(),
            |idx| {
                let host = &s[4..idx];
                let path = &s[idx + 1..];
                format!("https://{host}/{path}")
            },
        )
    } else {
        s.to_string()
    };

    // 去掉scheme（https:// 或 http://）
    let path_start = s.find("//").map_or(0, |idx| idx + 2);
    let path = &s[path_start..];
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 {
        let owner = parts[1].to_string();
        let mut repo = parts[2].to_string();
        // 去掉 .git 后缀 (case-insensitive)
        if std::path::Path::new(&repo)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("git"))
        {
            repo.truncate(repo.len() - 4);
        }
        return Some((owner, repo));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::extract_readme_description;

    #[test]
    fn readme_description_skips_centered_html_header() {
        let readme = r#"<div align="center">

# MagicNet

![badge](https://img.shields.io/badge/test-pass-green)

MagicNet is a KernelSU network module for Android devices.
"#;

        assert_eq!(
            extract_readme_description(readme).as_deref(),
            Some("MagicNet is a KernelSU network module for Android devices.")
        );
    }
}
