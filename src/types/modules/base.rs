use std::path::{Path, PathBuf};
// use git2 for repository operations instead of shelling out to `git`
use git2::{Cred, RemoteCallbacks, FetchOptions, build::RepoBuilder, CredentialType};
use tempfile::tempdir;
use std::fs;
use std::io::{self};
use std::collections::HashMap;

use crate::errors::{KamError, Result};
use crate::cache::KamCache;
use crate::types::source::Source;
pub use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::sections::VariableDefinition;


pub const DEFAULT_DEPENDENCY_SOURCE: &str = "https://github.com/MemDeco-WG/Kam-Index";

/// A lightweight abstraction of a Kam module. Owns a KamToml and an optional Source.
#[derive(Debug, Clone)]
pub struct KamModule {
    pub toml: KamToml,
    pub source: Option<Source>,
}

/// Trait for module backends that can fetch and install module sources.
pub trait ModuleBackend {
    fn canonical_cache_name(&self) -> Option<String>;
    fn fetch_to_temp(&self) -> Result<PathBuf>;
    fn install_into_cache(&self, cache: &KamCache) -> Result<PathBuf>;
}

/// ModuleBackend contract and semantics
///
/// Implementers of this trait provide three responsibilities:
/// - `canonical_cache_name` (optional): return a stable name to install the
///   module under in the cache (typically `id-version`). When `None` the
///   caller will derive a name from the source.
/// - `fetch_to_temp`: fetch the module source and return a filesystem path
///   containing the unpacked source. The returned path points to a persisted
///   directory that the caller may inspect. Ownership/cleanup: implementers
///   are allowed to persist the fetched data (for example by using a
///   temporary directory that is "kept"). Callers that do not need the
///   intermediate copy should remove it when finished. In short, the caller
///   is responsible for deleting the returned path if it should not be kept.
/// - `install_into_cache`: move or copy the fetched contents into the
///   provided `KamCache` and return the destination path inside the cache.
///
/// Concurrency / atomicity: this trait does not prescribe locking semantics.
/// The default `KamModule` implementation will overwrite an existing
/// destination (remove + copy). If callers require concurrent-safe installs
/// they should implement higher-level locking (for example file locks or
/// a per-cache mutex) around calls to `install_into_cache`.
///
/// Note: the trait is intentionally small so callers can mock or provide
/// alternate backends (HTTP, Git, local archives, etc.).

impl KamModule {
    /// Create from an owned KamToml and optional Source.
    pub fn new(toml: KamToml, source: Option<Source>) -> Self {
        Self { toml, source }
    }

    /// Parse a source spec string and attach it to the KamModule constructed from KamToml.
    pub fn from_spec_and_toml(spec: &str, toml: KamToml) -> Result<Self> {
        let src = Source::parse(spec).map_err(|e| KamError::ParseSourceFailed(format!("parse source spec: {}", e)))?;
        Ok(Self::new(toml, Some(src)))
    }

    /// Return a canonical name for installing into cache: id-version when available.
    pub fn canonical_cache_name(&self) -> Option<String> {
        let id = &self.toml.prop.id;
        let ver = &self.toml.prop.version;
        if !id.is_empty() && !ver.is_empty() {
            Some(format!("{}-{}", id, ver))
        } else {
            None
        }
    }

