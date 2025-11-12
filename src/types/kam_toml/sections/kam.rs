
use serde::{Serialize, Deserialize};
use super::{SupportedArch, ModuleType, DependencySection, BuildSection, TmplSection, LibSection};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// `[kam]` 部分的高层结构，包含与 Kam 平台相关的配置
///
/// 该结构反映 `kam.toml` 中的部分字段，许多字段是可选的（Option），
/// 但 Default 实现会提供合理的空值以便模板和代码更容易使用。
pub struct KamSection {
    /// 最低兼容 API 版本（0 表示未指定或所有版本）
    pub min_api: Option<u32>,
    /// 最高兼容 API 版本（0 表示未指定或不限制）
    pub max_api: Option<u32>,
    /// 支持的 CPU 架构列表（例如 ["arm", "arm64"]）
    pub supported_arch: Option<Vec<SupportedArch>>,
    /// 与该模块冲突的模块 ID 列表
    pub conflicts: Option<Vec<String>>,
    /// 依赖声明（分 kam / dev）
    pub dependency: Option<DependencySection>,
    /// 打包/构建相关的配置
    pub build: Option<BuildSection>,
    /// 模块类型（kam/template/library）
    pub module_type: ModuleType,
    /// 模板相关子配置
    pub tmpl: Option<TmplSection>,
    /// 库相关子配置
    pub lib: Option<LibSection>,
}

impl Default for KamSection {
    fn default() -> Self {
        KamSection {
            min_api: Some(0),
            max_api: Some(0),
            supported_arch: Some(Vec::new()),
            conflicts: Some(Vec::new()),
            dependency: Some(DependencySection::default()),
            build: Some(BuildSection::default()),
            module_type: ModuleType::Kam,
            tmpl: Some(TmplSection::default()),
            lib: Some(LibSection::default()),
        }
    }
}
