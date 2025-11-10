use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 仓库/发布信息节，包含展示与分发相关的元数据
pub struct RepoSection {
    /// 许可证文件名或 SPDX 标识符（常用默认为 `LICENSE`）
    pub license: Option<String>,
    /// 项目主页 URL
    pub homepage: Option<String>,
    /// README 文件名（相对路径），例如 `README.md`
    pub readme: Option<String>,
    /// 屏幕截图 URL 列表
    pub screenshots: Option<Vec<String>>,
    /// 类别标签列表（可用于分类展示）
    pub categories: Option<Vec<String>>,
    /// 关键字标签列表，便于搜索或索引
    pub keywords: Option<Vec<String>>,
    /// 维护者列表（用户名或联系方式）
    pub maintainers: Option<Vec<String>>,
    /// 源代码仓库地址（例如 GitHub 仓库 URL）
    pub repository: Option<String>,
    /// 文档链接（外部文档或站点）
    pub documentation: Option<String>,
    /// 问题跟踪（issues）链接
    pub issues: Option<String>,
    /// 资助/捐赠链接集合或描述
    pub funding: Option<String>,
    /// 官方支持入口（例如 issue 页面或支持站点）
    pub support: Option<String>,
    /// 捐赠链接（例如 PayPal 等）
    pub donate: Option<String>,
    /// 封面图片 URL（适用于展示）
    pub cover: Option<String>,
    /// 图标 URL（可用于 UI 显示）
    pub icon: Option<String>,
    /// 支持或针对的设备列表（字符串标识）
    pub devices: Option<Vec<String>>,
    /// 支持的 CPU 架构列表（如 arm64-v8a）
    pub arch: Option<Vec<String>>,
    /// 运行或安装所需的其它模块/组件标识列表
    pub require: Option<Vec<String>>,
    /// 可显示的提示/通知块（标题、消息和颜色）
    pub note: Option<NoteSection>,
    /// 各种包管理器/平台的最小版本或需求配置
    pub manager: Option<ManagerSection>,
    /// 与模块不兼容/禁用的功能标签
    pub antifeatures: Option<Vec<String>>,
    /// 额外选项（例如归档压缩配置）
    pub options: Option<OptionsSection>,
    /// 最大数量（语义依赖于上层使用场景，默认 0 表示未设置）
    pub max_num: Option<u64>,
    /// 模块所需的最小 Kam API 版本
    pub min_api: Option<u32>,
    /// 模块支持的最大 Kam API 版本
    pub max_api: Option<u32>,
    /// 是否经过验证（verified 标记）
    pub verified: Option<bool>,
    /// 模块提供的功能/特性列表，用于展示、索引或描述模块能力。
    pub features: Option<Vec<String>>,
}

impl Default for RepoSection {
    fn default() -> Self {
        RepoSection {
            license: Some("LICENSE".to_string()),
            homepage: Some(String::new()),
            readme: Some("README.md".to_string()),
            screenshots: Some(vec![]),
            categories: Some(vec![]),
            keywords: Some(vec![]),
            maintainers: Some(vec![]),
            repository: Some(String::new()),
            documentation: Some(String::new()),
            issues: Some(String::new()),
            funding: Some(String::new()),
            support: Some(String::new()),
            donate: Some(String::new()),
            cover: Some(String::new()),
            icon: Some(String::new()),
            devices: Some(vec![]),
            arch: Some(vec![]),
            require: Some(vec![]),
            note: Some(NoteSection::default()),
            manager: Some(ManagerSection::default()),
            antifeatures: Some(vec![]),
            options: Some(OptionsSection::default()),
            max_num: Some(0),
            min_api: Some(0),
            max_api: Some(0),
            verified: Some(false),
            features: Some(vec![]),
        }
    }
}



#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 简单的提示/通知结构
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 管理器平台的配置（例如 magisk、kernelsu、apatch）
pub struct ManagerConfig {
    /// 最低兼容版本或约束（按字符串表达）
    pub min: Option<String>,
    /// 支持的设备列表（可用于过滤）
    pub devices: Option<Vec<String>>,
    /// 支持的架构列表
    pub arch: Option<Vec<String>>,
    /// 依赖的其它模块/组件标识
    pub require: Option<Vec<String>>,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        ManagerConfig { min: None, devices: Some(vec![]), arch: Some(vec![]), require: Some(vec![]) }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 不同包管理器或平台的配置组合
pub struct ManagerSection {
    /// Magisk 相关配置
    pub magisk: Option<ManagerConfig>,
    /// kernelsu 相关配置
    pub kernelsu: Option<ManagerConfig>,
    /// apatch 相关配置
    pub apatch: Option<ManagerConfig>,
}

impl Default for ManagerSection {
    fn default() -> Self {
        ManagerSection { magisk: Some(ManagerConfig::default()), kernelsu: Some(ManagerConfig::default()), apatch: Some(ManagerConfig::default()) }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 额外选项节（用于放置扩展配置，例如归档参数）
pub struct OptionsSection {
    /// 归档相关选项（比如压缩方式）
    pub archive: Option<ArchiveOptions>,
}

impl Default for OptionsSection {
    fn default() -> Self {
        OptionsSection { archive: Some(ArchiveOptions::default()) }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 归档选项（比如压缩算法名称或参数）
pub struct ArchiveOptions {
    /// 压缩方式（如 "Deflate"、"Store" 等），空字符串表示未指定
    pub compression: Option<String>,
}

impl Default for ArchiveOptions {
    fn default() -> Self {
        ArchiveOptions { compression: Some(String::new()) }
    }
}
