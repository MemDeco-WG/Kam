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
/// .kam-venv/
/// ├── bin/         # Symlinks to cached binaries
/// ├── lib/         # Symlinks to cached libraries
/// ├── activate     # Activation script (Unix)
/// ├── activate.sh  # Activation script (Unix)
/// ├── activate.ps1 # Activation script (PowerShell)
/// ├── activate.bat # Activation script (Windows)
/// └── deactivate   # Deactivation script
/// ```

use std::path::{Path, PathBuf};
use std::fs;
use std::io::Read;
use crate::errors::KamError;
use crate::cache::KamCache;

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

        let v = KamVenv { root: root.to_path_buf(), venv_type };

        // mark dev if requested
        if v.venv_type == VenvType::Development {
            let _ = fs::write(v.root.join(".dev"), "");
        }

        // prepare replacements map
        let mut replacements: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        if let Ok(vv) = std::env::var("KAM_ID") { replacements.insert("id".to_string(), vv); }
        if let Ok(vv) = std::env::var("KAM_NAME") { replacements.insert("name".to_string(), vv); }
        if let Ok(vv) = std::env::var("KAM_VERSION") { replacements.insert("version".to_string(), vv); }
        if let Ok(vv) = std::env::var("KAM_AUTHOR") { replacements.insert("author".to_string(), vv); }
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

        // attempt to find a template zip in cache tmpl dir
        if let Ok(cache) = KamCache::new() {
            let tmpl_dir = cache.tmpl_dir();
            let template_key = std::env::var("KAM_VENV_TEMPLATE").unwrap_or_else(|_| "venv_template".to_string());
            let base = match template_key.as_str() {
                "venv" | "venv_template" => "venv_template",
                other => other,
            };
            let zip_path = tmpl_dir.join(format!("{}.zip", base));
            if zip_path.exists() {
                // extract zip
                let file = std::fs::File::open(&zip_path).map_err(|e| KamError::Io(e))?;
                let mut archive = zip::ZipArchive::new(file).map_err(|e| KamError::Other(format!("zip error: {}", e)))?;
                for i in 0..archive.len() {
                    let mut entry = archive.by_index(i).map_err(|e| KamError::Other(format!("zip entry error: {}", e)))?;
                    let name = entry.name().to_string();
                    // apply replacements to the path
                    let mut replaced = name.clone();
                    for (k, v) in &replacements {
                        if !v.is_empty() {
                            replaced = replaced.replace(&format!("{{{{{}}}}}", k), v);
                        }
                    }
                    let outpath = v.root.join(replaced);
                    if entry.is_dir() {
                        fs::create_dir_all(&outpath).map_err(|e| KamError::Io(e))?;
                    } else {
                        if let Some(p) = outpath.parent() {
                            fs::create_dir_all(p).map_err(|e| KamError::Io(e))?;
                        }
                        let mut data: Vec<u8> = Vec::new();
                        entry.read_to_end(&mut data).map_err(|e| KamError::Io(e))?;
                        // attempt text replacement
                        if let Ok(s) = String::from_utf8(data.clone()) {
                            let mut s2 = s;
                            for (k, v) in &replacements {
                                if !v.is_empty() {
                                    s2 = s2.replace(&format!("{{{{{}}}}}", k), v);
                                }
                            }
                            fs::write(&outpath, s2.as_bytes()).map_err(|e| KamError::Io(e))?;
                        } else {
                            fs::write(&outpath, &data).map_err(|e| KamError::Io(e))?;
                        }
                    }
                }
                return Ok(v);
            }
        }

        // fallback: generate simple activation scripts
        v.generate_fallback_scripts()?;

        Ok(v)
    }

    /// Load an existing venv (no validation beyond existence)
    pub fn load(root: &Path) -> Result<KamVenv, KamError> {
        if !root.exists() {
            return Err(KamError::Other(format!("Virtual environment not found: {}", root.display())));
        }
        // try to infer type from .dev marker
        let venv_type = if root.join(".dev").exists() { VenvType::Development } else { VenvType::Runtime };
        Ok(KamVenv { root: root.to_path_buf(), venv_type })
    }

    pub fn root(&self) -> &Path { &self.root }
    pub fn venv_type(&self) -> VenvType { self.venv_type }
    pub fn bin_dir(&self) -> PathBuf { self.root.join("bin") }
    pub fn lib_dir(&self) -> PathBuf { self.root.join("lib") }

    fn generate_fallback_scripts(&self) -> Result<(), KamError> {
        if !self.root.exists() {
            fs::create_dir_all(&self.root).map_err(|e| KamError::Io(e))?;
        }
        let bin_dir = self.bin_dir();
        let lib_dir = self.lib_dir();
        let _ = fs::create_dir_all(&bin_dir);
        let _ = fs::create_dir_all(&lib_dir);

        let unix_activate = format!("#!/bin/sh\n# Kam virtual env activate (fallback)\nPATH=\"{}:$PATH\"\nexport PATH\n", bin_dir.display());
        fs::write(self.root.join("activate"), &unix_activate).map_err(|e| KamError::Io(e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(self.root.join("activate"), fs::Permissions::from_mode(0o755)).map_err(|e| KamError::Io(e))?;
        }
        fs::write(self.root.join("activate.sh"), &unix_activate).map_err(|e| KamError::Io(e))?;

        let ps = format!("$env:PATH = \"{};$env:PATH\"\n$env:KAM_VENV_ACTIVE = '1'\n", bin_dir.display());
        fs::write(self.root.join("activate.ps1"), &ps).map_err(|e| KamError::Io(e))?;

        let bat = format!("set \"PATH={};%PATH%\"\nset \"KAM_VENV_ACTIVE=1\"\n", bin_dir.display());
        fs::write(self.root.join("activate.bat"), &bat).map_err(|e| KamError::Io(e))?;

        let deactivate_script = r#"#!/bin/sh
# Kam Virtual Environment Deactivation Script

if [ -n "${KAM_VENV_ACTIVE:-}" ]; then
    if type deactivate > /dev/null 2>&1; then
        deactivate
    else
        echo "No active Kam virtual environment found"
    fi
fi
"#;
        fs::write(self.root.join("deactivate"), deactivate_script).map_err(|e| KamError::Io(e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(self.root.join("deactivate"), fs::Permissions::from_mode(0o755)).map_err(|e| KamError::Io(e))?;
        }

        Ok(())
    }

    /// Link a binary from the cache to the venv
    pub fn link_binary(&self, name: &str, cache: &KamCache) -> Result<(), KamError> {
        let cache_bin = cache.bin_path(name);
        let venv_bin = self.bin_dir().join(name);

        if !cache_bin.exists() {
            return Err(KamError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, format!("Binary not found in cache: {}", name))));
        }

        // Create symlink (Unix) or copy (Windows)
        #[cfg(unix)]
        {
            if venv_bin.exists() { fs::remove_file(&venv_bin).map_err(|e| KamError::Io(e))?; }
            std::os::unix::fs::symlink(&cache_bin, &venv_bin).map_err(|e| KamError::Io(e))?;
        }
        #[cfg(not(unix))]
        {
            fs::create_dir_all(self.bin_dir()).map_err(|e| KamError::Io(e))?;
            fs::copy(&cache_bin, &venv_bin).map_err(|e| KamError::Io(e))?;
        }

        Ok(())
    }

    /// Link a library (module id and version) from cache into the venv
    pub fn link_library(&self, id: &str, version: &str, cache: &KamCache) -> Result<(), KamError> {
        let cache_lib = cache.lib_module_path(id, version);
        let venv_lib = self.lib_dir().join(format!("{}-{}", id, version));

        if !cache_lib.exists() {
            return Err(KamError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, format!("Library not found in cache: {} v{}", id, version))));
        }

        #[cfg(unix)]
        {
            if venv_lib.exists() { fs::remove_file(&venv_lib).map_err(|e| KamError::Io(e))?; }
            std::os::unix::fs::symlink(&cache_lib, &venv_lib).map_err(|e| KamError::Io(e))?;
        }
        #[cfg(not(unix))]
        {
            if venv_lib.exists() { fs::remove_dir_all(&venv_lib).map_err(|e| KamError::Io(e))?; }
            copy_dir_all(&cache_lib, &venv_lib).map_err(|e| KamError::Io(e))?;
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


