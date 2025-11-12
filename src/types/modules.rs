pub mod base;
pub mod kam;
pub mod library;
pub mod repo;
pub mod template;

// Re-export the common KamModule type for convenience
pub use base::{
    DEFAULT_DEPENDENCY_SOURCE, KamModule, KamToml, ModuleBackend, parse_template_variables,
    parse_template_vars,
};
pub use kam::KamSpecific;
pub use library::LibraryModule;
pub use repo::RepoModule;
pub use template::TemplateModule;
