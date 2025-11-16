use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use crate::errors::KamError;
use crate::types::kam_toml::enums::ModuleType;
use crate::types::modules::KamToml;

/// Pre-initialization data structure
pub struct PreInitData {
    pub path: PathBuf,
    pub id: String,
    pub name_map: BTreeMap<String, String>,
    pub version: String,
    pub author: String,
    pub description_map: BTreeMap<String, String>,
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

    // Determine module type and template
    // Use builtin templates indicated by the CLI flags. The legacy `--impl`
    // option has been removed; please use the dedicated flags (e.g. --kam,
    // --lib, --tmpl, --repo, --venv) which map to the builtin template ids.
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

    // Create initial KamToml with defaults
    let mut kam_toml = KamToml::new_with_current_timestamp(
        id.clone(),
        BTreeMap::new(), // Will be set below
        version.to_string(),
        String::new(),   // Will be set from git
        BTreeMap::new(), // Will be set below
        None,            // Will be set from git
        None,
    );

    // Auto-fill from git - required, no fallbacks
    kam_toml.auto_fill_from_git(&current_dir)?;

    // Set name and description maps
    let mut name_map = BTreeMap::new();
    name_map.insert("en".to_string(), id.clone());
    name_map.insert("zh-CN".to_string(), id.clone());
    name_map.insert("zh-TW".to_string(), id.clone());
    name_map.insert("ja".to_string(), id.clone());
    name_map.insert("ko".to_string(), id.clone());
    kam_toml.prop.name = name_map.clone();

    let description_map = KamToml::generate_description_map(&module_type);
    kam_toml.prop.description = description_map.clone();

    // Check required fields from git
    let author = kam_toml.prop.author.clone();
    if author.is_empty() {
        return Err(KamError::CommandFailed(
            "Git author not found. Please configure git user.name and user.email".to_string(),
        ));
    }

    let update_json = kam_toml.prop.updateJson.clone();
    if update_json.is_none() || update_json.as_ref().unwrap().is_empty() {
        return Err(KamError::CommandFailed(
            "Update JSON URL not generated from git".to_string(),
        ));
    }

    // For repo modules, initialize mmrl.repo with repository template variable
    if module_type == ModuleType::Repo {
        let mmrl = kam_toml.mmrl.get_or_insert_with(Default::default);
        let repo = mmrl.repo.get_or_insert_with(Default::default);
        repo.repository = Some("{{repository}}".to_string());
    }

    Ok(PreInitData {
        path: project_path,
        id,
        name_map,
        version: version.to_string(),
        author,
        description_map,
        template_vars,
        impl_template,
        module_type,
        update_json,
        kam_toml,
    })
}