    /// Fetch the module source into a temporary directory and return the path.
    ///
    /// This is a synchronous/blocking helper. It does not permanently install into the cache.
    pub fn fetch_to_temp(&self) -> Result<PathBuf> {
        let src = match &self.source {
            Some(s) => s.clone(),
            None => return Err(KamError::ParseSourceFailed("no source specified for module".to_string())),
        };

        match src {
            Source::Local { path } => {
                let p = fs::canonicalize(&path).map_err(|e| KamError::Io(e))?;
                if p.is_file() {
                    let tmp = tempdir()?;
                    extract_archive(&p, tmp.path())?;
                    let kept = tmp.keep();
                    Ok(kept)
                } else {
                    let tmp = tempdir()?;
                    let dst = tmp.path().join("src");
                    fs::create_dir_all(&dst)?;
                    copy_dir_all(&p, &dst)?;
                    let kept = tmp.keep();
                    Ok(kept)
                }
            }
            Source::Url { url } => {
                let tmp = tempdir()?;
                let resp = reqwest::blocking::get(&url).map_err(|e| KamError::FetchFailed(format!("failed to download {}: {}", url, e)))?;
                if !resp.status().is_success() {
                    return Err(KamError::FetchFailed(format!("download failed: {} -> {}", url, resp.status())));
                }

                let mut data = Vec::new();
                let mut reader = resp;
                reader.copy_to(&mut data).map_err(|e| KamError::FetchFailed(format!("read download body: {}", e)))?;

                    if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
                        let file = tmp.path().join("download.tar.gz");
                        fs::write(&file, &data)?;
                        extract_tar_gz(&file, tmp.path())?;
                        let kept = tmp.keep();
                        return Ok(kept);
                    } else if url.ends_with(".zip") {
                        let file = tmp.path().join("download.zip");
                        fs::write(&file, &data)?;
                        extract_zip(&file, tmp.path())?;
                        let kept = tmp.keep();
                        return Ok(kept);
                    } else {
                        let file = tmp.path().join("download.bin");
                        fs::write(&file, &data)?;
                        let kept = tmp.keep();
                        return Ok(kept);
                    }
            }
            Source::Git { url, rev } => {
                let tmp = tempdir()?;

                // Prepare credential callbacks: try SSH agent first, then optional
                // SSH key path (KAM_GIT_SSH_KEY_PATH), token (KAM_GIT_TOKEN), or
                // username/password (KAM_GIT_USERNAME / KAM_GIT_PASSWORD).
                let mut callbacks = RemoteCallbacks::new();
                callbacks.credentials(move |_, username_from_url, allowed| {
                    // 1) SSH agent
                    if allowed.contains(CredentialType::SSH_KEY) {
                        if let Some(user) = username_from_url {
                            if let Ok(c) = Cred::ssh_key_from_agent(user) {
                                return Ok(c);
                            }
                        }
                        if let Ok(c) = Cred::ssh_key_from_agent("git") {
                            return Ok(c);
                        }
                    }

                    // 2) SSH key file provided via env
                    if allowed.contains(CredentialType::SSH_KEY) {
                        if let Ok(key_path) = std::env::var("KAM_GIT_SSH_KEY_PATH") {
                            let user = username_from_url.unwrap_or("git");
                            // try public key path as key_path + ".pub"
                            let pubkey_buf = std::path::PathBuf::from(format!("{}.pub", key_path));
                            let privkey_buf = std::path::PathBuf::from(&key_path);
                            let pubkey = pubkey_buf.as_path();
                            let privkey = privkey_buf.as_path();
                            if privkey.exists() {
                                // ignore potential errors and try
                                if let Ok(c) = Cred::ssh_key(user, Some(pubkey), privkey, None) {
                                    return Ok(c);
                                }
                            }
                        }
                    }

                    // 3) Token via env (use as basic auth password)
                    if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
                        if let Ok(token) = std::env::var("KAM_GIT_TOKEN") {
                            // Some providers accept username 'x-access-token' or 'git'
                            return Cred::userpass_plaintext("x-access-token", &token);
                        }
                        if let (Ok(user), Ok(pass)) = (std::env::var("KAM_GIT_USERNAME"), std::env::var("KAM_GIT_PASSWORD")) {
                            return Cred::userpass_plaintext(&user, &pass);
                        }
                    }

                    // Fallback
                    Cred::default()
                });

                let mut fo = FetchOptions::new();
                fo.remote_callbacks(callbacks);
                // request a shallow clone (depth 1) for remote transports.
                // Some local transports (file://) don't support shallow fetches,
                // so only set depth for non-file URLs.
                if !url.starts_with("file://") {
                    fo.depth(1);
                }

                let mut builder = RepoBuilder::new();
                builder.fetch_options(fo);

                let repo = builder.clone(&url, tmp.path()).map_err(|e| KamError::FetchFailed(format!("git clone {}: {}", url, e)))?;

                if let Some(r) = rev {
                    let obj = repo.revparse_single(&r).map_err(|e| KamError::FetchFailed(format!("resolve rev {}: {}", r, e)))?;
                    repo.checkout_tree(&obj, None).map_err(|e| KamError::FetchFailed(format!("checkout tree: {}", e)))?;
                    repo.set_head_detached(obj.id()).map_err(|e| KamError::FetchFailed(format!("set HEAD: {}", e)))?;
                }

                let kept = tmp.keep();
                Ok(kept)
            }
        }
    }

    /// Install (move) the fetched source into the cache under a canonical name if available.
    /// Returns the destination path in the cache.
    pub fn install_into_cache(&self, cache: &KamCache) -> Result<PathBuf> {
        let src_path = self.fetch_to_temp()?;

        // Determine destination name
        let dest_name = if let Some(name) = self.canonical_cache_name() {
            name
        } else {
            match &self.source {
                Some(Source::Git { url, .. }) => sanitize_name(url),
                Some(Source::Url { url }) => sanitize_name(url),
                Some(Source::Local { path }) => sanitize_name(&path.to_string_lossy()),
                    None => return Err(KamError::ParseSourceFailed("no source available to derive name".to_string())),
            }
        };

        let dest = cache.lib_dir().join(dest_name);

        // Remove any existing destination to ensure a clean install
        if dest.exists() {
            fs::remove_dir_all(&dest)?;
        }

        // Try to perform a cheap/atomic move (rename) to avoid copy when
        // possible ("zero-copy" case). This will succeed when the source
        // and destination are on the same filesystem. If rename fails we
        // fall back to copying the contents.
        //
        // Handle the common case where `src_path` contains a single child
        // directory that actually holds the module root — in that case try
        // to rename that child into place first.
        let entries: Vec<_> = fs::read_dir(&src_path)?.collect();
        if entries.len() == 1 {
            let only = entries[0].as_ref().unwrap().path();
            if only.is_dir() {
                // attempt rename of the single-child dir
                if let Err(_e) = fs::rename(&only, &dest) {
                    // rename failed (likely cross-device) -> copy fallback
                    copy_dir_all(&only, &dest)?;
                    // attempt to remove the original temporary tree
                    let _ = fs::remove_dir_all(&src_path);
                }
                return Ok(dest);
            }
        }

        // Otherwise attempt to rename the fetched root dir directly
        if let Err(_e) = fs::rename(&src_path, &dest) {
            // rename failed (e.g. cross-device); create dest and copy
            fs::create_dir_all(&dest)?;
            copy_dir_all(&src_path, &dest)?;
            // try to remove the temporary source tree; ignore errors
            let _ = fs::remove_dir_all(&src_path);
        }

        Ok(dest)
    }
}

