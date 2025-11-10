/// # Kam Cache System
/// 
/// Global cache mechanism for Kam modules, inspired by uv-cache.
/// 
/// ## Cache Structure
/// 
/// ```text
/// ~/.kam/ (or /data/adb/kam on Android)
/// ├── bin/      # Executable binary files (provided by library modules)
/// ├── lib/      # Library modules (extracted dependencies, not compressed)
/// ├── log/      # Log files
/// └── profile/  # Template module archives (compressed)
/// ```
/// 
/// ## Example Usage
/// 
/// ```rust,no_run
/// use kam::cache::KamCache;
/// 
/// let cache = KamCache::new()?;
/// cache.ensure_dirs()?;
/// 
/// // Get paths to cache subdirectories
/// let lib_path = cache.lib_dir();
/// let bin_path = cache.bin_dir();
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```

use std::path::{Path, PathBuf};
use crate::errors::cache::CacheError;

// CacheError is defined in `src/errors/cache.rs` and re-exported here for
// backwards compatibility as `crate::cache::CacheError`.

/// Global cache for Kam modules
/// 
/// ## Platform-specific locations
/// 
/// - **Non-Android (Linux, macOS, etc.)**: `~/.kam/`
/// - **Android**: `/data/adb/kam`
pub struct KamCache {
    /// Root directory of the cache
    root: PathBuf,
}

impl KamCache {
    /// Create a new KamCache instance with the default cache directory
    /// 
    /// ## Platform Detection
    /// 
    /// Automatically detects Android by checking for `/data/adb/` directory.
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// let cache = KamCache::new()?;
    /// println!("Cache root: {}", cache.root().display());
    /// ```
    pub fn new() -> Result<Self, CacheError> {
        let root = Self::default_cache_dir()?;
        Ok(Self { root })
    }
    
    /// Create a KamCache with a custom root directory
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// let cache = KamCache::with_root("/custom/cache/path")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_root<P: AsRef<Path>>(root: P) -> Result<Self, CacheError> {
        let root = root.as_ref().to_path_buf();
        if !root.is_absolute() {
            return Err(CacheError::InvalidPath(
                format!("Cache root must be absolute: {}", root.display())
            ));
        }
        Ok(Self { root })
    }
    
    /// Get the default cache directory based on platform
    /// 
    /// Returns:
    /// - `/data/adb/kam` on Android
    /// - `~/.kam` on other platforms
    fn default_cache_dir() -> Result<PathBuf, CacheError> {
        // Check if we're on Android
        if Path::new("/data/adb").exists() {
            return Ok(PathBuf::from("/data/adb/kam"));
        }
        
        // For other platforms, use ~/.kam
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| CacheError::CacheDirNotFound)?;
        
        Ok(PathBuf::from(home).join(".kam"))
    }
    
    /// Get the cache root directory
    pub fn root(&self) -> &Path {
        &self.root
    }
    
    /// Get the bin directory (executable binary files)
    /// 
    /// Binary files provided by library modules are stored here.
    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }
    
    /// Get the lib directory (library modules)
    /// 
    /// Library modules are extracted here (not compressed).
    /// Dependencies are organized by module ID and version.
    pub fn lib_dir(&self) -> PathBuf {
        self.root.join("lib")
    }
    
    /// Get the log directory
    pub fn log_dir(&self) -> PathBuf {
        self.root.join("log")
    }
    
    /// Get the profile directory (template module archives)
    /// 
    /// Template modules are stored as compressed archives.
    pub fn profile_dir(&self) -> PathBuf {
        self.root.join("profile")
    }
    
    /// Ensure all cache directories exist
    /// 
    /// Creates the cache root and all subdirectories if they don't exist.
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// let cache = KamCache::new()?;
    /// cache.ensure_dirs()?;
    /// ```
    pub fn ensure_dirs(&self) -> Result<(), CacheError> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.bin_dir())?;
        std::fs::create_dir_all(self.lib_dir())?;
        std::fs::create_dir_all(self.log_dir())?;
        std::fs::create_dir_all(self.profile_dir())?;
        Ok(())
    }
    
    /// Get the path to a library module in the cache
    /// 
    /// ## Arguments
    /// 
    /// - `id`: Module ID
    /// - `version`: Module version
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// let module_path = cache.lib_module_path("core-lib", "1.0.0");
/// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn lib_module_path(&self, id: &str, version: &str) -> PathBuf {
        self.lib_dir().join(format!("{}-{}", id, version))
    }
    
    /// Get the path to a binary in the cache
    /// 
    /// ## Arguments
    /// 
    /// - `name`: Binary name
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// let bin_path = cache.bin_path("mytool");
    /// ```
    pub fn bin_path(&self, name: &str) -> PathBuf {
        self.bin_dir().join(name)
    }
    
    /// Get the path to a template archive in the cache
    /// 
    /// ## Arguments
    /// 
    /// - `id`: Template ID
    /// - `version`: Template version
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// let template_path = cache.profile_path("my-template", "1.0.0");
    /// ```
    pub fn profile_path(&self, id: &str, version: &str) -> PathBuf {
        self.profile_dir().join(format!("{}-{}.zip", id, version))
    }
    
    /// Clear the entire cache
    /// 
    /// **Warning**: This removes all cached modules, binaries, and logs.
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// cache.clear_all()?;
    /// ```
    pub fn clear_all(&self) -> Result<(), CacheError> {
        if self.root.exists() {
            std::fs::remove_dir_all(&self.root)?;
        }
        Ok(())
    }
    
    /// Clear a specific cache directory
    /// 
    /// ## Arguments
    /// 
    /// - `dir`: Directory type ("bin", "lib", "log", or "profile")
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// cache.clear_dir("log")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn clear_dir(&self, dir: &str) -> Result<(), CacheError> {
        let path = match dir {
            "bin" => self.bin_dir(),
            "lib" => self.lib_dir(),
            "log" => self.log_dir(),
            "profile" => self.profile_dir(),
            _ => return Err(CacheError::InvalidPath(
                format!("Unknown cache directory: {}", dir)
            )),
        };
        
        if path.exists() {
            std::fs::remove_dir_all(&path)?;
            std::fs::create_dir_all(&path)?;
        }
        
        Ok(())
    }
    
    /// Get cache statistics
    /// 
    /// Returns the total size and number of files in the cache.
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// let stats = cache.stats()?;
    /// println!("Cache size: {} bytes, {} files", stats.total_size, stats.file_count);
/// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn stats(&self) -> Result<CacheStats, CacheError> {
        let mut stats = CacheStats::default();
        
        if self.root.exists() {
            Self::compute_dir_stats(&self.root, &mut stats)?;
        }
        
        Ok(stats)
    }
    
    /// Recursively compute directory statistics
    fn compute_dir_stats(path: &Path, stats: &mut CacheStats) -> Result<(), CacheError> {
        if !path.exists() {
            return Ok(());
        }
        
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            
            if metadata.is_file() {
                stats.file_count += 1;
                stats.total_size += metadata.len();
            } else if metadata.is_dir() {
                Self::compute_dir_stats(&entry.path(), stats)?;
            }
        }
        
        Ok(())
    }
}

/// Cache statistics
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    /// Total size in bytes
    pub total_size: u64,
    /// Number of files
    pub file_count: usize,
}

impl CacheStats {
    /// Format the size as a human-readable string
    /// 
    /// ## Example
    /// 
    /// ```ignore
    /// let stats = cache.stats()?;
    /// println!("Cache size: {}", stats.format_size());
    /// ```
    pub fn format_size(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.total_size as f64;
        let mut unit_idx = 0;
        
        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }
        
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    
    #[test]
    fn test_cache_creation() {
        let temp_dir = std::env::temp_dir().join("kam_cache_test_1");
        let cache = KamCache::with_root(&temp_dir).unwrap();
        assert_eq!(cache.root(), temp_dir);
        let _ = fs::remove_dir_all(temp_dir);
    }
    
    #[test]
    fn test_ensure_dirs() {
        let temp_dir = std::env::temp_dir().join("kam_cache_test_2");
        let cache = KamCache::with_root(&temp_dir).unwrap();
        cache.ensure_dirs().unwrap();
        
        assert!(cache.bin_dir().exists());
        assert!(cache.lib_dir().exists());
        assert!(cache.log_dir().exists());
        assert!(cache.profile_dir().exists());
        
        let _ = fs::remove_dir_all(temp_dir);
    }
    
    #[test]
    fn test_paths() {
        let temp_dir = std::env::temp_dir().join("kam_cache_test_3");
        let cache = KamCache::with_root(&temp_dir).unwrap();
        
        assert_eq!(
            cache.lib_module_path("test-mod", "1.0.0"),
            temp_dir.join("lib").join("test-mod-1.0.0")
        );
        
        assert_eq!(
            cache.bin_path("mytool"),
            temp_dir.join("bin").join("mytool")
        );
        
        assert_eq!(
            cache.profile_path("my-template", "2.0.0"),
            temp_dir.join("profile").join("my-template-2.0.0.zip")
        );
        
        let _ = fs::remove_dir_all(temp_dir);
    }
    
    #[test]
    fn test_clear_dir() {
        let temp_dir = std::env::temp_dir().join("kam_cache_test_4");
        let cache = KamCache::with_root(&temp_dir).unwrap();
        cache.ensure_dirs().unwrap();
        
        // Create a test file
        let test_file = cache.log_dir().join("test.log");
        fs::write(&test_file, "test").unwrap();
        assert!(test_file.exists());
        
        // Clear log directory
        cache.clear_dir("log").unwrap();
        assert!(!test_file.exists());
        assert!(cache.log_dir().exists());
        
        let _ = fs::remove_dir_all(temp_dir);
    }
    
    #[test]
    fn test_stats() {
        let temp_dir = std::env::temp_dir().join("kam_cache_test_5");
        let cache = KamCache::with_root(&temp_dir).unwrap();
        cache.ensure_dirs().unwrap();
        
        // Create test files
        fs::write(cache.log_dir().join("test1.log"), "hello").unwrap();
        fs::write(cache.log_dir().join("test2.log"), "world").unwrap();
        
        let stats = cache.stats().unwrap();
        assert_eq!(stats.file_count, 2);
        assert_eq!(stats.total_size, 10); // "hello" + "world"
        
        let formatted = stats.format_size();
        assert!(formatted.contains("B"));
        
        let _ = fs::remove_dir_all(temp_dir);
    }
    
    #[test]
    fn test_format_size() {
        let mut stats = CacheStats::default();
        
        stats.total_size = 500;
        assert_eq!(stats.format_size(), "500.00 B");
        
        stats.total_size = 1536; // 1.5 KB
        assert!(stats.format_size().starts_with("1.5"));
        assert!(stats.format_size().contains("KB"));
        
        stats.total_size = 1048576; // 1 MB
        assert!(stats.format_size().starts_with("1.0"));
        assert!(stats.format_size().contains("MB"));
    }
    
    #[test]
    fn test_invalid_root() {
        let result = KamCache::with_root("relative/path");
        assert!(result.is_err());
    }
}
