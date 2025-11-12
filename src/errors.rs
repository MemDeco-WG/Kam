// Central error aggregation module. This file defines the global `KamError`
// and re-exports commonly used error types under `crate::errors::*`.
pub mod cache;
pub mod kam_toml;
pub mod kam;

pub use cache::CacheError;
pub use kam_toml::KamTomlError;
pub use kam_toml::ValidationResult;

pub use kam::KamError;
pub type Result<T> = std::result::Result<T, KamError>;
