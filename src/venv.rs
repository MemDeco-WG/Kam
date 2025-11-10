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
/// 
/// Provides isolated environment similar to Python virtualenv.
#[derive(Debug)]
pub struct KamVenv {
    /// Path to the virtual environment directory
    root: PathBuf,
    /// Type of environment
    venv_type: VenvType,
}

impl KamVenv {
    /// Create a new virtual environment
    /// 
    /// ## Arguments
    /// 
    /// - `root`: Path where the venv should be created
    /// - `venv_type`: Type of environment (Development or Runtime)
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// use kam::venv::{KamVenv, VenvType};
    /// 
    /// let venv = KamVenv::create(".kam-venv", VenvType::Development)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn create<P: AsRef<Path>>(root: P, venv_type: VenvType) -> Result<Self, KamError> {
        let root = root.as_ref().to_path_buf();
        
        if root.exists() {
            return Err(KamError::Other(format!("Virtual environment already exists at {}", root.display())));
        }
        
        // Create directory structure
        fs::create_dir_all(&root)?;
        fs::create_dir_all(root.join("bin"))?;
        fs::create_dir_all(root.join("lib"))?;
        
        let venv = Self { root, venv_type };
        
        // Generate activation scripts
        venv.generate_scripts()?;
        
        Ok(venv)
    }
    
    /// Load an existing virtual environment
    /// 
    /// ## Arguments
    /// 
    /// - `root`: Path to the venv directory
    pub fn load<P: AsRef<Path>>(root: P) -> Result<Self, KamError> {
        let root = root.as_ref().to_path_buf();
        
        if !root.exists() {
            return Err(KamError::Other(format!("Virtual environment not found at {}", root.display())));
        }
        
        // Try to detect venv type from marker file
        let venv_type = if root.join(".dev").exists() {
            VenvType::Development
        } else {
            VenvType::Runtime
        };
        
        Ok(Self { root, venv_type })
    }
    
    /// Get the venv root path
    pub fn root(&self) -> &Path {
        &self.root
    }
    
    /// Get the bin directory
    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }
    
    /// Get the lib directory
    pub fn lib_dir(&self) -> PathBuf {
        self.root.join("lib")
    }
    
    /// Get the venv type
    pub fn venv_type(&self) -> VenvType {
        self.venv_type
    }
    
    /// Generate activation and deactivation scripts
    fn generate_scripts(&self) -> Result<(), KamError> {
        let is_dev = self.venv_type == VenvType::Development;
        
        // Mark as dev if needed
        if is_dev {
            fs::write(self.root.join(".dev"), "")?;
        }
        
        // Unix shell script (activate)
        let unix_activate = format!(
            r#"#!/bin/sh
# Kam Virtual Environment Activation Script
# Type: {}

# Store old PATH
KAM_OLD_PATH="$PATH"
export KAM_OLD_PATH

# Store old prompt
KAM_OLD_PS1="$PS1"
export KAM_OLD_PS1

# Add venv bin to PATH
PATH="{}:$PATH"
export PATH

# Update prompt
PS1="(kam-venv) $PS1"
export PS1

# Set environment marker
KAM_VENV_ACTIVE="1"
export KAM_VENV_ACTIVE

# Define deactivate function
deactivate() {{
    # Restore PATH
    if [ -n "${{KAM_OLD_PATH:-}}" ]; then
        PATH="$KAM_OLD_PATH"
        export PATH
        unset KAM_OLD_PATH
    fi
    
    # Restore prompt
    if [ -n "${{KAM_OLD_PS1:-}}" ]; then
        PS1="$KAM_OLD_PS1"
        export PS1
        unset KAM_OLD_PS1
    fi
    
    # Unset environment marker
    unset KAM_VENV_ACTIVE
    
    # Remove deactivate function
    unset -f deactivate
}}

echo "Kam virtual environment activated ({} mode)"
echo "Run 'deactivate' to exit"
"#,
            if is_dev { "Development" } else { "Runtime" },
            self.bin_dir().display(),
            if is_dev { "development" } else { "runtime" }
        );
        
        // Write Unix scripts
        let activate_path = self.root.join("activate");
    fs::write(&activate_path, &unix_activate)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&activate_path, fs::Permissions::from_mode(0o755))?;
        }
        
    fs::write(self.root.join("activate.sh"), &unix_activate)?;
        
        // PowerShell script
        let ps_activate = format!(
            r#"# Kam Virtual Environment Activation Script (PowerShell)
# Type: {}

# Store old PATH
$env:KAM_OLD_PATH = $env:PATH

# Add venv bin to PATH
$env:PATH = "{};$env:PATH"

# Update prompt
function global:_OLD_KAM_PROMPT {{""}}
$function:_OLD_KAM_PROMPT = $function:prompt
function global:prompt {{
    Write-Host "(kam-venv) " -NoNewline
    & $function:_OLD_KAM_PROMPT
}}

# Set environment marker
$env:KAM_VENV_ACTIVE = "1"

# Define deactivate function
function global:deactivate {{
    # Restore PATH
    if (Test-Path env:KAM_OLD_PATH) {{
        $env:PATH = $env:KAM_OLD_PATH
        Remove-Item env:KAM_OLD_PATH
    }}
    
    # Restore prompt
    if (Test-Path function:_OLD_KAM_PROMPT) {{
        $function:prompt = $function:_OLD_KAM_PROMPT
        Remove-Item function:_OLD_KAM_PROMPT
    }}
    
    # Unset environment marker
    Remove-Item env:KAM_VENV_ACTIVE
    
    # Remove deactivate function
    Remove-Item function:deactivate
}}

Write-Host "Kam virtual environment activated ({} mode)" -ForegroundColor Green
Write-Host "Run 'deactivate' to exit" -ForegroundColor Green
"#,
            if is_dev { "Development" } else { "Runtime" },
            self.bin_dir().display(),
            if is_dev { "development" } else { "runtime" }
        );
        
    fs::write(self.root.join("activate.ps1"), ps_activate)?;
        
        // Windows batch script
        let bat_activate = format!(
            r#"@echo off
REM Kam Virtual Environment Activation Script (Windows)
REM Type: {}

REM Store old PATH
set "KAM_OLD_PATH=%PATH%"

REM Add venv bin to PATH
set "PATH={};%PATH%"

REM Set environment marker
set "KAM_VENV_ACTIVE=1"

REM Update prompt
set "PROMPT=(kam-venv) %PROMPT%"

echo Kam virtual environment activated ({} mode)
echo Run 'deactivate' to exit
"#,
            if is_dev { "Development" } else { "Runtime" },
            self.bin_dir().display(),
            if is_dev { "development" } else { "runtime" }
        );
        
    fs::write(self.root.join("activate.bat"), bat_activate)?;
        
        // Standalone deactivate script (Unix)
        let deactivate_script = r#"#!/bin/sh
