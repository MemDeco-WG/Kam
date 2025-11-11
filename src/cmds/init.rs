use clap::{Args, Subcommand};
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

    /// Subcommands for init (e.g. `kam init repo [path]`)
    #[command(subcommand)]
    pub action: Option<InitAction>,
}

/// Subcommands for `kam init`
#[derive(Subcommand, Debug)]
pub enum InitAction {
    /// Initialize a kam module repository project from template
    Repo(RepoArgs),
    /// Initialize a template project (tmpl)
    Tmpl(TmplArgs),
    /// Initialize a library module project
    Lib(LibArgs),
}

/// Arguments for `kam init repo [path]`
#[derive(Args, Debug)]
pub struct RepoArgs {
    /// Path to initialize the repo (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Force overwrite existing files
    #[arg(short, long)]
    pub force: bool,

    /// Template variables (key=value)
    #[arg(long)]
    pub var: Vec<String>,
}

/// Arguments for `kam init tmpl [path]`
#[derive(Args, Debug)]
pub struct TmplArgs {
    /// Path to initialize the template project (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Template selector or implementation zip
    #[arg(long)]
    pub r#impl: Option<String>,

    /// Force overwrite existing files
    #[arg(short, long)]
    pub force: bool,

    /// Template variables (key=value)
    #[arg(long)]
    pub var: Vec<String>,
}

#[derive(Args, Debug)]
pub struct LibArgs {
    /// Path to initialize the library project (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Force overwrite existing files
    #[arg(short, long)]
    pub force: bool,

    /// Template variables (key=value)
    #[arg(long)]
    pub var: Vec<String>,
}

#[derive(Args, Debug)]
pub struct KamArgs {
    /// Path to initialize the kam project (default: current directory)
    #[arg(default_value = ".")]
    pub path: String,

    /// Force overwrite existing files
    #[arg(short, long)]
    pub force: bool,

    /// Template variables (key=value)
    #[arg(long)]
    pub var: Vec<String>,
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

    // If a subcommand was provided (e.g. `kam init repo <path>`), handle it first.
    if let Some(action) = &args.action {
        match action {
            InitAction::Repo(rargs) => {
                let target = Path::new(&rargs.path);
                repo::init_repo(target, &id, name_map.clone(), &version, &author, description_map.clone(), &rargs.var, rargs.force)?;
                post_process::post_process(target, &args, &mut template_vars, &id, &name, &version, &author, &description)?;
                return Ok(());
            }
            InitAction::Tmpl(targs) => {
                let target = Path::new(&targs.path);
                template::init_template(target, &id, name_map.clone(), &version, &author, description_map.clone(), &targs.var, targs.r#impl.clone(), targs.force)?;
                post_process::post_process(target, &args, &mut template_vars, &id, &name, &version, &author, &description)?;
                return Ok(());
            }
            InitAction::Lib(largs) => {
                let target = Path::new(&largs.path);
                // library init uses init_kam with module_type = "library"
                kam::init_kam(target, &id, name_map.clone(), &version, &author, description_map.clone(), &template_vars, largs.force, "library")?;
                post_process::post_process(target, &args, &mut template_vars, &id, &name, &version, &author, &description)?;
                return Ok(());
            }

        }
    }

    // Handle tmpl or impl (repo should be used via the `kam init repo <path>` subcommand)
    if args.tmpl {
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
