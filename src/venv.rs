use crate::cache::KamCache;
use crate::errors::KamError;
use std::fs;
use std::io::{BufReader, Read};
/// # Kam Virtual Environment System
///
/// Virtual environment support for Kam modules, similar to Python's virtualenv.
///
/// ## Environment Types
///
/// - **Development**: Used during module development with `kam sync --dev`
/// - **Runtime**: Used when modules are installed and running in production
///
/// ## Directory Structure
///
/// ```text
/// .kam_venv/
/// ├── bin/         # Symlinks to cached binaries
/// ├── lib/         # Symlinks to cached libraries
/// ├── activate     # Activation script (Unix)
/// ├── activate.sh  # Activation script (Unix)
/// ├── activate.ps1 # Activation script (PowerShell)
/// ├── activate.bat # Activation script (Windows)
/// └── deactivate   # Deactivation script
/// ```
use std::path::{Path, PathBuf};

/// Virtual environment type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VenvType {
    /// Development environment (includes dev dependencies)
    Development,
    /// Runtime environment (production)
    Runtime,
}

/// Virtual environment for a Kam module
#[derive(Debug)]
pub struct KamVenv {
    /// Path to the virtual environment directory
    root: PathBuf,
    /// Type of environment
    venv_type: VenvType,
}

impl KamVenv {
    /// Create a new virtual environment at `root`.
    ///
    /// If a `.zip` archive named by env `KAM_VENV_TEMPLATE` (default: `venv_template`) is
    /// present in the global cache tmpl dir, it will be extracted and template
    /// placeholders replaced using env vars `KAM_VAR_*` and common keys (id,name,version,author).
    /// Otherwise a small fallback set of activation scripts is generated.
    pub fn create(root: &Path, venv_type: VenvType) -> Result<KamVenv, KamError> {
        if !root.exists() {
            fs::create_dir_all(root).map_err(|e| KamError::Io(e))?;
        }

        let v = KamVenv {
            root: root.to_path_buf(),
            venv_type,
        };

        // mark dev if requested
        if v.venv_type == VenvType::Development {
            let _ = fs::write(v.root.join(".dev"), "");
        }

        // prepare replacements map
        let mut replacements: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        if let Ok(vv) = std::env::var("KAM_ID") {
            replacements.insert("id".to_string(), vv);
        }
        if let Ok(vv) = std::env::var("KAM_NAME") {
            replacements.insert("name".to_string(), vv);
        }
        if let Ok(vv) = std::env::var("KAM_VERSION") {
            replacements.insert("version".to_string(), vv);
        }
        if let Ok(vv) = std::env::var("KAM_AUTHOR") {
            replacements.insert("author".to_string(), vv);
        }
        for (k, v) in std::env::vars() {
            if let Some(rest) = k.strip_prefix("KAM_VAR_") {
                replacements.insert(rest.to_lowercase(), v);
            }
        }

        // if id missing, try current dir name
        if !replacements.contains_key("id") {
            if let Ok(cwd) = std::env::current_dir() {
                if let Some(name) = cwd.file_name().and_then(|s| s.to_str()) {
                    replacements.insert("id".to_string(), name.to_string());
                }
            }
        }

        // Use the global cache for templates
        let cache = KamCache::new()?;
        let tmpl_dir = cache.tmpl_dir();
        let template_key =
            std::env::var("KAM_VENV_TEMPLATE").unwrap_or_else(|_| "venv_template".to_string());
        let base = match template_key.as_str() {
            "venv" | "venv_template" => "venv_template",
            other => other,
        };

        // Ensure the template is available in cache
        crate::template::TemplateManager::ensure_template(&base)?;
        // Try a few forms for the template: tar.gz/tgz/tar, zip, or an unpacked directory
        // tar.gz / tgz / tar support
        let tar_gz_path = tmpl_dir.join(format!("{}.tar.gz", base));
        let tgz_path = tmpl_dir.join(format!("{}.tgz", base));
        let tar_path = tmpl_dir.join(format!("{}.tar", base));
        let chosen_tar = if tar_gz_path.exists() {
            Some((tar_gz_path, true)) // true for gzipped
        } else if tgz_path.exists() {
            Some((tgz_path, true))
        } else if tar_path.exists() {
            Some((tar_path, false)) // false for plain tar
        } else {
            None
        };
        if let Some((tp, is_gzipped)) = chosen_tar {
            let f = std::fs::File::open(&tp).map_err(|e| KamError::Io(e))?;
            let reader: Box<dyn std::io::Read> = if is_gzipped {
                Box::new(flate2::read::GzDecoder::new(BufReader::new(f)))
            } else {
                Box::new(BufReader::new(f))
            };
            let mut archive = tar::Archive::new(reader);
            for entry_res in archive
                .entries()
                .map_err(|e| KamError::FetchFailed(format!("tar entries: {}", e)))?
            {
                let mut entry = entry_res
                    .map_err(|e| KamError::FetchFailed(format!("tar entry read: {}", e)))?;
                let path = match entry.path() {
                    Ok(p) => p.into_owned(),
                    Err(e) => return Err(KamError::FetchFailed(format!("tar entry path: {}", e))),
                };
                let name = path.to_string_lossy().to_string();

                let replace_placeholders = |s: &str| -> String {
                    let mut out = s.to_string();
                    for (k, v) in &replacements {
                        if !v.is_empty() {
                            out = out.replace(&format!("{{{{{}}}}}", k), v);
                        }
                    }
                    out
                };

                let replaced = replace_placeholders(&name);
                let outpath = v.root.join(replaced);
                if entry.header().entry_type().is_dir() {
                    fs::create_dir_all(&outpath).map_err(|e| KamError::Io(e))?;
                } else {
                    if let Some(p) = outpath.parent() {
                        fs::create_dir_all(p).map_err(|e| KamError::Io(e))?;
                    }
                    let mut data: Vec<u8> = Vec::new();
                    entry.read_to_end(&mut data).map_err(|e| KamError::Io(e))?;
                    match String::from_utf8(data) {
                        Ok(s) => {
                            let s2 = replace_placeholders(&s);
                            fs::write(&outpath, s2.as_bytes()).map_err(|e| KamError::Io(e))?;
                        }
                        Err(e) => {
                            let bytes = e.into_bytes();
                            fs::write(&outpath, &bytes).map_err(|e| KamError::Io(e))?;
                        }
                    }
                }
            }
            return Ok(v);
        }

        // zip support
        let zip_path = tmpl_dir.join(format!("{}.zip", base));
        if zip_path.exists() {
            // extract zip
            let file = std::fs::File::open(&zip_path).map_err(|e| KamError::Io(e))?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| KamError::FetchFailed(format!("zip error: {}", e)))?;
            for i in 0..archive.len() {
                let mut entry = archive
                    .by_index(i)
                    .map_err(|e| KamError::FetchFailed(format!("zip entry error: {}", e)))?;
                let name = entry.name().to_string();
                // small helper closure to apply replacements to a string
                let replace_placeholders = |s: &str| -> String {
                    let mut out = s.to_string();
                    for (k, v) in &replacements {
                        if !v.is_empty() {
                            out = out.replace(&format!("{{{{{}}}}}", k), v);
                        }
                    }
                    out
                };

                // apply replacements to the path
                let replaced = replace_placeholders(&name);
                let outpath = v.root.join(replaced);
                if entry.is_dir() {
                    fs::create_dir_all(&outpath).map_err(|e| KamError::Io(e))?;
                } else {
                    if let Some(p) = outpath.parent() {
                        fs::create_dir_all(p).map_err(|e| KamError::Io(e))?;
                    }
                    let mut data: Vec<u8> = Vec::new();
                    entry.read_to_end(&mut data).map_err(|e| KamError::Io(e))?;
                    match String::from_utf8(data) {
                        Ok(s) => {
                            let s2 = replace_placeholders(&s);
                            fs::write(&outpath, s2.as_bytes()).map_err(|e| KamError::Io(e))?;
                        }
                        Err(e) => {
                            let bytes = e.into_bytes();
                            fs::write(&outpath, &bytes).map_err(|e| KamError::Io(e))?;
                        }
                    }
                }
            }
            return Ok(v);
        }

