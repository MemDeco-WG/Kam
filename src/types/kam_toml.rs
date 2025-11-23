use chrono;
use git2::Repository;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use toml;

pub mod sections;
use sections::*;

use crate::types::modules::DEFAULT_DEPENDENCY_SOURCE;

pub mod enums;

/// KamToml: A superset of module.prop, update.json, and other metadata,
/// inspired by pyproject.toml format with hierarchical sections.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct KamToml {
    pub prop: PropSection,
    pub mmrl: Option<MmrlSection>,
    pub kam: KamSection,

    pub tmpl: Option<TmplSection>,
    pub tool: Option<ToolSection>,
    // lib字段在kam.lib!
    #[serde(skip)]
    pub raw: String,
}
impl Default for KamToml {
    fn default() -> Self {
        // Use defaults from section Default impls where appropriate.
        let mut default = KamToml::from_prop(PropSection::default());
        default.mmrl = Some(MmrlSection::default());
        default.kam = KamSection::default();
        default.raw = "".to_string();
        default
    }
}

impl KamToml {
    pub fn generate_description_map(module_type: &enums::ModuleType) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        map.insert(
            "en".to_string(),
            match module_type {
                enums::ModuleType::Kam => "A kam module",
                enums::ModuleType::Template => "A template module",
            }
            .to_string(),
        );
        map.insert(
            "zh-CN".to_string(),
            match module_type {
                enums::ModuleType::Kam => "kam",
                enums::ModuleType::Template => "模板",
            }
            .to_string(),
        );
        map.insert(
            "zh-TW".to_string(),
            match module_type {
                enums::ModuleType::Kam => "kam",
                enums::ModuleType::Template => "模板",
            }
            .to_string(),
        );
        map.insert(
            "ja".to_string(),
            match module_type {
                enums::ModuleType::Kam => "kam",
                enums::ModuleType::Template => "テンプレート",
            }
            .to_string(),
        );
        map.insert(
            "ko".to_string(),
            match module_type {
                enums::ModuleType::Kam => "kam",
                enums::ModuleType::Template => "템플릿",
            }
            .to_string(),
        );
        map
    }
    /// Get git remote origin URL from a directory
    fn get_git_remote_url(project_dir: &Path) -> Result<String, crate::errors::KamError> {
        let repo = Repository::open(project_dir).map_err(|e| {
            crate::errors::KamError::CommandFailed(format!("Failed to open git repo: {}", e))
        })?;
        let remote = repo.find_remote("origin").map_err(|e| {
            crate::errors::KamError::CommandFailed(format!("Failed to find remote: {}", e))
        })?;
        let url = remote.url().unwrap_or("https://github.com/MemDeco-WG/Kam");
        let url = if url.is_empty() {
            "https://github.com/MemDeco-WG/Kam".to_string()
        } else {
            url.to_string()
        };
        // Convert SSH URL to HTTPS if needed
        let url = if url.starts_with("git@") {
            // git@github.com:user/repo.git -> https://github.com/user/repo
            if let Some(colon_pos) = url.find(':') {
                if let Some(dot_pos) = url.rfind('.') {
                    let host_and_user = &url[4..colon_pos];
                    let repo_name = &url[colon_pos + 1..dot_pos];
                    format!("https://{}/{}", host_and_user, repo_name)
                } else {
                    url
                }
            } else {
                url
            }
        } else {
            url
        };
        // Remove .git suffix if present
        Ok(url.strip_suffix(".git").unwrap_or(&url).to_string())
    }

    /// Get git user info from a directory
    fn get_git_user_info(project_dir: &Path) -> Result<(String, String), crate::errors::KamError> {
        let repo = Repository::open(project_dir).map_err(|e| {
            crate::errors::KamError::CommandFailed(format!("Failed to open git repo: {}", e))
        })?;
        let config = repo.config().map_err(|e| {
            crate::errors::KamError::CommandFailed(format!("Failed to get config: {}", e))
        })?;
        let name = config
            .get_string("user.name")
            .unwrap_or("Unknown".to_string());
        let email = config
            .get_string("user.email")
            .unwrap_or("unknown@example.com".to_string());
        Ok((name, email))
    }

    /// Generate updateJson URL from repository remote URL, project ID, and default branch
    fn generate_update_json_url(remote_url: &str, id: &str, default_branch: &str) -> String {
        if remote_url.contains("github.com") {
            // Parse GitHub URL: https://github.com/owner/repo.git -> https://raw.githubusercontent.com/owner/repo/{branch}/update.json
            let parts: Vec<&str> = remote_url.trim_end_matches(".git").split('/').collect();
            if parts.len() >= 5 {
                let owner = parts[3];
                return format!(
                    "https://raw.githubusercontent.com/{}/{}/{}/update.json",
                    owner, id, default_branch
                );
            }
        } else if remote_url.contains("gitlab.com") {
            // GitLab: https://gitlab.com/owner/repo.git -> https://gitlab.com/owner/repo/-/raw/{branch}/update.json
            let parts: Vec<&str> = remote_url.trim_end_matches(".git").split('/').collect();
            if parts.len() >= 5 {
                let owner = parts[3];
                return format!(
                    "https://gitlab.com/{}/{}/-/raw/{}/update.json",
                    owner, id, default_branch
                );
            }
        }
        // Default or unknown
        format!(
            "https://raw.githubusercontent.com/user/{}/{}/update.json",
            id, default_branch
        )
    }

    /// Get git default branch from a directory
    fn get_git_default_branch(project_dir: &Path) -> Result<String, crate::errors::KamError> {
        let repo = Repository::open(project_dir).map_err(|e| {
            crate::errors::KamError::CommandFailed(format!("Failed to open git repo: {}", e))
        })?;

        // Try remote HEAD first
        if let Some(branch) = Self::try_get_branch_from_head(&repo) {
            return Ok(branch);
        }

        // Fallback to common branches
        Self::find_existing_branch(&repo, &["main", "master"])
    }

    /// Try to get branch name from refs/remotes/origin/HEAD
    fn try_get_branch_from_head(repo: &Repository) -> Option<String> {
        let reference = repo.find_reference("refs/remotes/origin/HEAD").ok()?;
        let target = reference.target()?;
        let branch_ref_name = format!("refs/remotes/origin/{}", target);
        let branch_ref = repo.find_reference(&branch_ref_name).ok()?;
        let name = branch_ref.name()?;
        name.strip_prefix("refs/remotes/origin/")
            .map(|s| s.to_string())
    }

    /// Find the first existing branch from a list
    fn find_existing_branch(
        repo: &Repository,
        branches: &[&str],
    ) -> Result<String, crate::errors::KamError> {
        for &branch in branches {
            let ref_name = format!("refs/remotes/origin/{}", branch);
            if repo.find_reference(&ref_name).is_ok() {
                return Ok(branch.to_string());
            }
        }
        Ok("main".to_string()) // Default fallback
    }

    /// Construct a KamToml starting from a PropSection (useful for default
    /// composition). This helper keeps the same signature as other
    /// constructors in this module.
    pub fn from_prop(prop: PropSection) -> Self {
        KamToml {
            prop,
            mmrl: Some(MmrlSection::default()),
            kam: KamSection::default(),
            tmpl: Some(TmplSection::default()),
            tool: Some(ToolSection::default()),
            raw: String::new(),
        }
    }

    /// Create a new KamToml with current timestamp for versionCode
    pub fn new_with_current_timestamp(
        id: String,
        name: BTreeMap<String, String>,
        version: String,
        author: String,
        description: BTreeMap<String, String>,
        update_json: Option<String>,
        module_type: Option<ModuleType>,
    ) -> Self {
        let mut kt = KamToml::from_prop(PropSection {
            id,
            name,
            version,
            versionCode: chrono::Utc::now().timestamp_millis(),
            author,
            description,
            updateJson: update_json,
        });
        if let Some(mt) = module_type {
            kt.kam.module_type = mt;
        }
        kt
    }

    /// Load KamToml from a directory (looks for kam.toml)
    pub fn load_from_dir<P: AsRef<std::path::Path>>(dir: P) -> crate::errors::Result<Self> {
        let path = dir.as_ref().join("kam.toml");
        Self::load_from_file(path)
    }

    /// Load KamToml from a file
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> crate::errors::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut kt: KamToml = toml::from_str(&content)?;
        kt.raw = content;
        Ok(kt)
    }

    /// Auto-fill repository metadata from git remote
    pub fn auto_fill_from_git(
        &mut self,
        project_dir: &std::path::Path,
    ) -> Result<(), crate::errors::KamError> {
        if let Some(mmrl) = &mut self.mmrl {
            if let Some(repo) = &mut mmrl.repo {
                // Get git remote URL
                let remote_url = match Self::get_git_remote_url(project_dir) {
                    Ok(url) => url,
                    Err(_) => {
                        // Fallback for known repo
                        "https://github.com/MemDeco-WG/Kam".to_string()
                    }
                };
                repo.repository = Some(remote_url.clone());
                repo.homepage = Some(remote_url.clone());

                // Set issues URL if it's a GitHub/GitLab repo
                if remote_url.contains("github.com") || remote_url.contains("gitlab.com") {
                    repo.issues = Some(format!("{}/issues", remote_url));
                    repo.readme = Some(format!("{}#readme", remote_url));

                    // Get default branch for changelog URL
                    let default_branch =
                        Self::get_git_default_branch(project_dir).unwrap_or("main".to_string());
                    repo.changelog = Some(format!(
                        "{}/blob/{}/CHANGELOG.md",
                        remote_url, default_branch
                    ));

                    // Set support to issues
                    repo.support = Some(format!("{}/issues", remote_url));
                }

                // Set documentation URL (same as homepage for now)
                repo.documentation = Some(remote_url.clone());

                // Generate updateJson URL
                let default_branch =
                    Self::get_git_default_branch(project_dir).unwrap_or("main".to_string());
                let update_json_url =
                    Self::generate_update_json_url(&remote_url, &self.prop.id, &default_branch);
                if self.prop.updateJson.is_none() {
                    self.prop.updateJson = Some(update_json_url);
                }

                // Get git user info for maintainers and author
                if let Ok((name, email)) = Self::get_git_user_info(project_dir) {
                    if !name.is_empty() && name != "Unknown" {
                        let maintainer = if email != "unknown@example.com" {
                            format!("{} <{}>", name.clone(), email)
                        } else {
                            name.clone()
                        };
                        repo.maintainers = Some(vec![maintainer]);

                        // Set author if not set
                        if self.prop.author.is_empty() {
                            self.prop.author = if email != "unknown@example.com" {
                                format!("{} ({})", name, email)
                            } else {
                                name
                            };
                        }
                    }
                }

                // Set some reasonable defaults for Kam modules
                if repo.categories.as_ref().unwrap_or(&vec![]).is_empty() {
                    repo.categories = Some(vec!["utility".to_string()]);
                }

                if repo.keywords.as_ref().unwrap_or(&vec![]).is_empty() {
                    repo.keywords = Some(vec!["kam".to_string(), "module".to_string()]);
                }

                // Set supported architectures for Android modules
                if repo.arch.as_ref().unwrap_or(&vec![]).is_empty() {
                    repo.arch = Some(vec!["arm64-v8a".to_string()]);
                }

                // Set reasonable API levels
                if repo.min_api == Some(0) {
                    repo.min_api = Some(21); // Android 5.0
                }
                if repo.max_api == Some(0) {
                    repo.max_api = Some(35); // Latest Android
                }

                // Set some common features
                if repo.features.as_ref().unwrap_or(&vec![]).is_empty() {
                    repo.features = Some(vec!["systemless".to_string(), "bootless".to_string()]);
                }
            }
        }

        // Set workspace if not present
        if self.kam.workspace.is_none() {
            self.kam.workspace = Some(WorkspaceSection::default());
        }

        Ok(())
    }

    /// Write KamToml to a directory as kam.toml
    pub fn write_to_dir<P: AsRef<std::path::Path>>(&self, dir: P) -> crate::errors::Result<()> {
        let path = dir.as_ref().join("kam.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Apply template variables to the KamToml structure
    pub fn apply_vars(&mut self, kam_vars: Vec<(String, String)>) -> crate::errors::Result<()> {
        let mut value: toml::Value = toml::from_str(&self.raw)?;
        for (key, val) in kam_vars {
            let key = key.strip_prefix('#').unwrap_or(&key);
            Self::set_value_by_path(&mut value, key, &val);
        }
        self.raw = toml::to_string_pretty(&value)?;
        *self = toml::from_str(&self.raw)?;
        Ok(())
    }

    fn set_value_by_path(value: &mut toml::Value, path: &str, new_value: &str) {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value.as_table_mut().unwrap();
        for &part in &parts[..parts.len() - 1] {
            if !current.contains_key(part) {
                current.insert(part.to_string(), toml::Value::Table(Default::default()));
            }
            current = current[part].as_table_mut().unwrap();
        }
        let last = &parts[parts.len() - 1];
        if *last == "versionCode" {
            if let Ok(num) = new_value.parse::<i64>() {
                current.insert(last.to_string(), toml::Value::Integer(num));
            }
        } else {
            current.insert(last.to_string(), toml::Value::String(new_value.to_string()));
        }
    }

    /// Get effective source URL for dependencies
    pub fn get_effective_source(dep: &Dependency) -> String {
        dep.source
            .clone()
            .unwrap_or_else(|| DEFAULT_DEPENDENCY_SOURCE.to_string())
    }

    /// Resolve dependencies into flattened groups
    pub fn resolve_dependencies(&self) -> crate::errors::Result<sections::FlatDependencyGroups> {
        self.kam
            .dependency
            .as_ref()
            .unwrap_or(&DependencySection::default())
            .resolve()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_fill_from_git() {
        let mut kt = KamToml::default();
        let current_dir = std::env::current_dir().unwrap();
        let result = kt.auto_fill_from_git(&current_dir);
        // Should not error
        assert!(result.is_ok());
        // Check if repository is set
        if let Some(mmrl) = &kt.mmrl {
            if let Some(repo) = &mmrl.repo {
                assert!(repo.repository.is_some());
                assert!(repo.homepage.is_some());
                // For GitHub repo, issues should be set
                if repo.repository.as_ref().unwrap().contains("github.com") {
                    assert!(repo.issues.is_some());
                }
            }
        }
    }
}