# Kam Virtual Environment Deactivation Script

if [ -n "${KAM_VENV_ACTIVE:-}" ]; then
    # Call the deactivate function if it exists
    if type deactivate > /dev/null 2>&1; then
        deactivate
    else
        echo "No active Kam virtual environment found"
    fi
else
    echo "No active Kam virtual environment found"
fi
"#;
        
        let deactivate_path = self.root.join("deactivate");
    fs::write(&deactivate_path, deactivate_script)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&deactivate_path, fs::Permissions::from_mode(0o755))?;
        }
        
        Ok(())
    }
    
    /// Link a binary from the cache to the venv
    /// 
    /// ## Arguments
    /// 
    /// - `name`: Binary name
    /// - `cache`: Reference to KamCache
    pub fn link_binary(&self, name: &str, cache: &KamCache) -> Result<(), KamError> {
        let cache_bin = cache.bin_path(name);
        let venv_bin = self.bin_dir().join(name);
        
        if !cache_bin.exists() {
            return Err(KamError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Binary not found in cache: {}", name),
            )));
        }
        
        // Create symlink (Unix) or copy (Windows)
        #[cfg(unix)]
        {
            if venv_bin.exists() {
                fs::remove_file(&venv_bin)?;
            }
            std::os::unix::fs::symlink(&cache_bin, &venv_bin)?;
        }
        
        #[cfg(not(unix))]
        {
            fs::copy(&cache_bin, &venv_bin)?;
        }
        
        Ok(())
    }
    
    /// Link a library from the cache to the venv
    /// 
    /// ## Arguments
    /// 
    /// - `id`: Module ID
    /// - `version`: Module version
    /// - `cache`: Reference to KamCache
    pub fn link_library(&self, id: &str, version: &str, cache: &KamCache) -> Result<(), KamError> {
        let cache_lib = cache.lib_module_path(id, version);
        let venv_lib = self.lib_dir().join(format!("{}-{}", id, version));
        
        if !cache_lib.exists() {
            return Err(KamError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Library not found in cache: {} v{}", id, version),
            )));
        }
        
        // Create symlink (Unix) or copy directory (Windows)
        #[cfg(unix)]
        {
            if venv_lib.exists() {
                fs::remove_file(&venv_lib)?;
            }
            std::os::unix::fs::symlink(&cache_lib, &venv_lib)?;
        }
        
        #[cfg(not(unix))]
        {
            if venv_lib.exists() {
                fs::remove_dir_all(&venv_lib)?;
            }
            copy_dir_all(&cache_lib, &venv_lib)?;
        }
        
        Ok(())
    }
    
    /// Remove the virtual environment
    pub fn remove(self) -> Result<(), KamError> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    
    #[test]
    fn test_venv_create() {
        let temp_dir = std::env::temp_dir().join("kam_venv_test_1");
        let _ = fs::remove_dir_all(&temp_dir);
        
        let venv = KamVenv::create(&temp_dir, VenvType::Development).unwrap();
        assert!(venv.root().exists());
        assert!(venv.bin_dir().exists());
        assert!(venv.lib_dir().exists());
        assert_eq!(venv.venv_type(), VenvType::Development);
        
        // Check activation scripts exist
        assert!(temp_dir.join("activate").exists());
        assert!(temp_dir.join("activate.sh").exists());
        assert!(temp_dir.join("activate.ps1").exists());
        assert!(temp_dir.join("activate.bat").exists());
        assert!(temp_dir.join("deactivate").exists());
        
        let _ = fs::remove_dir_all(temp_dir);
    }
    
    #[test]
    fn test_venv_load() {
        let temp_dir = std::env::temp_dir().join("kam_venv_test_2");
        let _ = fs::remove_dir_all(&temp_dir);
        
        let _venv = KamVenv::create(&temp_dir, VenvType::Runtime).unwrap();
        let loaded = KamVenv::load(&temp_dir).unwrap();
        
        assert_eq!(loaded.root(), temp_dir.as_path());
        assert_eq!(loaded.venv_type(), VenvType::Runtime);
        
        let _ = fs::remove_dir_all(temp_dir);
    }
    
    #[test]
    fn test_venv_already_exists() {
        let temp_dir = std::env::temp_dir().join("kam_venv_test_3");
        let _ = fs::remove_dir_all(&temp_dir);
        
        let _venv = KamVenv::create(&temp_dir, VenvType::Development).unwrap();
        let result = KamVenv::create(&temp_dir, VenvType::Development);
        
    assert!(result.is_err());
    // After switching to the global `KamError`, we surface existence
    // as `KamError::Other(...)` with a descriptive message.
    assert!(matches!(result.unwrap_err(), KamError::Other(_)));
        
        let _ = fs::remove_dir_all(temp_dir);
    }
    
    #[test]
    fn test_venv_not_found() {
        let temp_dir = std::env::temp_dir().join("kam_venv_test_nonexistent");
        let result = KamVenv::load(&temp_dir);
        
    assert!(result.is_err());
    // Not-found is now represented by `KamError::Other(...)`.
    assert!(matches!(result.unwrap_err(), KamError::Other(_)));
    }
    
    #[test]
    fn test_venv_remove() {
        let temp_dir = std::env::temp_dir().join("kam_venv_test_4");
        let _ = fs::remove_dir_all(&temp_dir);
        
        let venv = KamVenv::create(&temp_dir, VenvType::Development).unwrap();
        assert!(temp_dir.exists());
        
        venv.remove().unwrap();
        assert!(!temp_dir.exists());
    }
    
    #[test]
    fn test_dev_marker() {
        let temp_dir = std::env::temp_dir().join("kam_venv_test_5");
        let _ = fs::remove_dir_all(&temp_dir);
        
        let _venv = KamVenv::create(&temp_dir, VenvType::Development).unwrap();
        assert!(temp_dir.join(".dev").exists());
        
        let _ = fs::remove_dir_all(temp_dir);
        
        let temp_dir2 = std::env::temp_dir().join("kam_venv_test_6");
        let _ = fs::remove_dir_all(&temp_dir2);
        
        let _venv = KamVenv::create(&temp_dir2, VenvType::Runtime).unwrap();
        assert!(!temp_dir2.join(".dev").exists());
        
        let _ = fs::remove_dir_all(temp_dir2);
    }
}
