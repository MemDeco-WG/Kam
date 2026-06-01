use crate::errors::KamError;
use crate::template::TemplateCacheManager;

use super::args::{CacheArgs, CacheCommands, TemplateCacheCommands};

fn list_template_cache() -> Result<(), KamError> {
    use crate::utils::Utils;

    let templates = TemplateCacheManager::list_local_templates()?;
    if templates.is_empty() {
        Utils::info(crate::i18n::tr("cache.no_templates"));
    } else {
        Utils::section(crate::i18n::tr("cache.local_cached_templates"));
        for tmpl in templates {
            Utils::info(&tmpl);
        }
    }

    Ok(())
}

fn clean_template_cache() -> Result<(), KamError> {
    use crate::utils::Utils;

    let cache_dir = TemplateCacheManager::get_cache_dir()?;
    if cache_dir.exists() {
        std::fs::remove_dir_all(&cache_dir).map_err(KamError::Io)?;
        std::fs::create_dir_all(&cache_dir).map_err(KamError::Io)?;
        Utils::success(crate::i18n::tr("cache.cleaned_successfully"));
    } else {
        Utils::info(crate::i18n::tr("cache.directory_empty_or_not_exists"));
    }

    Ok(())
}

fn add_template_cache(name: &str, path: &std::path::Path) -> Result<(), KamError> {
    use crate::utils::Utils;

    TemplateCacheManager::install_template(name, path)?;
    Utils::success(&trf!("cache.template_added", name, path.display()));
    Ok(())
}

fn remove_template_cache(name: &str) -> Result<(), KamError> {
    use crate::utils::Utils;

    TemplateCacheManager::remove_template(name)?;
    Utils::success(&trf!("cache.template_removed", name));
    Ok(())
}

fn show_template_cache_root() -> Result<(), KamError> {
    let cache_dir = TemplateCacheManager::get_cache_dir()?;
    if let Some(root) = cache_dir.parent() {
        println!("{}", root.display());
    } else {
        println!("{}", cache_dir.display());
    }
    Ok(())
}

fn handle_template_cache_command(command: TemplateCacheCommands) -> Result<(), KamError> {
    match command {
        TemplateCacheCommands::List => list_template_cache(),
        TemplateCacheCommands::Clean => clean_template_cache(),
        TemplateCacheCommands::Add { name, path } => add_template_cache(&name, &path),
        TemplateCacheCommands::Remove { name } => remove_template_cache(&name),
        TemplateCacheCommands::Path => show_template_cache_root(),
    }
}

/// 处理模板缓存相关的命令
///
/// # Errors
///
/// Returns `KamError` when cache operations or I/O fail (e.g., file system
/// operations or template install/remove errors).
#[allow(clippy::too_many_lines)] // TODO: split this function into smaller helpers
pub fn run(args: CacheArgs) -> Result<(), KamError> {
    use crate::utils::Utils;
    match args.command {
        CacheCommands::List => list_template_cache()?,
        CacheCommands::Clean => clean_template_cache()?,
        CacheCommands::Add { name, path } => add_template_cache(&name, &path)?,
        CacheCommands::Remove { name } => remove_template_cache(&name)?,
        CacheCommands::Path => show_template_cache_root()?,
        CacheCommands::Templates(subargs) => handle_template_cache_command(subargs.command)?,

        CacheCommands::Modules(subargs) => {
            // Module cache root (same logic as repo's cache_root_dir())
            let cache_root = crate::cmds::repo::cache_root_dir()?;

            match subargs.command {
                super::args::ModuleCacheCommands::Path => {
                    println!("{}", cache_root.display());
                }

                super::args::ModuleCacheCommands::List => {
                    // List index_*.json files (search index) and modules/<id>.json (module caches)
                    let mut found_index = false;
                    if cache_root.exists()
                        && let Ok(rd) = std::fs::read_dir(&cache_root)
                    {
                        for e in rd.flatten() {
                            let p = e.path();
                            if p.is_file()
                                && let Some(name) = p.file_name().and_then(|n| n.to_str())
                                && name.starts_with("index_")
                                && std::path::Path::new(name)
                                    .extension()
                                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                            {
                                if !found_index {
                                    Utils::section(crate::i18n::tr("cache.modules.index_files"));
                                    found_index = true;
                                }
                                if let Ok(meta) = p.metadata() {
                                    Utils::info(&trf!(
                                        "cache.modules.index_entry",
                                        name,
                                        meta.len()
                                    ));
                                } else {
                                    Utils::info(name);
                                }
                            }
                        }
                    }
                    if !found_index {
                        Utils::info(crate::i18n::tr("cache.modules.no_index_files"));
                    }

                    let modules_dir = cache_root.join("modules");
                    if modules_dir.exists()
                        && let Ok(rd) = std::fs::read_dir(&modules_dir)
                    {
                        Utils::section(crate::i18n::tr("cache.modules.detail_cache"));
                        for e in rd.flatten() {
                            let p = e.path();
                            if p.is_file()
                                && let Some(name) = p.file_name().and_then(|n| n.to_str())
                            {
                                Utils::info(name);
                            }
                        }
                    }
                }

                super::args::ModuleCacheCommands::Clean => {
                    if cache_root.exists()
                        && let Ok(rd) = std::fs::read_dir(&cache_root)
                    {
                        // remove index_*.json files
                        for e in rd.flatten() {
                            let p = e.path();
                            if p.is_file()
                                && let Some(name) = p.file_name().and_then(|n| n.to_str())
                                && name.starts_with("index_")
                                && std::path::Path::new(name)
                                    .extension()
                                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                            {
                                let _ = std::fs::remove_file(&p);
                            }
                        }
                        // remove modules directory if present
                        let modules_dir = cache_root.join("modules");
                        if modules_dir.exists() {
                            let _ = std::fs::remove_dir_all(&modules_dir);
                        }
                        Utils::success(crate::i18n::tr("cache.modules.cleaned_successfully"));
                    } else {
                        Utils::info(crate::i18n::tr(
                            "cache.modules.directory_empty_or_not_exists",
                        ));
                    }
                }

                super::args::ModuleCacheCommands::Remove { name } => {
                    // Try exact filename in cache root
                    let target = cache_root.join(&name);
                    if target.exists() {
                        std::fs::remove_file(&target).map_err(KamError::Io)?;
                        Utils::success(&trf!("cache.modules.removed", name));
                    } else {
                        // Try modules/<name>.json
                        let mpath = cache_root.join("modules").join(format!("{name}.json"));
                        if mpath.exists() {
                            std::fs::remove_file(&mpath).map_err(KamError::Io)?;
                            Utils::success(&trf!("cache.modules.removed_module_cache", name));
                        } else {
                            // Fallback: try any file containing the name
                            let mut removed = false;
                            if cache_root.exists()
                                && let Ok(rd) = std::fs::read_dir(&cache_root)
                            {
                                for e in rd.flatten() {
                                    let p = e.path();
                                    if p.is_file()
                                        && let Some(fname) = p.file_name().and_then(|n| n.to_str())
                                        && fname.contains(&name)
                                    {
                                        let _ = std::fs::remove_file(&p);
                                        Utils::info(&trf!("cache.modules.removed", fname));
                                        removed = true;
                                    }
                                }
                            }
                            if !removed {
                                Utils::info(&trf!("cache.modules.no_matching_cache_file", name));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