// Implement the ModuleBackend trait for KamModule so callers can use the
// abstraction explicitly.
impl ModuleBackend for KamModule {
    fn canonical_cache_name(&self) -> Option<String> { self.canonical_cache_name() }
    fn fetch_to_temp(&self) -> Result<PathBuf> { self.fetch_to_temp() }
    fn install_into_cache(&self, cache: &KamCache) -> Result<PathBuf> { self.install_into_cache(cache) }
}

fn sanitize_name(s: &str) -> String {
    let mut out = s.replace("https://", "").replace("http://", "");
    out = out.replace(['/', ':', '@'], "-");
    if out.ends_with(".git") {
        out.truncate(out.len() - 4);
    }
    out
}

// Small helpers (no external utils module required)
fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else if file_type.is_file() {
            fs::copy(&entry.path(), &dest_path)?;
        } else if file_type.is_symlink() {
            // attempt to copy symlink target as file/dir depending on target
            let target = fs::read_link(entry.path())?;
            if target.is_dir() {
                copy_dir_all(&target, &dest_path)?;
            } else {
                fs::copy(&target, &dest_path)?;
            }
        }
    }
    Ok(())
}

fn extract_zip(zip_path: &Path, dst: &Path) -> Result<()> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i)?;
        let outpath = dst.join(f.name());
        if f.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut f, &mut outfile)?;
        }
    }
    Ok(())
}

fn extract_tar_gz(tar_path: &Path, dst: &Path) -> Result<()> {
    let f = fs::File::open(tar_path)?;
    let decompressor = flate2::read::GzDecoder::new(f);
    let mut archive = tar::Archive::new(decompressor);
    archive.unpack(dst)?;
    Ok(())
}

fn extract_archive(path: &Path, dst: &Path) -> Result<()> {
    let s = path.to_string_lossy().to_lowercase();
    if s.ends_with(".zip") {
        extract_zip(path, dst)?;
    } else if s.ends_with(".tar.gz") || s.ends_with(".tgz") {
        extract_tar_gz(path, dst)?;
    } else {
        return Err(KamError::UnsupportedArchive(format!("unsupported archive format: {}", path.display())));
    }
    Ok(())
}

/// Parse template variables from CLI arguments
pub fn parse_template_vars(vars: &[String]) -> Result<HashMap<String, String>> {
    let mut template_vars = HashMap::new();
    for var in vars {
        if let Some((key, value)) = var.split_once('=') {
            template_vars.insert(key.to_string(), value.to_string());
        } else {
            return Err(KamError::InvalidVarFormat(format!("Invalid template variable format: {}", var)));
        }
    }
    Ok(template_vars)
}

/// Parse template variable definitions from CLI arguments
pub fn parse_template_variables(vars: &[String]) -> Result<HashMap<String, VariableDefinition>> {
    let mut variables = HashMap::new();
    for var in vars {
        if let Some((key, value)) = var.split_once('=') {
            // Accept an optional fourth field as a human-friendly note/message.
            // Format: type:required:default[:note]
            let mut parts_iter = value.splitn(4, ':');
            let var_type = parts_iter.next().unwrap_or("").to_string();
            let required = parts_iter.next().unwrap_or("") == "true";
            let default_part = parts_iter.next().unwrap_or("");
            let default = if default_part.is_empty() { None } else { Some(default_part.to_string()) };
            let note = parts_iter.next().map(|s| s.to_string());
            variables.insert(key.to_string(), VariableDefinition {
                var_type,
                required,
                default,
                note,
                help: None,
                example: None,
                choices: None,
            });
        } else {
            return Err(KamError::InvalidVarFormat(format!("Invalid template variable format: {}. Expected key=type:required:default", var)));
        }
    }
    Ok(variables)
}
