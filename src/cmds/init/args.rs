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

    /// Deprecated: Template source to implement (local path, URL, or git repo)
    /// NOTE: This option has been removed from the CLI. Use the dedicated
    /// template selection flags (e.g. --kam, --tmpl, --repo, --venv)
    /// which correspond to builtin template IDs instead.
    #[arg(skip)]
    pub r#impl: Option<String>,

    /// Template variables in key=value format
    #[arg(long)]
    pub var: Vec<String>,

    /// Create a kam module
    /// Template id: "kam_template"
    #[arg(long)]
    pub kam: bool,

    

    /// Create a template project
    /// Template id: "tmpl_template"
    #[arg(long)]
    pub tmpl: bool,

    /// Create a repo module repository project
    /// Template id: "repo_template"
    #[arg(long)]
    pub repo: bool,

    /// Create a venv template
    /// Template id: "venv_template"
    #[arg(long)]
    pub venv: bool,
}
