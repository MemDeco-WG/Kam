use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// Tool section for custom tool configurations
pub struct ToolSection {
    // Add fields as needed
    pub data: Option<serde_json::Value>,
}

impl Default for ToolSection {
    fn default() -> Self {
        ToolSection { data: None }
    }
}
