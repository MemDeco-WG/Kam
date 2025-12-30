use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[allow(non_snake_case)]
// 工具配置节，用于自定义工具配置
// 目前用JSON值存储，以后可能需要扩展
#[derive(Default)]
pub struct ToolSection {
    // 暂时用JSON值存储，需要时再扩展
    pub data: Option<serde_json::Value>,
}
