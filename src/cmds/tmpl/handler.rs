use colored::Colorize;

use crate::errors::KamError;
use crate::template::TemplateCacheManager;

use super::args::{TmplArgs, TmplCommands};
use super::{export, import, pull};

// 模板命令的入口，就是处理各种模板相关的操作
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

    use crate::utils::Utils;
    if templates.is_empty() {
        Utils::warn(crate::i18n::tr_key("tmpl.no_templates_in_cache"));
        println!();
        Utils::info(&trf!("tmpl.use_import_command", "kam tmpl import".bold()));
    } else {
        Utils::section("Templates in Cache");
        for template in &templates {
            Utils::info(template);
        }
        println!();
        Utils::success(&format!("{} template(s) available", templates.len()));
    }

    Ok(())
}

fn remove_template(name: &str) -> Result<(), KamError> {
    TemplateCacheManager::remove_template(name)?;
    use crate::utils::Utils;
    Utils::success(&format!("Template '{}' removed successfully", name));
    Ok(())
}

fn show_cache_path() -> Result<(), KamError> {
    let cache_dir = TemplateCacheManager::get_cache_dir()?;
    println!("{}", cache_dir.display());
    Ok(())
}
