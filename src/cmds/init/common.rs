use colored::{Color, Colorize};
use rust_embed::RustEmbed;

use std::path::Path;

use crate::errors::KamError;

#[derive(RustEmbed)]
#[folder = "src/assets/"]
struct Assets;

pub fn print_status(path: &Path, rel: &str, is_dir: bool, force: bool) {
    if force || !path.exists() {
        let color = if is_dir { Color::Blue } else { Color::Green };
        println!("{}", format!("+ {}", rel).color(color));
    } else {
        println!("{}", format!("~ {}", rel).color(Color::Yellow));
    }
}

pub fn extract_builtin_template(template_type: &str) -> Result<(std::path::PathBuf, tempfile::TempDir), KamError> {
    // Debug: list all embedded files
    println!("Available embedded files:");
    for file in Assets::iter() {
        println!("  {}", file.as_ref());
    }

    // Accept legacy short keys ("tmpl","lib") as well as canonical values
    // used in kam.toml ("template","library","kam"), and also accept
    // direct asset base names like "tmpl_template".
    // Map template type to a stable base name (no version numbers).
    let (base_name, _folder_name): (&str, &str) = match template_type {
        "tmpl" | "template" | "tmpl_template" => ("tmpl_template", "tmpl_template"),
        "lib" | "library" | "lib_template" => ("lib_template", "lib_template"),
        "kam" | "kam_template" => ("kam_template", "kam_template"),
        _ => return Err(KamError::UnknownTemplateType("Unknown template type".to_string())),
    };

    println!("Extracting template: {}, base: {}", template_type, base_name);

    // Try a list of fixed candidate filenames (no wildcard). We no longer
    // attempt to find versioned filenames - templates must be packaged using
    // one of these canonical names. Try both direct and `tmpl/` prefixed
    // locations depending on asset packaging.
    let candidates = vec![
        format!("{}.tar.gz", base_name),
    ];

    let mut found: Option<rust_embed::EmbeddedFile> = None;
    for cand in &candidates {
        if let Some(f) = Assets::get(cand) {
            println!("Found asset: {}", cand);
            found = Some(f);
            break;
        }
        let pref = format!("tmpl/{}", cand);
        if let Some(f) = Assets::get(&pref) {
            println!("Found asset: {}", pref);
            found = Some(f);
            break;
        }
    }

    let file = found.ok_or(KamError::TemplateNotFound("Template not found".to_string()))?;
    println!("Found template data, size: {}", file.data.len());

    // Write embedded data to a temp file and extract using extract_archive_to_temp
    let temp_file = tempfile::NamedTempFile::new()?;
    std::fs::write(temp_file.path(), &file.data)?;
    let (temp_dir, template_path) = super::template::extract_archive_to_temp(temp_file.path())?;
    Ok((template_path, temp_dir))
}
