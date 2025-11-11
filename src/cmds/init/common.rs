use colored::{Color, Colorize};
use rust_embed::RustEmbed;
use std::io::Cursor;
use std::path::Path;
use tempfile::TempDir;
use flate2::read::GzDecoder;
use tar::Archive as TarArchive;

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

pub fn extract_builtin_template(template_type: &str) -> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn std::error::Error>> {
    // Debug: list all embedded files
    println!("Available embedded files:");
    for file in Assets::iter() {
        println!("  {}", file.as_ref());
    }

    // Accept legacy short keys ("tmpl","lib") as well as canonical values
    // used in kam.toml ("template","library","kam"), and also accept
    // direct asset base names like "tmpl_template".
    // Map template type to a stable base name (no version numbers).
    let (base_name, folder_name): (&str, &str) = match template_type {
        "tmpl" | "template" | "tmpl_template" => ("tmpl_template", "tmpl_template"),
        "lib" | "library" | "lib_template" => ("lib_template", "lib_template"),
        "kam" | "kam_template" => ("kam_template", "kam_template"),
        _ => return Err("Unknown template type".into()),
    };

    println!("Extracting template: {}, base: {}", template_type, base_name);

    // Try a list of fixed candidate filenames (no wildcard). We no longer
    // attempt to find versioned filenames - templates must be packaged using
    // one of these canonical names. Try both direct and `tmpl/` prefixed
    // locations depending on asset packaging.
    let candidates = vec![
        format!("{}.tar.gz", base_name),
        format!("{}-src.tar.gz", base_name),
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

    let file = found.ok_or("Template not found")?;
    println!("Found template data, size: {}", file.data.len());

    let temp_dir = TempDir::new()?;

    // Assume a gzipped tar archive and unpack it.
    let cursor = Cursor::new(file.data.as_ref());
    let gz = GzDecoder::new(cursor);
    let mut archive = TarArchive::new(gz);
    archive.unpack(temp_dir.path())?;

    let template_path = temp_dir.path().join(folder_name);
    println!("Template path exists: {}", template_path.exists());
    println!("Contents of template_path:");
    for entry in std::fs::read_dir(&template_path)? {
        println!("  {}", entry?.path().display());
    }
    Ok((temp_dir, template_path))
}
