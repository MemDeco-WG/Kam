

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 提示/通知结构
pub struct NoteSection {
    /// 通知标题
    pub title: String,
    /// 通知正文/消息
    pub message: String,
    /// 颜色代码（可选，用于 UI 显示）
    pub color: Option<String>,
}

impl Default for NoteSection {
    fn default() -> Self {
        NoteSection { title: String::new(), message: String::new(), color: None }
    }
}
