use crate::errors::KamError;
use crate::template::TemplateCacheManager;
use colored::Colorize;

pub mod args;
pub mod export;
pub mod import;
pub mod pull;

pub use args::{TmplArgs, TmplCommands};

/// Run the tmpl command
pub fn run(args: TmplArgs) -> Result<(), KamError> {
    match args.command {
        TmplCommands::List => list_templates(),
        TmplCommands::Import { path, name, force } => import::import_template(&path, name, force),
        TmplCommands::Pull { url, global } => pull::run_pull(url, global),
        TmplCommands::Update { global } => pull::run_update(global),
        TmplCommands::Export {
            templates,
            output,
            force,
        } => export::export_template(&templates, &output, force),
        TmplCommands::Remove { name } => remove_template(&name),
        TmplCommands::Path => show_cache_path(),
    }
}

fn list_templates() -> Result<(), KamError> {
    let templates = TemplateCacheManager::list_local_templates()?;

    if templates.is_empty() {
        println!("{} No templates found in cache", "!".yellow());
        println!("\nUse {} to import templates", "kam tmpl import".bold());
    } else {
        println!("{} Templates in cache:", "✓".green());
        for template in &templates {
            println!("  • {}", template);
        }
        println!(
            "\n{} {} template(s) available",
            "✓".green(),
            templates.len()
        );
    }

    Ok(())
}

fn remove_template(name: &str) -> Result<(), KamError> {
    TemplateCacheManager::remove_template(name)?;
    println!(
        "{} Template '{}' removed successfully",
        "✓".green(),
        name.bold()
    );
    Ok(())
}

fn show_cache_path() -> Result<(), KamError> {
    let cache_dir = TemplateCacheManager::get_cache_dir()?;
    println!("{}", cache_dir.display());
    Ok(())
}
