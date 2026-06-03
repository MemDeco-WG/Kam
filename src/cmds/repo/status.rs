use super::{cache, effective_base_url};
use crate::errors::KamError;

pub(crate) fn handle_repo_status(modules_url: Option<&str>, quiet: bool) -> Result<(), KamError> {
    let base = effective_base_url(modules_url);
    let cache_root = cache::cache_root_dir()?;
    let index_path = cache::index_cache_path(&base)?;
    let module_cache_dir = cache_root.join("modules");
    let module_cache_count = count_json_files(&module_cache_dir);
    let index_entries = cache::read_local_index(&base).map_or(None, |entries| Some(entries.len()));

    if quiet {
        println!("base_url={base}");
        println!("cache_root={}", cache_root.display());
        println!("index_path={}", index_path.display());
        println!("index_present={}", index_path.exists());
        println!(
            "index_entries={}",
            index_entries.map_or_else(|| "unknown".to_string(), |count| count.to_string())
        );
        println!("module_metadata={module_cache_count}");
        return Ok(());
    }

    println!("Repository     : {base}");
    println!("Cache Root     : {}", cache_root.display());
    println!("Index Path     : {}", index_path.display());
    println!(
        "Index Present  : {}",
        if index_path.exists() { "yes" } else { "no" }
    );
    println!(
        "Index Entries  : {}",
        index_entries.map_or_else(|| "unknown".to_string(), |count| count.to_string())
    );
    println!("Module Metadata: {module_cache_count}");
    Ok(())
}

fn count_json_files(dir: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            let path = entry.path();
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        })
        .count()
}
