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
    ///
    /// NOTE: This method is the classic API that uses environment variables to
    /// build replacements. Use `create_with_replacements` if you want to explicitly
    /// provide replacements rather than relying on environment variables.
    pub fn create(root: &Path, venv_type: VenvType) -> Result<KamVenv, KamError> {
        Self::create_with_replacements(root, venv_type, None)
    }

    /// Create a new virtual environment at `root`, using an explicit replacements map.
    ///
    /// If `replacements_opt` is `Some(map)`, the map will be used as the source of
    /// template replacements. When `None` the environment variables will be used as
    /// before.
    pub fn create_with_replacements(
        root: &Path,
        venv_type: VenvType,
        replacements_opt: Option<std::collections::HashMap<String, String>>,
    ) -> Result<KamVenv, KamError> {
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

        // Prefer the provided replacements map, if present; otherwise fall back to environment variables.
        if let Some(map) = replacements_opt {
            for (k, val) in map.into_iter() {
                replacements.insert(k, val);
            }
        } else {
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
        }

        // if id missing, try current dir name
        if !replacements.contains_key("id") {
            if let Ok(cwd) = std::env::current_dir() {
                if let Some(name) = cwd.file_name().and_then(|s| s.to_str()) {
                    replacements.insert("id".to_string(), name.to_string());
                }
            }
        }

        // Since cache system is removed, we'll use embedded assets directly
        // For now, we'll create a minimal venv structure since we removed the cache system
        // First ensure the template is available
        let template_key =
            std::env::var("KAM_VENV_TEMPLATE").unwrap_or_else(|_| "venv_template".to_string());
        let base = match template_key.as_str() {
            "venv" | "venv_template" => "venv_template",
            other => other,
        };

        // Ensure the template is available
        crate::template::TemplateManager::ensure_template(&base)?;
        
        // Create the bin and lib directories
        fs::create_dir_all(v.bin_dir()).map_err(|e| KamError::Io(e))?;
        fs::create_dir_all(v.lib_dir()).map_err(|e| KamError::Io(e))?;
        
        // Create activation scripts with template replacements
        let activation_script = format!(
            r#"#!/bin/bash
# Kam venv activation script
export KAM_VENV_ACTIVE=1
export KAM_VENV_DIR="{}"
export PATH="$KAM_VENV_DIR/bin:$PATH"
export PS1="(kam-{}) $PS1"

deactivate() {{
    if [ -n "${{KAM_OLD_PATH:-}}" ]; then
        export PATH="$KAM_OLD_PATH"
        unset KAM_OLD_PATH
    fi
    if [ -n "${{KAM_OLD_PS1:-}}" ]; then
        export PS1="$KAM_OLD_PS1"
        unset KAM_OLD_PS1
    fi
    unset KAM_VENV_ACTIVE
    unset KAM_VENV_DIR
    unset -f deactivate 2>/dev/null || true
    echo "Kam virtual environment deactivated."
}}

export KAM_OLD_PATH="$PATH"
export KAM_OLD_PS1="${{PS1:-}}"
"#,
            v.root.display(),
            replacements.get("id").unwrap_or(&"default".to_string())
        );
        
        fs::write(v.root.join("activate"), &activation_script).map_err(|e| KamError::Io(e))?;
        fs::write(v.root.join("activate.sh"), &activation_script).map_err(|e| KamError::Io(e))?;
        
        // Create deactivate script
        let deactivate_script = r#"#!/bin/sh
# Deactivate script for Kam venv

if [ -n "${KAM_OLD_PATH:-}" ]; then
    export PATH="$KAM_OLD_PATH"
    unset KAM_OLD_PATH
fi
if [ -n "${KAM_OLD_PS1:-}" ]; then
    export PS1="$KAM_OLD_PS1"
    unset KAM_OLD_PS1
fi
unset KAM_VENV_ACTIVE
unset KAM_VENV_DIR
echo "Kam virtual environment deactivated."
"#;
        
        fs::write(v.root.join("deactivate"), deactivate_script).map_err(|e| KamError::Io(e))?;
        
        Ok(v)
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

    /// Link a library from a source path to the venv
    pub fn link_library_from_path(
        &self,
        source_path: &Path,
    ) -> Result<(), KamError> {
        let venv_lib = self.lib_dir();

        if !source_path.exists() {
            return Err(KamError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Library path not found: {}", source_path.display()),
            )));
        }

        #[cfg(unix)]
        {
            if venv_lib.exists() {
                fs::remove_dir_all(&venv_lib).map_err(|e| KamError::Io(e))?;
            }
            std::os::unix::fs::symlink(source_path, &venv_lib).map_err(|e| KamError::Io(e))?;
        }
        #[cfg(not(unix))]
        {
            fs::create_dir_all(self.lib_dir()).map_err(|e| KamError::Io(e))?;
            if venv_lib.exists() {
                fs::remove_dir_all(&venv_lib).map_err(|e| KamError::Io(e))?;
            }
            // Try symlink recursively, fallback to copy
            if symlink_dir_all(source_path, &venv_lib).is_err() {
                copy_dir_all(source_path, &venv_lib).map_err(|e| KamError::Io(e))?;
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

// Cross-platform symlink and copy utilities are implemented centrally in `crate::utils`.
// This module relies on `use crate::utils::{symlink_dir_all, copy_dir_all}` and no longer
// provides local Windows-only implementations for those helpers.
