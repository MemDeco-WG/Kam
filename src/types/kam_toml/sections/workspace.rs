use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[allow(non_snake_case)]
// 工作区配置节，用于Kam工作区管理，类似Cargo的workspaces
pub struct WorkspaceSection {
    // 工作区成员列表（相对于工作区根的路径）
    pub members: Option<Vec<String>>,
    // 要从工作区排除的路径列表
    pub exclude: Option<Vec<String>>,
}

impl Default for WorkspaceSection {
    fn default() -> Self {
        Self {
            members: Some(vec![".".to_string()]),
            exclude: None,
        }
    }
}
