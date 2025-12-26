use super::{BuildSection, ModuleType, SupportedArch, TmplSection, WorkspaceSection};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[allow(non_snake_case)]
// [kam]部分的高层结构，包含与Kam平台相关的配置
// 这个结构反映kam.toml中的字段，很多字段是可选的
// Default实现会提供合理的空值，方便模板和代码使用
pub struct KamSection {
    // 最低兼容API版本（0表示未指定或所有版本）
    pub min_api: Option<u32>,
    // 最高兼容API版本（0表示未指定或不限制）
    pub max_api: Option<u32>,
    // 支持的CPU架构列表（例如 ["arm", "arm64"]）
    pub supported_arch: Option<Vec<SupportedArch>>,
    // 与该模块冲突的模块ID列表
    pub conflicts: Option<Vec<String>>,

    // 打包/构建相关的配置
    pub build: Option<BuildSection>,
    // 模块类型（kam/template/library）
    pub module_type: ModuleType,
    // 模板相关子配置
    pub tmpl: Option<TmplSection>,

    // 工作区配置
    pub workspace: Option<WorkspaceSection>,
}

impl Default for KamSection {
    fn default() -> Self {
        Self {
            min_api: Some(0),
            max_api: Some(0),
            supported_arch: Some(Vec::new()),
            conflicts: Some(Vec::new()),

            build: Some(BuildSection::default()),
            module_type: ModuleType::Kam,
            tmpl: Some(TmplSection::default()),

            workspace: Some(WorkspaceSection::default()),
        }
    }
}
