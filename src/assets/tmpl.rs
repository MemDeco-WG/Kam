use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "src/assets/tmpl"]
pub struct TmplAssets;
