// Declare submodules
pub mod build;
pub mod dependency;
pub mod kam;
pub mod kamlib;
pub mod manager;
pub mod mmrl;
pub mod note;
pub mod options;
pub mod prop;
pub mod repo;
pub mod tmpl;
pub mod tool;

// Re-export main types
pub use crate::types::kam_toml::enums::{ModuleType, SupportedArch};
pub use build::BuildSection;
pub use dependency::{Dependency, DependencySection, VersionSpec, FlatDependencyGroup, FlatDependencyGroups};
pub use kam::KamSection;
pub use kamlib::LibSection;
pub use manager::ManagerSection;
pub use mmrl::MmrlSection;
pub use note::NoteSection;
pub use options::OptionsSection;
pub use prop::PropSection;
pub use repo::RepoSection;
pub use tmpl::{TmplSection, VariableDefinition};
pub use tool::ToolSection;