        // finally, accept a pre-unpacked directory named by base
        let dir_path = tmpl_dir.join(base);
        if dir_path.exists() && dir_path.is_dir() {
            // copy directory contents into v.root with placeholder replacement
            // walk entries
            for entry in walkdir::WalkDir::new(&dir_path) {
                let entry =
                    entry.map_err(|e| KamError::FetchFailed(format!("walkdir error: {}", e)))?;
                let rel = entry
                    .path()
                    .strip_prefix(&dir_path)
                    .map_err(|e| KamError::StripPrefixFailed(format!("strip_prefix: {}", e)))?;
                let name = rel.to_string_lossy().to_string();

                let replace_placeholders = |s: &str| -> String {
                    let mut out = s.to_string();
                    for (k, v) in &replacements {
                        if !v.is_empty() {
                            out = out.replace(&format!("{{{{{}}}}}", k), v);
                        }
                    }
                    out
                };

                let replaced = replace_placeholders(&name);
                let outpath = v.root.join(replaced);
                if entry.file_type().is_dir() {
                    fs::create_dir_all(&outpath).map_err(|e| KamError::Io(e))?;
                } else if entry.file_type().is_file() {
                    if let Some(p) = outpath.parent() {
                        fs::create_dir_all(p).map_err(|e| KamError::Io(e))?;
                    }
                    let data = std::fs::read(entry.path()).map_err(|e| KamError::Io(e))?;
                    match String::from_utf8(data) {
                        Ok(s) => {
                            let s2 = replace_placeholders(&s);
                            fs::write(&outpath, s2.as_bytes()).map_err(|e| KamError::Io(e))?;
                        }
                        Err(e) => {
                            let bytes = e.into_bytes();
                            fs::write(&outpath, &bytes).map_err(|e| KamError::Io(e))?;
                        }
                    }
                }
            }
            return Ok(v);
        }

