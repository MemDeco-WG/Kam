use clap::Args;

/// Arguments for the init command
#[derive(Args, Debug)]
pub struct InitArgs {
    /// Path to initialize the project
    #[arg(value_name = "PATH")]
    pub name: String,

    /// Project ID (default: folder name)
    #[arg(long)]
    pub id: Option<String>,

    /// Project name (default: "My Module")
    #[arg(long)]
    pub project_name: Option<String>,

    /// Project version (default: "1.0.0")
    #[arg(long)]
    pub version: Option<String>,

    /// Author name (default: "Author")
    #[arg(long)]
    pub author: Option<String>,

    /// Update JSON URL (default: auto-generated from git)
    #[arg(long)]
    pub update_json: Option<String>,

    /// Description (default: "A module description")
    #[arg(long)]
    pub description: Option<String>,

    /// Force overwrite existing files
    #[arg(short, long)]
    pub force: bool,

    /// Template source to implement (local path, URL, or git repo)
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

    /// Create a kam module
    #[arg(long)]
    pub kam: bool,

    /// Create a library module (provides dependencies)
    #[arg(long)]
    pub lib: bool,

    /// Create a template project
    #[arg(long)]
    pub tmpl: bool,

    /// Create a repo module repository project
    #[arg(long)]
    pub repo: bool,

    /// Create a venv template
    #[arg(long)]
    pub venv: bool,
}
