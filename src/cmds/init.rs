use clap::Args;
use colored::{Color, Colorize};
use std::collections::HashMap;
use std::fs::File;
use std::io;

use std::path::Path;
use tempfile::TempDir;
use zip::ZipArchive;

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

    /// Create a library module (no module.prop, provides dependencies)
    #[arg(long)]
    pub lib: bool,

    /// Create a template project
    #[arg(long)]
    pub tmpl: bool,

    /// Template zip file to implement
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
}

/// Run the init command
pub fn run(args: InitArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(&args.path);

    // Determine ID from folder name if not provided
    let id = if let Some(id) = args.id {
        id
    } else {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my_module")
            .to_string()
    };

    // Parse template variables
    let mut template_vars = HashMap::new();
    for var in &args.var {
        if let Some((key, value)) = var.split_once('=') {
            template_vars.insert(key.to_string(), value.to_string());
        } else {
            return Err(format!("Invalid template variable format: {}", var).into());
        }
    }

    let name = args.name.unwrap_or_else(|| "My Module".to_string());
    let version = args.version.unwrap_or_else(|| "1.0.0".to_string());
    let author = args.author.unwrap_or_else(|| "Author".to_string());
    let description = args.description.unwrap_or_else(|| "A module description".to_string());

    // Create name and description maps (English only for simplicity)
    let mut name_map = HashMap::new();
    name_map.insert("en".to_string(), name.clone());

    let mut description_map = HashMap::new();
    description_map.insert("en".to_string(), description.clone());

    // Handle tmpl or impl
    if args.tmpl {
        // Create template project
        // Parse template variables from --var
        let mut variables = std::collections::HashMap::new();
        for var in &args.var {
            if let Some((key, value)) = var.split_once('=') {
                let parts: Vec<&str> = value.split(':').collect();
                if parts.len() == 3 {
                    let var_type = parts[0].to_string();
                    let required = parts[1] == "true";
                    let default = if parts[2].is_empty() { None } else { Some(parts[2].to_string()) };
                    variables.insert(key.to_string(), crate::types::kam_toml::VariableDefinition {
                        var_type,
                        required,
                        default,
                    });
                } else {
                    return Err(format!("Invalid template variable format: {}. Expected type:required:default", var).into());
                }
            } else {
                return Err(format!("Invalid template variable format: {}. Expected key=type:required:default", var).into());
            }
        }
        let mut kt = crate::types::kam_toml::KamToml::new_template(
            id.clone(),
            name_map,
            version.clone(),
            1,
            author.clone(),
            description_map,
            None,
        );
        kt.kam.tmpl = Some(crate::types::kam_toml::TmplSection { used_template: None, variables });
        kt.write_to_dir(path)?;
        let kam_toml_rel = "kam.toml".to_string();
        print_status(&path.join("kam.toml"), &kam_toml_rel, false, args.force);
        // Copy src from my_template without replace
        let template_path = Path::new("my_template");
        if template_path.exists() {
            let src_temp = template_path.join("src").join("{{id}}");
            if src_temp.exists() {
                let src_dir = path.join("src").join(&id);
                std::fs::create_dir_all(&src_dir)?;
                let src_rel = format!("src/{}/", id);
                print_status(&src_dir, &src_rel, true, args.force);
                for entry in std::fs::read_dir(&src_temp)? {
                    let entry = entry?;
                    let filename = entry.file_name();
                    std::fs::copy(entry.path(), src_dir.join(&filename))?;
                    let file_rel = format!("src/{}/{}", id, filename.to_string_lossy());
                    print_status(&src_dir.join(&filename), &file_rel, false, args.force);
                }
            }
        } else {
            return Err("my_template not found".into());
        }
    } else if let Some(impl_zip) = &args.r#impl {
        // Implement template
        let mut zip_id = "unknown".to_string();
        let (template_path, _temp_dir) = if impl_zip.ends_with(".zip") {
            let file = File::open(impl_zip)?;
            let mut archive = ZipArchive::new(file)?;
            let temp_dir = TempDir::new()?;
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let outpath = temp_dir.path().join(file.name());
                if file.name().ends_with('/') {
                    std::fs::create_dir_all(&outpath)?;
                } else {
                    if let Some(p) = outpath.parent() {
                        std::fs::create_dir_all(p)?;
                    }
                    let mut outfile = File::create(&outpath)?;
                    io::copy(&mut file, &mut outfile)?;
                }
            }
            // Assume the zip has a root directory
            let root = if archive.len() > 0 {
                archive.by_index(0).unwrap().name().split('/').next().unwrap_or("").to_string()
            } else {
                "".to_string()
            };
            zip_id = root.clone();
            (temp_dir.path().join(root), Some(temp_dir))
        } else {
            (Path::new(impl_zip).to_path_buf(), None)
        };
        // Load template variables and insert defaults
        let template_kam_path = template_path.join("kam.toml");
        if template_kam_path.exists() {
            let kt_template = crate::types::kam_toml::KamToml::load_from_file(&template_kam_path)?;
            // Update zip_id from the template's id
            zip_id = kt_template.prop.id.clone();
            if let Some(tmpl) = &kt_template.kam.tmpl {
                for (var_name, var_def) in &tmpl.variables {
                    if !template_vars.contains_key(var_name) {
                        if var_def.required {
                            if let Some(default) = &var_def.default {
                                template_vars.insert(var_name.clone(), default.clone());
                            } else {
                                return Err(format!("Required template variable '{}' not provided", var_name).into());
                            }
                        } else if let Some(default) = &var_def.default {
                            template_vars.insert(var_name.clone(), default.clone());
                        }
                    }
                }
            }
        }
        let mut kt = crate::types::kam_toml::KamToml::new_with_current_timestamp(
            id.clone(),
            name_map,
            version.clone(),
            author.clone(),
            description_map,
            None,
        );
        kt.kam.tmpl = Some(crate::types::kam_toml::TmplSection { used_template: Some(zip_id.clone()), variables: std::collections::HashMap::new() });
        kt.write_to_dir(path)?;
        let kam_toml_path = path.join("kam.toml");
        let kam_toml_rel = "kam.toml".to_string();
        print_status(&kam_toml_path, &kam_toml_rel, false, args.force);
        // Replace in kam.toml
        if !template_vars.is_empty() {
            let mut content = std::fs::read_to_string(&kam_toml_path)?;
            for (key, value) in &template_vars {
                let default_value = match key.as_str() {
                    "id" => &id,
                    "name" => &name,
                    "version" => &version,
                    "author" => &author,
                    "description" => &description,
                    _ => continue,
                };
                content = content.replace(default_value, value);
            }
            std::fs::write(&kam_toml_path, content)?;
        }
        // Copy src from template with replace
        if impl_zip.ends_with(".zip") {
            if template_path.exists() {
                let src_temp = template_path.join("src").join(&zip_id);
                if src_temp.exists() {
                    let src_dir = path.join("src").join(&id);
                    std::fs::create_dir_all(&src_dir)?;
                    let src_rel = format!("src/{}/", id);
                    print_status(&src_dir, &src_rel, true, args.force);
                    for entry in std::fs::read_dir(&src_temp)? {
                        let entry = entry?;
                        let filename = entry.file_name();
                        let mut content = std::fs::read_to_string(entry.path())?;
                        for (key, value) in &template_vars {
                            content = content.replace(&format!("{{{{{}}}}}", key), value);
                        }
                        let dest_file = src_dir.join(&filename);
                        std::fs::write(&dest_file, content)?;
                        let file_rel = format!("src/{}/{}", id, filename.to_string_lossy());
                        print_status(&dest_file, &file_rel, false, args.force);
                    }
                }
            } else {
                return Err("Template not found".into());
            }
        } else {
            if template_path.exists() {
                let src_temp = template_path.join("src").join(&zip_id);
                if src_temp.exists() {
                    let src_dir = path.join("src").join(&id);
                    std::fs::create_dir_all(&src_dir)?;
                    let src_rel = format!("src/{}/", id);
                    print_status(&src_dir, &src_rel, true, args.force);
                    for entry in std::fs::read_dir(&src_temp)? {
                        let entry = entry?;
                        let filename = entry.file_name();
                        let mut content = std::fs::read_to_string(entry.path())?;
                        for (key, value) in &template_vars {
                            content = content.replace(&format!("{{{{{}}}}}", key), value);
                        }
                        let dest_file = src_dir.join(&filename);
                        std::fs::write(&dest_file, content)?;
                        let file_rel = format!("src/{}/{}", id, filename.to_string_lossy());
                        print_status(&dest_file, &file_rel, false, args.force);
                    }
                }
            } else {
                return Err("Template not found".into());
            }
        }
    } else {
        // Normal init
        let kt = crate::types::kam_toml::KamToml::new_with_current_timestamp(
            id.clone(),
            name_map,
            version.clone(),
            author.clone(),
            description_map,
            None,
        );
        kt.write_to_dir(path)?;
        let kam_toml_path = path.join("kam.toml");
        let kam_toml_rel = "kam.toml".to_string();
        print_status(&kam_toml_path, &kam_toml_rel, false, args.force);
        // Replace in kam.toml
        if !template_vars.is_empty() {
            let mut content = std::fs::read_to_string(&kam_toml_path)?;
            for (key, value) in &template_vars {
                let default_value = match key.as_str() {
                    "id" => &id,
                    "name" => &name,
                    "version" => &version,
                    "author" => &author,
                    "description" => &description,
                    _ => continue,
                };
                content = content.replace(default_value, value);
            }
            std::fs::write(&kam_toml_path, content)?;
        }
        // Copy src from my_template with replace
        let template_path = Path::new("my_template");
        if template_path.exists() {
            let src_temp = template_path.join("src").join("{{id}}");
            if src_temp.exists() {
                let src_dir = path.join("src").join(&id);
                std::fs::create_dir_all(&src_dir)?;
                let src_rel = format!("src/{}/", id);
                print_status(&src_dir, &src_rel, true, args.force);
                for entry in std::fs::read_dir(&src_temp)? {
                    let entry = entry?;
                    let filename = entry.file_name();
                    let mut content = std::fs::read_to_string(entry.path())?;
                    for (key, value) in &template_vars {
                        content = content.replace(&format!("{{{{{}}}}}", key), value);
                    }
                    let dest_file = src_dir.join(&filename);
                    std::fs::write(&dest_file, content)?;
                    let file_rel = format!("src/{}/{}", id, filename.to_string_lossy());
                    print_status(&dest_file, &file_rel, false, args.force);
                }
            }
        } else {
            return Err("my_template not found".into());
        }
    }

    // For impl, require vars if not empty
    if args.r#impl.is_some() && template_vars.is_empty() {
        return Err("Implementation requires template variables. Use --var key=value".into());
    }

    // If no template vars, set defaults
    if template_vars.is_empty() {
        template_vars.insert("id".to_string(), id.clone());
        template_vars.insert("name".to_string(), name.clone());
        template_vars.insert("version".to_string(), version.clone());
        template_vars.insert("author".to_string(), author.clone());
        template_vars.insert("description".to_string(), description.clone());
    }

    // Helper function for status output
    fn print_status(path: &Path, rel: &str, is_dir: bool, force: bool) {
        if force || !path.exists() {
            let color = if is_dir { Color::Blue } else { Color::Green };
            println!("{}", format!("+ {}", rel).color(color));
        } else {
            println!("{}", format!("~ {}", rel).color(Color::Yellow));
        }
    }

    // Folders
    if args.meta_inf {
        let meta_inf_dir = path.join("META-INF");
        let meta_inf_rel = "META-INF/".to_string();
        if !meta_inf_dir.exists() {
            std::fs::create_dir_all(&meta_inf_dir)?;
        }
        print_status(&meta_inf_dir, &meta_inf_rel, true, args.force);
    }

    if args.web_root {
        let web_root_dir = path.join("WEB-ROOT");
        let web_root_rel = "WEB-ROOT/".to_string();
        if !web_root_dir.exists() {
            std::fs::create_dir_all(&web_root_dir)?;
        }
        print_status(&web_root_dir, &web_root_rel, true, args.force);
    }

    println!("Initialized Kam project in {}", path.display());

    Ok(())
}