        // Not found: fail rather than generating fallback scripts.
        Err(KamError::TemplateNotFound(format!(
            "venv template '{}' not found in global cache tmpl dir: {}",
            base,
            tmpl_dir.display()
        )))
    }

    /// Load an existing venv (no validation beyond existence)
    pub fn load(root: &Path) -> Result<KamVenv, KamError> {
        if !root.exists() {
            return Err(KamError::VenvNotFound(format!(
                "Virtual environment not found: {}",
                root.display()
            )));
        }
        // try to infer type from .dev marker
        let venv_type = if root.join(".dev").exists() {
            VenvType::Development
        } else {
            VenvType::Runtime
        };
        Ok(KamVenv {
            root: root.to_path_buf(),
            venv_type,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn venv_type(&self) -> VenvType {
        self.venv_type
    }
    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }
    pub fn lib_dir(&self) -> PathBuf {
        self.root.join("lib")
    }

    /// Link a binary from the source path to the venv
    pub fn link_binary(&self, source_path: &Path) -> Result<(), KamError> {
        let name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| KamError::InvalidFilename("invalid binary name".to_string()))?;
        let venv_bin = self.bin_dir().join(name);

        if !source_path.exists() {
            return Err(KamError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Binary not found: {}", source_path.display()),
            )));
        }

        // Create symlink (Unix) or copy (Windows)
        #[cfg(unix)]
        {
            if venv_bin.exists() {
                fs::remove_file(&venv_bin).map_err(|e| KamError::Io(e))?;
            }
            std::os::unix::fs::symlink(source_path, &venv_bin).map_err(|e| KamError::Io(e))?;
        }
        #[cfg(not(unix))]
        {
            fs::create_dir_all(self.bin_dir()).map_err(|e| KamError::Io(e))?;
            if venv_bin.exists() {
                fs::remove_file(&venv_bin).map_err(|e| KamError::Io(e))?;
            }
            // Try symlink first, fallback to copy
            if std::os::windows::fs::symlink_file(source_path, &venv_bin).is_err() {
                fs::copy(source_path, &venv_bin).map_err(|e| KamError::Io(e))?;
            }
        }

        Ok(())
    }

    /// Link a library (module id and version) from cache into the venv
    pub fn link_library(&self, id: &str, version: &str, cache: &KamCache) -> Result<(), KamError> {
        let cache_lib = cache.lib_module_path(id, version).join("lib");
        let venv_lib = self.lib_dir();

        if !cache_lib.exists() {
            return Err(KamError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Library lib/ not found in cache: {} v{}", id, version),
            )));
        }

        #[cfg(unix)]
        {
            if venv_lib.exists() {
                fs::remove_dir_all(&venv_lib).map_err(|e| KamError::Io(e))?;
            }
            std::os::unix::fs::symlink(&cache_lib, &venv_lib).map_err(|e| KamError::Io(e))?;
        }
        #[cfg(not(unix))]
        {
            if venv_lib.exists() {
                fs::remove_dir_all(&venv_lib).map_err(|e| KamError::Io(e))?;
            }
            // Try symlink recursively, fallback to copy
            if symlink_dir_all(&cache_lib, &venv_lib).is_err() {
                copy_dir_all(&cache_lib, &venv_lib).map_err(|e| KamError::Io(e))?;
            }
        }

        Ok(())
    }

    /// Remove the virtual environment
    pub fn remove(self) -> Result<(), KamError> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root).map_err(|e| KamError::Io(e))?;
        }
        Ok(())
    }
}

/// Symlink a directory recursively (for Windows)
#[cfg(not(unix))]
fn symlink_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let ty = entry.file_type()?;
        if ty.is_dir() {
            if std::os::windows::fs::symlink_dir(&src_path, &dst_path).is_err() {
                symlink_dir_all(&src_path, &dst_path)?;
            }
        } else {
            if std::os::windows::fs::symlink_file(&src_path, &dst_path).is_err() {
                fs::copy(&src_path, &dst_path)?;
            }
        }
    }
    Ok(())
}

/// Copy a directory recursively (for Windows)
#[cfg(not(unix))]
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}
