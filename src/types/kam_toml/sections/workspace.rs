use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// Workspace section for Kam workspace management, similar to Cargo workspaces
pub struct WorkspaceSection {
    /// List of workspace members (paths relative to the workspace root)
    pub members: Option<Vec<String>>,
    /// List of paths to exclude from the workspace
    pub exclude: Option<Vec<String>>,
}

impl Default for WorkspaceSection {
    fn default() -> Self {
        WorkspaceSection {
            members: Some(vec![".".to_string()]),
            exclude: None,
        }
    }
}
