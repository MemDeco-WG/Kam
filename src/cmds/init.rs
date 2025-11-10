use clap::Args;
use std::collections::HashMap;
use std::path::Path;

mod common;
mod template_vars;
mod normal;
mod template;
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
}

/// Run the init command
pub fn run(args: InitArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(&args.path);

    // Determine ID from folder name if not provided
    let id = if let Some(id) = args.id.clone() {
        id
    } else {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my_module")
            .to_string()
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

    // Handle tmpl or impl
    if args.tmpl {
        template::init_template(&path, &id, name_map, &version, &author, description_map, &args.var, args.force)?;
    } else if let Some(impl_zip) = &args.r#impl {
        impl_mod::init_impl(&path, &id, name_map, &version, &author, description_map, impl_zip, &mut template_vars, args.force)?;
    } else {
        normal::init_normal(&path, &id, name_map, &version, &author, description_map, &template_vars, args.force)?;
    }

    post_process::post_process(&path, &args, &mut template_vars, &id, &name, &version, &author, &description)?;

    Ok(())
}
