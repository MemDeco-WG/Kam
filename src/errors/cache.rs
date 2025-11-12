use std::io;
use thiserror::Error;

/// Errors that can occur when working with the cache
#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Failed to determine cache directory")]
    CacheDirNotFound,

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

impl CacheError {
    // keep helper if any needed later
    #[allow(dead_code)]
    pub fn as_str(&self) -> String {
        format!("{}", self)
    }
}
