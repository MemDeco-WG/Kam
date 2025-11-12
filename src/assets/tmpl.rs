use rust_embed::RustEmbed;

// tmpl template

// kam_template

// repo_template

// venv_template

#[derive(RustEmbed)]
#[folder = "src/assets/tmpl"]
pub struct TmplAssets;
