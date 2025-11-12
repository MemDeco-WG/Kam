
use serde::{Serialize, Deserialize};
use super::repo::RepoSection;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// MMRL 顶层节：包含仓库/发布相关的子配置
pub struct MmrlSection {
    /// 仓库相关元数据，包含许可、主页、屏幕截图等信息
    pub repo: Option<RepoSection>,
}

impl Default for MmrlSection {
    fn default() -> Self {
        MmrlSection { repo: Some(RepoSection::default()) }
    }
}
