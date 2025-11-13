macro_rules! impl_from_module {
    ($struct:ident) => {
        impl $struct {
            pub fn from_module(m: KamModule) -> Self {
                Self { inner: m }
            }
        }
    };
}

pub mod base;
pub mod kam;
pub mod library;
pub mod repo;
pub mod template;

// Re-export the common KamModule type for convenience
pub use base::{
    DEFAULT_DEPENDENCY_SOURCE, KamModule, KamToml, ModuleBackend,
};
pub use kam::KamSpecific;
pub use library::LibraryModule;
pub use repo::RepoModule;
pub use template::TemplateModule;
