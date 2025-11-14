use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::errors::KamError;
use crate::types::kam_toml::enums::ModuleType;
use crate::types::modules::KamToml;

pub mod args;
pub mod impl_mod;
pub mod kam;
pub mod post_init;
pub mod repo;
pub mod status;
pub mod tmpl_mod;
pub use args::InitArgs;

/// Get git repository information
fn get_git_info() -> Result<(String, String, String, String), KamError> {
    let repo = git2::Repository::discover(".")?;

    // Get author name, handle non-UTF-8
    let author = std::process::Command::new("git")
        .args(&["config", "--get", "user.name"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or("Author".to_string());

    // Get author email using git command
    let email = std::process::Command::new("git")
        .args(&["config", "--get", "user.email"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or("author@example.com".to_string());

    // Get remote URL
    let remote_url = if let Ok(remote) = repo.find_remote("origin") {
        remote
            .url()
            .map(|s| s.to_string())
            .unwrap_or("".to_string())
    } else {
        "".to_string()
    };

    // Get default branch
    let default_branch = if let Ok(reference) = repo.find_reference("refs/remotes/origin/HEAD") {
        if let Some(target) = reference.symbolic_target() {
            target
                .strip_prefix("refs/remotes/origin/")
                .unwrap_or("master")
                .to_string()
        } else {
            "master".to_string()
        }
    } else {
        "master".to_string()
    };

    Ok((author, email, remote_url, default_branch))
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

/// Run the init command
pub fn run(args: InitArgs) -> Result<(), KamError> {
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
    let name = args.project_name.as_deref().unwrap_or("My Module");
    let path = project_path.as_path();

    // Ensure cache is initialized early so templates and builtins are available.
    // Try automatic initialization; if it fails, print a helpful hint and continue.
    if let Err(e) = crate::cache::KamCache::new().and_then(|c| c.ensure_dirs()) {
        println!("Note: failed to initialize Kam cache: {}", e);
        println!("You can initialize the cache by running: 'kam cache info' or 'kam sync'.");
        println!(
            "Continuing init without a cache - some templates or modules may not be available."
        );
    }

    // Validate conflicting flags
    let type_flags = [args.kam, args.lib, args.tmpl, args.repo, args.venv]
        .iter()
        .filter(|&&x| x)
        .count();
    if type_flags > 1 {
        return Err(KamError::InvalidModuleType(
            "Cannot specify multiple module types: --kam, --lib, --tmpl, --repo, --venv"
                .to_string(),
        ));
    }

    // Module type determination will be handled in the main logic below

    // Environment variables used in this project (collected):
    // - GITHUB_TOKEN       : used by `publish` as a default auth token when --token is not provided
    // - KAM_PUBLISH_TOKEN  : alternative token for publish (fallback)
    // - KAM_LOCAL_REPO     : used by `sync` as a candidate local repository path
    // - HOME / USERPROFILE : used by cache code to locate the user's home directory
    //

    // Determine module type and template first
    let (module_type, impl_template) = if args.kam {
        (ModuleType::Kam, "kam_template".to_string())
    } else if args.lib {
        (ModuleType::Library, "lib_template".to_string())
    } else if args.tmpl {
        (ModuleType::Template, "tmpl_template".to_string())
    } else if args.repo {
        (ModuleType::Repo, "repo_template".to_string())
    } else if args.venv {
        (ModuleType::Template, "venv_template".to_string())
    } else if let Some(impl_name) = &args.r#impl {
        (ModuleType::Kam, impl_name.clone())
    } else {
        (ModuleType::Kam, "kam_template".to_string())
    };

    // Parse template variables
    let mut template_vars = crate::template::TemplateManager::parse_template_vars(&args.var)?;

    let version = args.version.as_deref().unwrap_or("1.0.0");

    // Add project_name and description to template_vars
    let project_name = args.project_name.as_deref().unwrap_or("My Module");
    let description = args.description.as_deref().unwrap_or(&match module_type {
        ModuleType::Kam => "A kam module",
        ModuleType::Library => "A library module",
        ModuleType::Template => "A template module",
        ModuleType::Repo => "A repository module",
    });
    template_vars.insert("project_name".to_string(), project_name.to_string());
    template_vars.insert("description".to_string(), description.to_string());

    // Get git info for smart defaults
    let (git_author, git_email, git_remote, git_default_branch) = get_git_info().unwrap_or((
        "Author".to_string(),
        "author@example.com".to_string(),
        "".to_string(),
        "master".to_string(),
    ));
    let default_author = format!("{} ({})", git_author, git_email);
    let author = args.author.as_deref().unwrap_or(&default_author);

    // Determine ID from the project path's basename
    let id = if args.name == "." {
        std::env::current_dir()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    } else {
        args.name.clone()
    };

    let update_json = if args.update_json.is_some() {
        Some(args.update_json.as_deref().unwrap().to_string())
    } else {
        Some(generate_update_json_url(
            &git_remote,
            &id,
            &git_default_branch,
        ))
    };

// Create name and description maps with multiple languages
let mut name_map = BTreeMap::new();
name_map.insert("en".to_string(), id.clone()); // Use ID for all languages
name_map.insert("zh-CN".to_string(), id.clone());
name_map.insert("zh-TW".to_string(), id.clone());
name_map.insert("ja".to_string(), id.clone());
name_map.insert("ko".to_string(), id.clone());

let mut description_map = BTreeMap::new();
description_map.insert("en".to_string(), description.to_string());
description_map.insert(
    "zh-CN".to_string(),
    format!(
        "一个{}模块",
        match module_type {
            ModuleType::Kam => "kam",
            ModuleType::Library => "库",
            ModuleType::Template => "模板",
            ModuleType::Repo => "仓库",
        }
    ),
);
description_map.insert(
    "zh-TW".to_string(),
    format!(
        "一個{}模組",
        match module_type {
            ModuleType::Kam => "kam",
            ModuleType::Library => "庫",
            ModuleType::Template => "模板",
            ModuleType::Repo => "倉庫",
        }
    ),
);
description_map.insert(
    "ja".to_string(),
    format!(
        "{}モジュール",
        match module_type {
            ModuleType::Kam => "kam",
            ModuleType::Library => "ライブラリ",
            ModuleType::Template => "テンプレート",
            ModuleType::Repo => "リポジトリ",
        }
    ),
);
description_map.insert(
    "ko".to_string(),
    format!(
        "{} 모듈",
        match module_type {
            ModuleType::Kam => "kam",
            ModuleType::Library => "라이브러리",
            ModuleType::Template => "템플릿",
            ModuleType::Repo => "저장소",
        }
    ),
);

// Create KamToml
let mut kt = KamToml::new_with_current_timestamp(
    id.clone(),
    name_map.clone(),
    version.to_string(),
    author.to_string(),
    description_map.clone(),
    update_json.clone(),
    None,
);

// For repo modules, initialize mmrl.repo with repository template variable
if module_type == ModuleType::Repo {
    let mmrl = kt.mmrl.get_or_insert_with(Default::default);
    let repo = mmrl.repo.get_or_insert_with(Default::default);
    repo.repository = Some("{{repository}}".to_string());
}

    // Initialize using template
    tmpl_mod::init_template(
        &path,
        &id,
        name_map,
        &version,
        &author,
        description_map,
        &args.var,
        Some(impl_template),
        args.force,
        module_type,
        update_json,
    )?;

    post_init::post_process(
        &path,
        &args,
        &mut template_vars,
        &id,
        &name,
        &version,
        &author,
        &description,
    )?;

    Ok(())
}
