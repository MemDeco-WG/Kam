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
/// ├── profile/  # ksu profile archives
/// └── tmpl/     # built-in templates extracted from assets/tmpl
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
use rust_embed::RustEmbed;
use std::io::Write;
use zip::ZipArchive;

#[derive(RustEmbed)]
#[folder = "src/assets/tmpl"]
struct TmplAssets;

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
    /// use kam::cache::KamCache;
    /// let cache = KamCache::new().unwrap();
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
    /// use kam::cache::KamCache;
    /// let cache = KamCache::with_root(std::env::temp_dir().join("kam_cache_custom")).unwrap();
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
        // Honor explicit override via KAM_CACHE_ROOT environment variable.
        // If provided, use it (relative paths are resolved against CWD).
        if let Some(v) = std::env::var_os("KAM_CACHE_ROOT") {
            let p = PathBuf::from(v);
            let abs = if p.is_absolute() { p } else { std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(p) };
            return Ok(abs);
        }

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
    /// use kam::cache::KamCache;
    /// let cache = KamCache::new().unwrap();
    /// cache.ensure_dirs().unwrap();
    /// ```
    pub fn ensure_dirs(&self) -> Result<(), CacheError> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.bin_dir())?;
        std::fs::create_dir_all(self.lib_dir())?;
        std::fs::create_dir_all(self.log_dir())?;
        std::fs::create_dir_all(self.profile_dir())?;
        // Ensure builtin templates are available in the cache tmpl directory.
        // This extracts embedded template archives (from src/assets/tmpl)
        // into `${cache_root}/tmpl/<template_name>/...` and writes the
        // archive as `${cache_root}/tmpl/<template_name>.zip` so we don't
        // re-extract on subsequent runs.
        self.ensure_builtin_templates()?;
        Ok(())
    }

    /// Get the tmpl directory (where built-in templates will be extracted)
    pub fn tmpl_dir(&self) -> PathBuf {
        self.root.join("tmpl")
    }


    /// Ensure built-in template archives from `src/assets/tmpl` are present
    /// in the cache and extracted. Idempotent: skips archives already
    /// written and directories already extracted.
    fn ensure_builtin_templates(&self) -> Result<(), CacheError> {
        std::fs::create_dir_all(self.tmpl_dir())?;

        for entry in TmplAssets::iter() {
            let name = entry.as_ref();
            if !name.ends_with(".zip") {
                continue;
            }

            // Use the base name without .zip as the extraction folder
            let base = match name.strip_suffix(".zip") {
                Some(b) => b,
                None => continue,
            };

            let zip_path = self.tmpl_dir().join(format!("{}.zip", base));
            let extract_dir = self.tmpl_dir().join(base);

            // If both archive and extracted dir exist, skip
            if zip_path.exists() && extract_dir.exists() {
                continue;
            }

            if let Some(content) = TmplAssets::get(name) {
                // Write archive file if missing
                if !zip_path.exists() {
                    let mut f = std::fs::File::create(&zip_path)?;
                    f.write_all(&content.data)?;
                }

                // Extract archive into extract_dir
                let file = std::fs::File::open(&zip_path)?;
                // ZipArchive requires Read + Seek
                let mut archive = ZipArchive::new(file).map_err(|e| {
                    CacheError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
                })?;

                for i in 0..archive.len() {
                    let mut file = archive.by_index(i).map_err(|e| {
                        CacheError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
                    })?;
                    let outpath = extract_dir.join(file.name());
                    if file.name().ends_with('/') {
                        std::fs::create_dir_all(&outpath)?;
                    } else {
                        if let Some(p) = outpath.parent() {
                            std::fs::create_dir_all(p)?;
                        }
                        let mut outfile = std::fs::File::create(&outpath)?;
                        std::io::copy(&mut file, &mut outfile)?;
                    }
                }
            }
        }

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
    /// use kam::cache::KamCache;
    /// let cache = KamCache::new().unwrap();
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
    /// use kam::cache::KamCache;
    /// let cache = KamCache::new().unwrap();
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
    /// use kam::cache::KamCache;
    /// let cache = KamCache::new().unwrap();
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
    /// use kam::cache::KamCache;
    /// let cache = KamCache::new().unwrap();
    /// cache.clear_all().unwrap();
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
    /// use kam::cache::KamCache;
    /// let cache = KamCache::new().unwrap();
    /// cache.clear_dir("log").unwrap();
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
    /// use kam::cache::KamCache;
    /// let cache = KamCache::new().unwrap();
    /// let stats = cache.stats().unwrap();
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
    /// ```rust,no_run
    /// use kam::cache::KamCache;
    /// let cache = KamCache::new().unwrap();
    /// let stats = cache.stats().unwrap();
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

