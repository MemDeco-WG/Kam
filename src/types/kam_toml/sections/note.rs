use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 提示/通知结构（MMRL V4+ 不支持 color 字段）
pub struct NoteSection {
    /// 通知标题
    pub title: String,
    /// 通知正文/消息
    pub message: String,
}

impl Default for NoteSection {
    fn default() -> Self {
        NoteSection {
            title: String::new(),
            message: String::new(),
        }
    }
}
