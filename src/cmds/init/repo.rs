use std::collections::HashMap;
use std::path::Path;
use std::fs;
use crate::types::modules::base::KamToml;
use crate::errors::KamError;
use chrono;

/// Initialize a kam module repository project by copying `tmpl/repo_templeta` into target
pub fn init_repo(
    path: &Path,
    id: &str,
    name_map: HashMap<String, String>,
    version: &str,
    author: &str,
    description_map: HashMap<String, String>,
    _vars: &[String],
    force: bool,
) -> Result<(), KamError> {
    // Determine source template dir relative to the crate root
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmpl").join("repo_templeta");
    if !src_dir.exists() {
        return Err(KamError::RepoTemplateNotFound(format!("Repo template not found: {}", src_dir.display())));
    }

    // Copy files recursively from src_dir to path
    fn copy_recursive(src: &Path, dst: &Path, force: bool) -> Result<(), KamError> {
        if src.is_dir() {
            fs::create_dir_all(dst)?;
            for entry in fs::read_dir(src)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let src_path = entry.path();
                let dst_path = dst.join(&file_name);
                copy_recursive(&src_path, &dst_path, force)?;
            }
        } else {
            if dst.exists() && !force {
                // skip existing file
            } else {
                if let Some(p) = dst.parent() {
                    fs::create_dir_all(p)?;
                }
                fs::copy(src, dst)?;
            }
        }
        Ok(())
    }

    copy_recursive(&src_dir, path, force)?;

    // After copying, load kam.toml and patch metadata fields
    let mut kt = KamToml::load_from_dir(path)?;
    kt.prop.id = id.to_string();
    let name_btree = name_map.into_iter().collect();
    kt.prop.name = name_btree;
    kt.prop.version = version.to_string();
    kt.prop.versionCode = chrono::Utc::now().timestamp_millis() as u64;
    kt.prop.author = author.to_string();
    let desc_btree = description_map.into_iter().collect();
    kt.prop.description = desc_btree;
    // Mark as module repo type
    kt.kam.module_type = crate::types::modules::base::ModuleType::Repo;
    kt.write_to_dir(path)?;

    Ok(())
}
