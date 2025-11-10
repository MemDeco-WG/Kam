use colored::{Color, Colorize};
use rust_embed::RustEmbed;
use std::io::Cursor;
use std::path::Path;
use tempfile::TempDir;
use zip::ZipArchive;

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

    let (zip_name, folder_name): (&str, &str) = match template_type {
        "tmpl" => ("tmpl_template.zip", "tmpl_template"),
        "lib" => ("lib_template.zip", "lib_template"),
        "kam" => ("kam_template.zip", "kam_template"),
        _ => return Err("Unknown template type".into()),
    };

    println!("Extracting template: {}, zip_name: {}", template_type, zip_name);
    let zip_data = Assets::get(zip_name).ok_or("Template not found")?;
    println!("Found zip_data, size: {}", zip_data.data.len());

    let temp_dir = TempDir::new()?;
    println!("Opening zip archive");
    let mut archive = ZipArchive::new(Cursor::new(zip_data.data.as_ref()))?;
    println!("Archive opened, len: {}", archive.len());

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        println!("Processing file: {}", file.name());
        let outpath = temp_dir.path().join(file.name());
        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    let template_path = temp_dir.path().join(folder_name);
    println!("Template path exists: {}", template_path.exists());
    println!("Contents of template_path:");
    for entry in std::fs::read_dir(&template_path)? {
        println!("  {}", entry?.path().display());
    }
    Ok((temp_dir, template_path))
}
