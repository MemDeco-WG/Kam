use std::collections::HashMap;
use std::path::Path;

use super::InitArgs;
use crate::errors::KamError;

pub fn post_process(
    path: &Path,
    args: &InitArgs,
    template_vars: &mut HashMap<String, String>,
    id: &str,
    name: &str,
    version: &str,
    author: &str,
    description: &str,
) -> Result<(), KamError> {
    // For impl, require vars if not empty
    if args.r#impl.is_some() && template_vars.is_empty() {
        return Err(KamError::ImplRequiresVars(
            "Implementation requires template variables. Use --var key=value".to_string(),
        ));
    }

    // If no template vars, set defaults
    if template_vars.is_empty() {
        template_vars.insert("id".to_string(), id.to_string());
        template_vars.insert("name".to_string(), name.to_string());
        template_vars.insert("version".to_string(), version.to_string());
        template_vars.insert("author".to_string(), author.to_string());
        template_vars.insert("description".to_string(), description.to_string());
    }

    // Folders
    if args.meta_inf {
        let meta_inf_dir = path.join("META-INF");
        let meta_inf_rel = "META-INF/".to_string();
        if !meta_inf_dir.exists() {
            std::fs::create_dir_all(&meta_inf_dir)?;
        }
        crate::utils::Utils::print_status(
            &meta_inf_dir,
            &meta_inf_rel,
            crate::utils::PrintOp::Create { is_dir: true },
            args.force,
        );
    }

    if args.web_root {
        let web_root_dir = path.join("WEB-ROOT");
        let web_root_rel = "WEB-ROOT/".to_string();
        if !web_root_dir.exists() {
            std::fs::create_dir_all(&web_root_dir)?;
        }
        crate::utils::Utils::print_status(
            &web_root_dir,
            &web_root_rel,
            crate::utils::PrintOp::Create { is_dir: true },
            args.force,
        );
    }

    println!("Initialized Kam project in {}", path.display());

    Ok(())
}
