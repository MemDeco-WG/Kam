pub mod base;
pub mod kam;
pub mod template;
pub mod library;
pub mod repo;

// Re-export the common KamModule type for convenience
pub use base::{
    KamToml,
    KamModule,
    ModuleBackend,
    DEFAULT_DEPENDENCY_SOURCE,
    parse_template_vars,
    parse_template_variables,

};
pub use library::LibraryModule;
pub use kam::KamSpecific;
pub use repo::RepoModule;
pub use template::TemplateModule;
