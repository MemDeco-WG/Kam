pub mod tmpl;

use rust_embed::RustEmbed;
pub use tmpl::TmplAssets;

#[derive(RustEmbed)]
#[folder = "src/assets/"]
pub struct Assets;
