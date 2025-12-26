use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[allow(non_snake_case)]
// 提示/通知结构（MMRL V4+不支持color字段）
#[derive(Default)]
pub struct NoteSection {
    // 通知标题
    pub title: String,
    // 通知正文/消息
    pub message: String,
}

