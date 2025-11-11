use clap::Args;
use std::collections::HashMap;
use std::path::Path;

mod common;
mod template_vars;
mod kam;
mod template;
mod repo;
mod impl_mod;
mod post_process;



/// Arguments for the init command
#[derive(Args, Debug)]
pub struct InitArgs {
    /// Path to initialize the project (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Project ID (default: folder name)
    #[arg(long)]
    pub id: Option<String>,

    /// Project name (default: "My Module")
    #[arg(long)]
    pub name: Option<String>,

    /// Project version (default: "1.0.0")
    #[arg(long)]
    pub version: Option<String>,

    /// Author name (default: "Author")
    #[arg(long)]
    pub author: Option<String>,

    /// Description (default: "A module description")
    #[arg(long)]
    pub description: Option<String>,

    /// Force overwrite existing files
    #[arg(short, long)]
    pub force: bool,

    /// Create a library module (no module.prop, provides dependencies)
    #[arg(long)]
    pub lib: bool,

    /// Create a template project
    #[arg(long)]
    pub tmpl: bool,

    /// Create a kam module (supports kernelsu/apatch/magisk)
    #[arg(long)]
    pub kam: bool,

    /// Template zip file to implement
    #[arg(long)]
    pub r#impl: Option<String>,

    /// Create META-INF folder for traditional Magisk modules
    #[arg(long)]
    pub meta_inf: bool,

    /// Create WEB-ROOT folder for web interface
    #[arg(long)]
    pub web_root: bool,

    /// Template variables in key=value format
    #[arg(long)]
    pub var: Vec<String>,
    /// Initialize a kam module repository project (uses tmpl/repo_templeta)
    #[arg(long)]
    pub module_repo: bool,
}

/// Run the init command
pub fn run(args: InitArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(&args.path);

    // Ensure cache is initialized early so templates and builtins are available.
    // Try automatic initialization; if it fails, print a helpful hint and continue.
    match crate::cache::KamCache::new().and_then(|c| c.ensure_dirs()) {
        Ok(_) => {
            // Cache ready
        }
        Err(e) => {
            println!("Note: failed to initialize Kam cache: {}", e);
            println!("You can initialize the cache by running: 'kam cache info' or 'kam sync'.");
            println!("Continuing init without a cache - some templates or modules may not be available.");
        }
    }

    // Validate conflicting flags
    let module_flags = [args.tmpl, args.lib, args.kam].iter().filter(|&&x| x).count();
    if module_flags > 1 {
        return Err("Cannot specify multiple module types: --tmpl, --lib, --kam".into());
    }

    // Determine module type (use canonical kam.toml serialization values)
    // ModuleType is serialized as lowercase strings: "kam", "template", "library", "repo"
    let module_type = if args.tmpl {
        "template"
    } else if args.lib {
        "library"
    } else {
        "kam"
    };

    // Environment variables used in this project (collected):
    // - GITHUB_TOKEN       : used by `publish` as a default auth token when --token is not provided
    // - KAM_PUBLISH_TOKEN  : alternative token for publish (fallback)
    // - KAM_LOCAL_REPO     : used by `sync` as a candidate local repository path
    // - HOME / USERPROFILE : used by cache code to locate the user's home directory
    //
    // Determine ID from folder name if not provided. If the provided `path` is a
    // relative marker like `.` then `path.file_name()` may be None. In that case
    // fall back to the current working directory's name. If that also fails,
    // fall back to the literal "my_module" to preserve previous behaviour.
    let id = if let Some(id) = args.id.clone() {
        id
    } else {
        let name_from_path = path.file_name().and_then(|n| n.to_str().map(|s| s.to_string()));
        let name = name_from_path.or_else(|| {
            std::env::current_dir().ok().and_then(|p| p.file_name().and_then(|n| n.to_str().map(|s| s.to_string())))
        });
        name.unwrap_or_else(|| "my_module".to_string())
    };

    // Parse template variables
    let mut template_vars = template_vars::parse_template_vars(&args.var)?;

    let name = args.name.clone().unwrap_or_else(|| "My Module".to_string());
    let version = args.version.clone().unwrap_or_else(|| "1.0.0".to_string());
    let author = args.author.clone().unwrap_or_else(|| "Author".to_string());
    let description = args.description.clone().unwrap_or_else(|| "A module description".to_string());

    // Create name and description maps (English only for simplicity)
    let mut name_map = HashMap::new();
    name_map.insert("en".to_string(), name.clone());

    let mut description_map = HashMap::new();
    description_map.insert("en".to_string(), description.clone());

    // Handle special module repo init, tmpl or impl
    if args.module_repo {
        // Initialize a kam module repository project using the repo template
        repo::init_repo(&path, &id, name_map, &version, &author, description_map, &args.var, args.force)?;
    } else if args.tmpl {
        // Pass optional implementation/template selector through --impl
        template::init_template(&path, &id, name_map, &version, &author, description_map, &args.var, args.r#impl.clone(), args.force)?;
    } else if let Some(impl_zip) = &args.r#impl {
        impl_mod::init_impl(&path, &id, name_map, &version, &author, description_map, impl_zip, &mut template_vars, args.force)?;
    } else {
        kam::init_kam(&path, &id, name_map, &version, &author, description_map, &template_vars, args.force, module_type)?;
    }

    post_process::post_process(&path, &args, &mut template_vars, &id, &name, &version, &author, &description)?;

    Ok(())
}
