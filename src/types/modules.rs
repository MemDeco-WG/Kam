pub mod base;
pub mod kam;
pub mod template;
pub mod library;
pub mod repo;

// Re-export the common KamModule type for convenience
pub use base::{KamModule, ModuleBackend};
