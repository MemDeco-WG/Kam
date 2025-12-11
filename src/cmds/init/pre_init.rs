use git2::Repository;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;

/// Pre-initialization data structure
pub struct PreInitData {
    pub path: PathBuf,
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub template_vars: HashMap<String, String>,
    pub impl_template: String,
    pub module_type: ModuleType,
    pub update_json: Option<String>,
    pub kam_toml: KamToml,
}

/// Prepare initialization data before creating the project
pub fn prepare_init(args: &super::InitArgs) -> Result<PreInitData, KamError> {
    let current_dir = std::env::current_dir()?;
    let project_name = &args.name;
    let project_path: PathBuf = if project_name.starts_with('/')
        || project_name.starts_with('\\')
        || project_name.contains(':')
    {
        PathBuf::from(project_name)
    } else {
        current_dir.join(project_name)
    };

    // Validate conflicting flags
    // Only `--tmpl` and `-t/--template` are valid mutually exclusive options.
    let type_flags = [args.tmpl, args.template.is_some()]
        .iter()
        .filter(|&&x| x)
        .count();
    if type_flags > 1 {
        return Err(KamError::InvalidModuleType(
            "Cannot specify multiple module types: --tmpl, -t/--template".to_string(),
        ));
    }

    // Determine module type and template.
    // The -t/--template option supports:
    //  1) A full template id (e.g., "kam_template" or "kam_template.tar.gz"),
    //  2) A short builtin id (e.g., "kam", "meta", "ak3") which will map to "<id>_template",
    //  3) A local path or archive (e.g., /path/to/template.tar.gz or https://.../template.tar.gz).
    let (module_type, impl_template) = if args.tmpl {
        (ModuleType::Template, "tmpl_template".to_string())
    } else if let Some(t) = &args.template {
        // Detect likely path/archive/URL to avoid appending suffix in those cases.
        let is_path_or_archive = t.contains('/')
            || t.contains('\\')
            || t.contains(':')
            || t.ends_with(".tar.gz")
            || t.ends_with(".tgz")
            || t.ends_with(".zip");

        let impl_spec = if t.ends_with("_template") || is_path_or_archive {
            t.clone()
        } else {
            format!("{}_template", t)
        };

        (ModuleType::Kam, impl_spec)
    } else {
        // default to kam module
        (ModuleType::Kam, "kam_template".to_string())
    };

    // Parse template variables
    let mut template_vars = crate::template::TemplateManager::parse_template_vars(&args.var)?;

    let version = args.version.as_deref().unwrap_or("1.0.0");

    // Add project_name and description to template_vars
    let project_name_str = args
        .project_name
        .as_deref()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Example Module Name".to_string());
    let description_str = args
        .description
        .as_deref()
        .unwrap_or_else(|| match module_type {
            ModuleType::Kam => "Describe your module here",
            ModuleType::Template => "Describe your template here",
        });
    template_vars.insert("project_name".to_string(), project_name_str.clone());
    template_vars.insert("description".to_string(), description_str.to_string());

    // Discover Git metadata early so we can use it for defaults (id, author, repo URL, etc)
    let mut git_author: Option<String> = None;
    let mut git_repo_url: Option<String> = None;
    // Try discovering git repository
    if let Ok(repo) = Repository::discover(&current_dir) {
        if let Ok(cfg) = repo.config() {
            if let Ok(name) = cfg.get_string("user.name") {
                git_author = Some(name);
            }
        }
        if let Ok(remote) = repo.find_remote("origin") {
            if let Some(url) = remote.url() {
                git_repo_url = Some(url.to_string());
            }
        }
        // if we didn't find origin, pick the first remote available
        if git_repo_url.is_none() {
            if let Ok(remotes) = repo.remotes() {
                if let Some(name) = remotes.get(0) {
                    if let Ok(remote) = repo.find_remote(name) {
                        if let Some(url) = remote.url() {
                            git_repo_url = Some(url.to_string());
                        }
                    }
                }
            }
        }
    }

    // Determine ID: use --id if provided, otherwise use git repo name (if repo detected and name is not '.'),
    // otherwise use folder name
    let id = if let Some(custom_id) = &args.id {
        custom_id.clone()
    } else if args.name == "." {
        // if git repo identified, prefer repo name
        if let Some(repo_url) = git_repo_url.clone() {
            if let Some((_owner, repo_name)) = parse_git_remote_url(&repo_url) {
                repo_name
            } else {
                std::env::current_dir()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            }
        } else {
            std::env::current_dir()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        }
    } else {
        // Extract basename from path (e.g., "/tmp/test_kam_init" -> "test_kam_init")
        std::path::Path::new(&args.name)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&args.name)
            .to_string()
    };

    // Validate ID format (alphanumeric, dots, dashes, underscores)
    if !id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        return Err(KamError::InvalidConfig(format!(
            "Invalid module ID '{}': ID must contain only alphanumeric characters, dots, dashes, and underscores",
            id
        )));
    }

    if id.is_empty() {
        return Err(KamError::InvalidConfig(
            "Module ID cannot be empty".to_string(),
        ));
    }

    // determine author selection continues below using discovered metadata
    // Also check global config for default author (~/.kam/config.toml)
    let mut global_author: Option<String> = None;
    if let Some(home) = dirs::home_dir() {
        let cfg_path = home.join(".kam").join("config.toml");
        if cfg_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&cfg_path) {
                if let Ok(v) = toml::from_str::<toml::Value>(&content) {
                    if let Some(prop) = v.get("prop") {
                        if let Some(author_val) = prop.get("author") {
                            if let Some(s) = author_val.as_str() {
                                global_author = Some(s.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let author = if let Some(a) = args.author.as_deref() {
        a.to_string()
    } else if let Some(a) = git_author.clone() {
        a
    } else if let Some(a) = global_author {
        a
    } else {
        "Your Name".to_string()
    };

    let update_json_val = args
        .update_json
        .clone()
        .or_else(|| crate::types::kam_toml::sections::PropSection::default().updateJson);

    // Create initial KamToml with defaults
    let mut kam_toml = KamToml::new_with_current_timestamp(
        id.clone(),
        project_name_str.to_string(),
        version.to_string(),
        author.clone(),
        description_str.to_string(),
        update_json_val,
        None,
    );

    // Set name and description (allow git-based defaults to be overridden later)
    kam_toml.prop.name = project_name_str.to_string();
    kam_toml.prop.description = description_str.to_string();

    let update_json = kam_toml.prop.updateJson.clone();

    if let Some(uj) = &update_json {
        template_vars.insert("update_json".to_string(), uj.clone());
    }

    // Set zipUrl and changelog with proper id
    template_vars
        .entry("zipUrl".to_string())
        .or_insert_with(|| {
            format!(
                "https://github.com/user/repo/releases/latest/download/{}.zip",
                id
            )
        });
    template_vars
        .entry("changelog".to_string())
        .or_insert_with(|| {
            "https://raw.githubusercontent.com/user/repo/branch/CHANGELOG.md".to_string()
        });

    // If we discovered a git repository remote, try to populate more intelligent defaults
    if let Some(repo_url) = git_repo_url {
        // Parse basic owner/repo from remote URL
        if let Some((owner, repo_name)) = parse_git_remote_url(&repo_url) {
            // If update_json is unset, provide a default to raw.githubusercontent
            if kam_toml.prop.updateJson.is_none() {
                let default_update = format!(
                    "https://raw.githubusercontent.com/{}/{}/main/update.json",
                    owner, repo_name
                );
                kam_toml.prop.updateJson = Some(default_update.clone());
                template_vars.insert("update_json".to_string(), default_update);
            }

            // Replace zipUrl and changelog with repo-based values if they are still defaults
            template_vars
                .entry("zipUrl".to_string())
                .or_insert_with(|| {
                    format!(
                        "https://github.com/{}/{}/releases/latest/download/{}.zip",
                        owner, repo_name, id
                    )
                });
            template_vars
                .entry("changelog".to_string())
                .or_insert_with(|| {
                    format!(
                        "https://raw.githubusercontent.com/{}/{}/main/CHANGELOG.md",
                        owner, repo_name
                    )
                });

            // If mmrl repo section is present or not, set repository
            kam_toml
                .mmrl
                .get_or_insert(crate::types::kam_toml::sections::MmrlSection::default());
            if let Some(mmrl) = &mut kam_toml.mmrl {
                if mmrl.repo.is_none() {
                    mmrl.repo = Some(crate::types::kam_toml::sections::RepoSection::default());
                }
                if let Some(repo) = &mut mmrl.repo {
                    repo.repository = Some(repo_url.clone());
                }
            }
        }
    }

    Ok(PreInitData {
        path: project_path,
        id: id.clone(),
        name: project_name_str.to_string(),
        version: version.to_string(),
        author: kam_toml.prop.author.clone(),
        description: description_str.to_string(),
        template_vars,
        impl_template,
        module_type,
        update_json,
        kam_toml,
    })
}

fn parse_git_remote_url(remote: &str) -> Option<(String, String)> {
    // Normalize urls like:
    // git@github.com:owner/repo.git
    // https://github.com/owner/repo.git
    // We extract owner and repo
    let s = remote.trim();
    let s = if s.starts_with("git@") {
        // git@github.com:owner/repo.git -> https://github.com/owner/repo.git
        if let Some(idx) = s.find(':') {
            let host = &s[4..idx];
            let path = &s[idx + 1..];
            format!("https://{}/{}", host, path)
        } else {
            s.to_string()
        }
    } else {
        s.to_string()
    };

    // Strip scheme
    let path_start = if let Some(idx) = s.find("//") {
        idx + 2
    } else {
        0
    };
    let path = &s[path_start..];
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 {
        let owner = parts[1].to_string();
        let mut repo = parts[2].to_string();
        // remove .git suffix
        if repo.ends_with(".git") {
            repo.truncate(repo.len() - 4);
        }
        return Some((owner, repo));
    }
    None
}
