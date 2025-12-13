use clap::Args;

/// Arguments for the init command
#[derive(Args, Debug)]
pub struct InitArgs {
    /// Path to initialize the project
    /// When running interactively (`-i`/`--interactive`) this PATH may be omitted;
    /// the interactive flow will prompt for it instead.
    #[arg(value_name = "PATH", required_unless_present = "interactive")]
    pub name: Option<String>,

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

    /// Run the init interactively; ask for required values
    #[arg(short = 'i', long = "interactive")]
    pub interactive: bool,

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
