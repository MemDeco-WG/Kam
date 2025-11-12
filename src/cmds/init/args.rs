use clap::Args;

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

    /// Create a library module (provides dependencies)
    #[arg(long)]
    pub lib: bool,

    /// Create a template project
    #[arg(long)]
    pub tmpl: bool,

    /// Create a repo module repository project
    #[arg(long)]
    pub repo: bool,
}
