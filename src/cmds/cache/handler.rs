use crate::errors::KamError;
use crate::template::TemplateCacheManager;

use super::args::{CacheArgs, CacheCommands};

// 处理模板缓存相关的命令
pub fn run(args: CacheArgs) -> Result<(), KamError> {
    match args.command {
        CacheCommands::List => {
            // 列出所有缓存的模板
            let templates = TemplateCacheManager::list_local_templates()?;
            use crate::utils::Utils;
            if templates.is_empty() {
                Utils::info(crate::i18n::tr_key("No templates found in local cache."));
            } else {
                Utils::section(crate::i18n::tr_key("Local Cached Templates"));
                for tmpl in templates {
                    Utils::info(&tmpl);
                }
            }
        }
        CacheCommands::Clean => {
            // 清理缓存（删除所有模板）
            let cache_dir = TemplateCacheManager::get_cache_dir()?;
            if cache_dir.exists() {
                // 直接删除整个目录然后重建，简单粗暴
                std::fs::remove_dir_all(&cache_dir).map_err(KamError::Io)?;
                std::fs::create_dir_all(&cache_dir).map_err(KamError::Io)?;
                use crate::utils::Utils;
                Utils::success(crate::i18n::tr_key("Cache cleaned successfully"));
            } else {
                use crate::utils::Utils;
                Utils::info(crate::i18n::tr_key("Cache directory is already empty or does not exist."));
            }
        }
        CacheCommands::Add { name, path } => {
            // 添加模板到缓存
            TemplateCacheManager::install_template(&name, &path)?;
            use crate::utils::Utils;
            Utils::success(&trf!("Template '{}' added to cache from {}", name, path.display()));
        }
        CacheCommands::Remove { name } => {
            // 从缓存删除模板
            TemplateCacheManager::remove_template(&name)?;
            use crate::utils::Utils;
            Utils::success(&trf!("Template '{}' removed from cache", name));
        }
        CacheCommands::Path => {
            // 显示缓存目录路径
            let cache_dir = TemplateCacheManager::get_cache_dir()?;
            // 显示父目录（.kam），这样用户知道配置在哪
            if let Some(root) = cache_dir.parent() {
                println!("{}", root.display());
            } else {
                println!("{}", cache_dir.display());
            }
        }
    }

    Ok(())
}
