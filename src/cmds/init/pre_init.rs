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
        .unwrap_or("Example Module Name");
    let description_str = args
        .description
        .as_deref()
        .unwrap_or_else(|| match module_type {
            ModuleType::Kam => "Describe your module here",
            ModuleType::Template => "Describe your template here",
        });
    template_vars.insert("project_name".to_string(), project_name_str.to_string());
    template_vars.insert("description".to_string(), description_str.to_string());

    // Determine ID: use --id if provided, otherwise use folder name
    let id = if let Some(custom_id) = &args.id {
        custom_id.clone()
    } else if args.name == "." {
        std::env::current_dir()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
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

    // Determine author
    let author = args.author.as_deref().unwrap_or("Your Name").to_string();

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

    // Set name and description
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
