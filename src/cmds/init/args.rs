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

    /// Project name (default: "Example Module Name")
    #[arg(long)]
    pub project_name: Option<String>,

    /// Project version (default: "1.0.0")
    #[arg(long)]
    pub version: Option<String>,

    /// Author name (default: "Your Name")
    #[arg(long)]
    pub author: Option<String>,

    /// Update JSON URL (default: auto-generated from git)
    #[arg(long)]
    pub update_json: Option<String>,

    /// Description (default: "Describe your module here")
    #[arg(long)]
    pub description: Option<String>,

    /// Force overwrite existing files
    #[arg(short, long)]
    pub force: bool,

    /// Deprecated: Template source to implement (local path, URL, or git repo)
    /// NOTE: This option has been removed from the CLI. Use -t/--template
    /// and --tmpl to select built-in templates (e.g., kam_template, ak3_template).
    #[arg(skip)]
    pub r#impl: Option<String>,

    /// Template variables in key=value format
    #[arg(long)]
    pub var: Vec<String>,

    /// Template to use (built-in ID or local path)
    #[arg(short, long)]
    pub template: Option<String>,

    /// Create a template project
    /// Template id: "tmpl_template"
    #[arg(long)]
    pub tmpl: bool,
}
