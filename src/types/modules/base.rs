use serde::{Deserialize, Serialize, Serializer, Deserializer};
use serde::de::{self, Visitor};
use std::fmt;
use std::collections::BTreeMap;
use toml;
use chrono;

/// Version specification for dependencies
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum VersionSpec {
    /// Exact version code
    Exact(u64),
    /// Version range (e.g., "[1000,2000)")
    Range(String),
}

impl VersionSpec {
    pub fn as_display(&self) -> String {
        match self {
            VersionSpec::Exact(v) => v.to_string(),
            VersionSpec::Range(r) => r.clone(),
        }
    }
}

/// A dependency entry
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Dependency {
    /// Module ID
    pub id: String,
    /// Version specification
    #[serde(rename = "versionCode")]
    pub version_code: Option<VersionSpec>,
    /// Optional source URL
    pub source: Option<String>,
}

/// Dependency section with kam and dev groups
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DependencySection {
    /// Runtime dependencies
    pub kam: Option<Vec<Dependency>>,
    /// Development dependencies
    pub dev: Option<Vec<Dependency>>,
}

impl Default for DependencySection {
    fn default() -> Self {
        DependencySection {
            kam: Some(Vec::new()),
            dev: Some(Vec::new()),
        }
    }
}

const DEFAULT_DEPENDENCY_SOURCE: &str = "https://github.com/MemDeco-WG/Kam-Index";

/// KamToml: A superset of module.prop, update.json, and other metadata,
/// inspired by pyproject.toml format with hierarchical sections.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct KamToml {
    pub prop: PropSection,
    pub mmrl: Option<MmrlSection>,
    pub kam: KamSection,
    pub tool: Option<serde_json::Value>,
    pub tmpl: Option<serde_json::Value>,
    pub lib: Option<serde_json::Value>,
    #[serde(skip)]
    pub raw: String,
}

impl Default for KamToml {
    fn default() -> Self {
        // Use defaults from section Default impls where appropriate.
        let mut default = KamToml::from_prop(PropSection::default());
        default.mmrl = Some(MmrlSection::default());
        default.kam = KamSection::default();
        default.raw = "".to_string();
        default
    }
}

impl KamToml {
    /// Construct a KamToml starting from a PropSection (useful for default
    /// composition). This helper keeps the same signature as other
    /// constructors in this module.
    pub fn from_prop(prop: PropSection) -> Self {
        KamToml {
            prop,
            mmrl: Some(MmrlSection::default()),
            kam: KamSection::default(),
            tool: None,
            tmpl: None,
            lib: None,
            raw: String::new(),
        }
    }

    /// Create a new KamToml with current timestamp for versionCode
    pub fn new_with_current_timestamp(
        id: String,
        name: BTreeMap<String, String>,
        version: String,
        author: String,
        description: BTreeMap<String, String>,
        module_type: Option<ModuleType>,
    ) -> Self {
        let mut kt = KamToml::from_prop(PropSection {
            id,
            name,
            version,
            versionCode: chrono::Utc::now().timestamp_millis() as u64,
            author,
            description,
            updateJson: Some("https://example.com/update.json".to_string()),
        });
        if let Some(mt) = module_type {
            kt.kam.module_type = mt;
        }
        kt
    }

    /// Load KamToml from a directory (looks for kam.toml)
    pub fn load_from_dir<P: AsRef<std::path::Path>>(dir: P) -> crate::errors::Result<Self> {
        let path = dir.as_ref().join("kam.toml");
        Self::load_from_file(path)
    }

    /// Load KamToml from a file
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> crate::errors::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut kt: KamToml = toml::from_str(&content)?;
        kt.raw = content;
        Ok(kt)
    }

    /// Write KamToml to a directory as kam.toml
    pub fn write_to_dir<P: AsRef<std::path::Path>>(&self, dir: P) -> crate::errors::Result<()> {
        let path = dir.as_ref().join("kam.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Apply template variables to the KamToml structure
    pub fn apply_vars(&mut self, kam_vars: Vec<(String, String)>) -> crate::errors::Result<()> {
        for (key, value) in kam_vars {
            // Remove leading '#'
            let key = key.strip_prefix('#').unwrap_or(&key);
            // Apply to prop section
            match key {
                "prop.id" => self.prop.id = value,
                "prop.version" => self.prop.version = value,
                "prop.author" => self.prop.author = value,
                "prop.versionCode" => {
                    if let Ok(vc) = value.parse::<u64>() {
                        self.prop.versionCode = vc;
                    }
                }
                _ => {
                    // Handle nested keys like prop.name.en
                    if key.starts_with("prop.name.") {
                        let lang = key.strip_prefix("prop.name.").unwrap();
                        self.prop.name.insert(lang.to_string(), value);
                    } else if key.starts_with("prop.description.") {
                        let lang = key.strip_prefix("prop.description.").unwrap();
                        self.prop.description.insert(lang.to_string(), value);
                    }
                }
            }
        }
        Ok(())
    }

    /// Get effective source URL for dependencies
    pub fn get_effective_source(dep: &Dependency) -> String {
        dep.source.clone().unwrap_or_else(|| DEFAULT_DEPENDENCY_SOURCE.to_string())
    }

    /// Resolve dependencies into flattened groups
    pub fn resolve_dependencies(&self) -> crate::errors::Result<crate::dependency_resolver::FlatDependencyGroups> {
        use crate::dependency_resolver::DependencyResolver;
        let resolver = DependencyResolver::new(self.kam.dependency.as_ref().unwrap_or(&DependencySection::default()));
        resolver.resolve().map_err(|e| KamError::DependencyResolutionFailed(e.to_string()))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct PropSection {
    pub id: String,
    pub name: BTreeMap<String, String>,
    pub version: String,
    #[serde(rename = "versionCode")]
    pub version_code: u64,
    pub author: String,
    pub description: BTreeMap<String, String>,
    #[serde(rename = "updateJson")]
    pub update_json: Option<String>,
}

impl PropSection {
    pub fn get_name(&self) -> &str {
        if let Some(v) = self.name.get("en") {
            v.as_str()
        } else if let Some((_k, v)) = self.name.iter().next() {
            v.as_str()
        } else {
            ""
        }
    }

    pub fn get_description(&self) -> &str {
        if let Some(v) = self.description.get("en") {
            v.as_str()
        } else if let Some((_k, v)) = self.description.iter().next() {
            v.as_str()
        } else {
            ""
        }
    }
}

impl Default for PropSection {
    fn default() -> Self {
        let mut name = std::collections::BTreeMap::new();
        name.insert("en".to_string(), "My Module".to_string());
        let mut description = std::collections::BTreeMap::new();
        description.insert("en".to_string(), "A module description".to_string());
        PropSection {
            id: "my_module".to_string(),
            name,
            version: "0.1.0".to_string(),
            versionCode: 1,
            author: "Author".to_string(),
            description,
            updateJson: Some("https://example.com/update.json".to_string()),
        }
    }
}

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
    /// Changelog 文件名（相对路径），例如 `CHANGELOG.md`
    pub changelog: Option<String>,
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
            changelog: Some("CHANGELOG.md".to_string()),
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 表示库模块向外提供的单个提供项
pub struct Provide {
    /// 提供的名称，作为其他模块在依赖时可以引用的 id
    pub name: String,
    /// 相对路径（相对于模块根）指向实际实现文件或目录，可选
    pub path: Option<String>,
}

impl Default for Provide {
    fn default() -> Self {
        Provide {
            name: String::new(),
            path: None,
        }
    }
}


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
#[serde(rename_all = "lowercase")]
/// 模块类型（序列化为字符串，用于 `kam.toml` 中的 `module_type` 字段）
///
/// - `kam`：代表一个可发布的 Kam 模块
/// - `template`：代表一个模板（用于生成其他模块）
/// - `library`：代表一个仅作为库使用的模块
/// - `repo`：代表一个模块仓库
pub enum ModuleType {
    Kam,
    Template,
    Library,
    Repo,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 模板变量的定义
///
/// 用于描述模板中可被替换的变量的类型、是否必需以及可选的默认值。
pub struct VariableDefinition {
    /// 变量类型（例如 "string"、"bool"、"number" 等，自由约定）
    pub var_type: String,
    /// 是否为必需变量（未提供时模板引擎应报错或提示）
    pub required: bool,
    /// 可选的默认值（作为字符串表示）
    pub default: Option<String>,
    /// 可选的提示信息（模板作者可提供）
    ///
    /// 当变量为必需且未提供时，模板会使用此字段作为更友好的错误或提示文本，
    /// 告知用户如何通过命令行参数或变量传入该值。例如：
    /// "请运行 `kam init ... --var name=...` 来设置模块名称"
    pub note: Option<String>,
    /// 更详细的帮助文本，适合显示给用户，解释该变量的语义或格式。
    pub help: Option<String>,
    /// 示例值，供模板作者给出可选的示例输入。
    pub example: Option<String>,
    /// 可选的枚举候选项，模板或交互式提示可以用来展示可选值。
    pub choices: Option<Vec<String>>,
}

impl Default for VariableDefinition {
    fn default() -> Self {
        VariableDefinition {
            var_type: "string".to_string(),
            required: false,
            default: None,
            note: None,
            help: None,
            example: None,
            choices: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 模板相关配置节，用于在模块中引用/配置子模板
///
/// - `used_template`：可选引用的内置或自定义模板 id
/// - `variables`：模板变量定义表（变量名 -> 定义）
pub struct TmplSection {
    pub used_template: Option<String>,
    pub variables: BTreeMap<String, VariableDefinition>,
}

impl Default for TmplSection {
    fn default() -> Self {
        TmplSection {
            used_template: None,
            variables: BTreeMap::new(),
        }
    }
}

/// 支持的 CPU 架构枚举（序列化为字符串，例如 "arm", "arm64", "x86_64"）
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum SupportedArch {
    Arm,
    Arm64,
    X86,
    X86_64,
    Other(String),
}

impl Serialize for SupportedArch {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            SupportedArch::Arm => serializer.serialize_str("arm"),
            SupportedArch::Arm64 => serializer.serialize_str("arm64"),
            SupportedArch::X86 => serializer.serialize_str("x86"),
            SupportedArch::X86_64 => serializer.serialize_str("x86_64"),
            SupportedArch::Other(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> Deserialize<'de> for SupportedArch {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArchVisitor;

        impl<'de> Visitor<'de> for ArchVisitor {
            type Value = SupportedArch;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a CPU architecture string")
            }

            fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                let key = v.trim();
                let key_lc = key.to_ascii_lowercase();
                Ok(match key_lc.as_str() {
                    // ARM family aliases
                    "arm" | "armv7" | "armv7l" | "armv6" | "armhf" => SupportedArch::Arm,
                    // ARM64 / AArch64
                    "arm64" | "aarch64" => SupportedArch::Arm64,
                    // 32-bit x86 aliases
                    "x86" | "i386" | "i486" | "i586" | "i686" => SupportedArch::X86,
                    // 64-bit x86 aliases
                    "x86_64" | "x64" | "amd64" => SupportedArch::X86_64,
                    other => SupportedArch::Other(other.to_string()),
                })
            }
        }

        deserializer.deserialize_str(ArchVisitor)
    }
}

impl fmt::Display for SupportedArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SupportedArch::Arm => "arm",
            SupportedArch::Arm64 => "arm64",
            SupportedArch::X86 => "x86",
            SupportedArch::X86_64 => "x86_64",
            SupportedArch::Other(s) => return write!(f, "{}", s),
        };
        write!(f, "{}", s)
    }
}

// Allow comparing SupportedArch and String bidirectionally so existing code
// that works with `String` (e.g. `Vec<String>::contains`) keeps working.
impl PartialEq<String> for SupportedArch {
    fn eq(&self, other: &String) -> bool {
        self.to_string() == *other
    }
}

impl PartialEq<SupportedArch> for String {
    fn eq(&self, other: &SupportedArch) -> bool {
        *self == other.to_string()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 库模块的配置节
///
/// 注意：库模块宣告的是“它能向其他模块提供什么依赖（provides）”，
/// 而不是它自己需要哪些依赖（dependencies）。
///
/// `provides` 列表描述该库模块对外提供的接口/标识及可选的版本信息。
pub struct LibSection {
    /// 对外提供的条目（name + 可选 path），类似 Cargo 的 `[[bin]]` 声明。
    pub provides: Option<Vec<Provide>>,
}

impl Default for LibSection {
    fn default() -> Self {
        LibSection {
            provides: Some(Vec::new()),
        }
    }
}

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 打包/构建配置节
///
/// - `target_dir`：打包输出目录，默认 "dist"
/// - `output_file`：可选的输出文件名（为空时使用 `<id>-<version>.zip`）
/// - `pre_build` / `post_build`：可选的 shell/PowerShell 钩子命令字符串
pub struct BuildSection {
    pub target_dir: Option<String>,
    pub output_file: Option<String>,
    pub pre_build: Option<String>,
    pub post_build: Option<String>,
}

impl Default for BuildSection {
    fn default() -> Self {
        // Provide sensible cross-platform defaults for pre/post build hooks.
        // On Windows we use a simple echo, on other platforms use echo as well
        // but prefer single quotes to avoid PowerShell vs shell quoting issues.
        let pre = if cfg!(target_os = "windows") {
            Some("echo \"pre build...\"".to_string())
        } else {
            Some("echo 'pre build...'".to_string())
        };

        let post = if cfg!(target_os = "windows") {
            Some("echo \"post build...\"".to_string())
        } else {
            Some("echo 'post build...'".to_string())
        };

        BuildSection {
            target_dir: Some("dist".to_string()),
            output_file: Some(String::new()),
            pre_build: pre,
            post_build: post,
        }
    }
}

use std::path::{Path, PathBuf};
// use git2 for repository operations instead of shelling out to `git`
use git2::{Cred, RemoteCallbacks, FetchOptions, build::RepoBuilder, CredentialType};
use tempfile::tempdir;
use std::fs;
use std::io::{self};

use crate::errors::{KamError, Result};
use crate::cache::KamCache;
use crate::types::source::Source;

/// A lightweight abstraction of a Kam module. Owns a KamToml and an optional Source.
#[derive(Debug, Clone)]
pub struct KamModule {
    pub toml: KamToml,
    pub source: Option<Source>,
}

/// Trait for module backends that can fetch and install module sources.
pub trait ModuleBackend {
    fn canonical_cache_name(&self) -> Option<String>;
    fn fetch_to_temp(&self) -> Result<PathBuf>;
    fn install_into_cache(&self, cache: &KamCache) -> Result<PathBuf>;
}

/// ModuleBackend contract and semantics
///
/// Implementers of this trait provide three responsibilities:
/// - `canonical_cache_name` (optional): return a stable name to install the
///   module under in the cache (typically `id-version`). When `None` the
///   caller will derive a name from the source.
/// - `fetch_to_temp`: fetch the module source and return a filesystem path
///   containing the unpacked source. The returned path points to a persisted
///   directory that the caller may inspect. Ownership/cleanup: implementers
///   are allowed to persist the fetched data (for example by using a
///   temporary directory that is "kept"). Callers that do not need the
///   intermediate copy should remove it when finished. In short, the caller
///   is responsible for deleting the returned path if it should not be kept.
/// - `install_into_cache`: move or copy the fetched contents into the
///   provided `KamCache` and return the destination path inside the cache.
///
/// Concurrency / atomicity: this trait does not prescribe locking semantics.
/// The default `KamModule` implementation will overwrite an existing
/// destination (remove + copy). If callers require concurrent-safe installs
/// they should implement higher-level locking (for example file locks or
/// a per-cache mutex) around calls to `install_into_cache`.
///
/// Note: the trait is intentionally small so callers can mock or provide
/// alternate backends (HTTP, Git, local archives, etc.).

impl KamModule {
    /// Create from an owned KamToml and optional Source.
    pub fn new(toml: KamToml, source: Option<Source>) -> Self {
        Self { toml, source }
    }

    /// Parse a source spec string and attach it to the KamModule constructed from KamToml.
    pub fn from_spec_and_toml(spec: &str, toml: KamToml) -> Result<Self> {
        let src = Source::parse(spec).map_err(|e| KamError::ParseSourceFailed(format!("parse source spec: {}", e)))?;
        Ok(Self::new(toml, Some(src)))
    }

    /// Return a canonical name for installing into cache: id-version when available.
    pub fn canonical_cache_name(&self) -> Option<String> {
        let id = &self.toml.prop.id;
        let ver = &self.toml.prop.version;
        if !id.is_empty() && !ver.is_empty() {
            Some(format!("{}-{}", id, ver))
        } else {
            None
        }
    }

    /// Fetch the module source into a temporary directory and return the path.
    ///
    /// This is a synchronous/blocking helper. It does not permanently install into the cache.
    pub fn fetch_to_temp(&self) -> Result<PathBuf> {
        let src = match &self.source {
            Some(s) => s.clone(),
            None => return Err(KamError::ParseSourceFailed("no source specified for module".to_string())),
        };

        match src {
            Source::Local { path } => {
                let p = fs::canonicalize(&path).map_err(|e| KamError::Io(e))?;
                if p.is_file() {
                    let tmp = tempdir()?;
                    extract_archive(&p, tmp.path())?;
                    let kept = tmp.keep();
                    Ok(kept)
                } else {
                    let tmp = tempdir()?;
                    let dst = tmp.path().join("src");
                    fs::create_dir_all(&dst)?;
                    copy_dir_all(&p, &dst)?;
                    let kept = tmp.keep();
                    Ok(kept)
                }
            }
            Source::Url { url } => {
                let tmp = tempdir()?;
                let resp = reqwest::blocking::get(&url).map_err(|e| KamError::FetchFailed(format!("failed to download {}: {}", url, e)))?;
                if !resp.status().is_success() {
                    return Err(KamError::FetchFailed(format!("download failed: {} -> {}", url, resp.status())));
                }

                let mut data = Vec::new();
                let mut reader = resp;
                reader.copy_to(&mut data).map_err(|e| KamError::FetchFailed(format!("read download body: {}", e)))?;

                    if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
                        let file = tmp.path().join("download.tar.gz");
                        fs::write(&file, &data)?;
                        extract_tar_gz(&file, tmp.path())?;
                        let kept = tmp.keep();
                        return Ok(kept);
                    } else if url.ends_with(".zip") {
                        let file = tmp.path().join("download.zip");
                        fs::write(&file, &data)?;
                        extract_zip(&file, tmp.path())?;
                        let kept = tmp.keep();
                        return Ok(kept);
                    } else {
                        let file = tmp.path().join("download.bin");
                        fs::write(&file, &data)?;
                        let kept = tmp.keep();
                        return Ok(kept);
                    }
            }
            Source::Git { url, rev } => {
                let tmp = tempdir()?;

                // Prepare credential callbacks: try SSH agent first, then optional
                // SSH key path (KAM_GIT_SSH_KEY_PATH), token (KAM_GIT_TOKEN), or
                // username/password (KAM_GIT_USERNAME / KAM_GIT_PASSWORD).
                let mut callbacks = RemoteCallbacks::new();
                callbacks.credentials(move |_, username_from_url, allowed| {
                    // 1) SSH agent
                    if allowed.contains(CredentialType::SSH_KEY) {
                        if let Some(user) = username_from_url {
                            if let Ok(c) = Cred::ssh_key_from_agent(user) {
                                return Ok(c);
                            }
                        }
                        if let Ok(c) = Cred::ssh_key_from_agent("git") {
                            return Ok(c);
                        }
                    }

                    // 2) SSH key file provided via env
                    if allowed.contains(CredentialType::SSH_KEY) {
                        if let Ok(key_path) = std::env::var("KAM_GIT_SSH_KEY_PATH") {
                            let user = username_from_url.unwrap_or("git");
                            // try public key path as key_path + ".pub"
                            let pubkey_buf = std::path::PathBuf::from(format!("{}.pub", key_path));
                            let privkey_buf = std::path::PathBuf::from(&key_path);
                            let pubkey = pubkey_buf.as_path();
                            let privkey = privkey_buf.as_path();
                            if privkey.exists() {
                                // ignore potential errors and try
                                if let Ok(c) = Cred::ssh_key(user, Some(pubkey), privkey, None) {
                                    return Ok(c);
                                }
                            }
                        }
                    }

                    // 3) Token via env (use as basic auth password)
                    if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
                        if let Ok(token) = std::env::var("KAM_GIT_TOKEN") {
                            // Some providers accept username 'x-access-token' or 'git'
                            return Cred::userpass_plaintext("x-access-token", &token);
                        }
                        if let (Ok(user), Ok(pass)) = (std::env::var("KAM_GIT_USERNAME"), std::env::var("KAM_GIT_PASSWORD")) {
                            return Cred::userpass_plaintext(&user, &pass);
                        }
                    }

                    // Fallback
                    Cred::default()
                });

                let mut fo = FetchOptions::new();
                fo.remote_callbacks(callbacks);
                // request a shallow clone (depth 1) for remote transports.
                // Some local transports (file://) don't support shallow fetches,
                // so only set depth for non-file URLs.
                if !url.starts_with("file://") {
                    fo.depth(1);
                }

                let mut builder = RepoBuilder::new();
                builder.fetch_options(fo);

                let repo = builder.clone(&url, tmp.path()).map_err(|e| KamError::FetchFailed(format!("git clone {}: {}", url, e)))?;

                if let Some(r) = rev {
                    let obj = repo.revparse_single(&r).map_err(|e| KamError::FetchFailed(format!("resolve rev {}: {}", r, e)))?;
                    repo.checkout_tree(&obj, None).map_err(|e| KamError::FetchFailed(format!("checkout tree: {}", e)))?;
                    repo.set_head_detached(obj.id()).map_err(|e| KamError::FetchFailed(format!("set HEAD: {}", e)))?;
                }

                let kept = tmp.keep();
                Ok(kept)
            }
        }
    }

    /// Install (move) the fetched source into the cache under a canonical name if available.
    /// Returns the destination path in the cache.
    pub fn install_into_cache(&self, cache: &KamCache) -> Result<PathBuf> {
        let src_path = self.fetch_to_temp()?;

        // Determine destination name
        let dest_name = if let Some(name) = self.canonical_cache_name() {
            name
        } else {
            match &self.source {
                Some(Source::Git { url, .. }) => sanitize_name(url),
                Some(Source::Url { url }) => sanitize_name(url),
                Some(Source::Local { path }) => sanitize_name(&path.to_string_lossy()),
                    None => return Err(KamError::ParseSourceFailed("no source available to derive name".to_string())),
            }
        };

        let dest = cache.lib_dir().join(dest_name);

        // Remove any existing destination to ensure a clean install
        if dest.exists() {
            fs::remove_dir_all(&dest)?;
        }

        // Try to perform a cheap/atomic move (rename) to avoid copy when
        // possible ("zero-copy" case). This will succeed when the source
        // and destination are on the same filesystem. If rename fails we
        // fall back to copying the contents.
        //
        // Handle the common case where `src_path` contains a single child
        // directory that actually holds the module root — in that case try
        // to rename that child into place first.
        let entries: Vec<_> = fs::read_dir(&src_path)?.collect();
        if entries.len() == 1 {
            let only = entries[0].as_ref().unwrap().path();
            if only.is_dir() {
                // attempt rename of the single-child dir
                if let Err(_e) = fs::rename(&only, &dest) {
                    // rename failed (likely cross-device) -> copy fallback
                    copy_dir_all(&only, &dest)?;
                    // attempt to remove the original temporary tree
                    let _ = fs::remove_dir_all(&src_path);
                }
                return Ok(dest);
            }
        }

        // Otherwise attempt to rename the fetched root dir directly
        if let Err(_e) = fs::rename(&src_path, &dest) {
            // rename failed (e.g. cross-device); create dest and copy
            fs::create_dir_all(&dest)?;
            copy_dir_all(&src_path, &dest)?;
            // try to remove the temporary source tree; ignore errors
            let _ = fs::remove_dir_all(&src_path);
        }

        Ok(dest)
    }
}

// Implement the ModuleBackend trait for KamModule so callers can use the
// abstraction explicitly.
impl ModuleBackend for KamModule {
    fn canonical_cache_name(&self) -> Option<String> { self.canonical_cache_name() }
    fn fetch_to_temp(&self) -> Result<PathBuf> { self.fetch_to_temp() }
    fn install_into_cache(&self, cache: &KamCache) -> Result<PathBuf> { self.install_into_cache(cache) }
}

fn sanitize_name(s: &str) -> String {
    let mut out = s.replace("https://", "").replace("http://", "");
    out = out.replace(['/', ':', '@'], "-");
    if out.ends_with(".git") {
        out.truncate(out.len() - 4);
    }
    out
}

// Small helpers (no external utils module required)
fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else if file_type.is_file() {
            fs::copy(&entry.path(), &dest_path)?;
        } else if file_type.is_symlink() {
            // attempt to copy symlink target as file/dir depending on target
            let target = fs::read_link(entry.path())?;
            if target.is_dir() {
                copy_dir_all(&target, &dest_path)?;
            } else {
                fs::copy(&target, &dest_path)?;
            }
        }
    }
    Ok(())
}

fn extract_zip(zip_path: &Path, dst: &Path) -> Result<()> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i)?;
        let outpath = dst.join(f.name());
        if f.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut f, &mut outfile)?;
        }
    }
    Ok(())
}

fn extract_tar_gz(tar_path: &Path, dst: &Path) -> Result<()> {
    let f = fs::File::open(tar_path)?;
    let decompressor = flate2::read::GzDecoder::new(f);
    let mut archive = tar::Archive::new(decompressor);
    archive.unpack(dst)?;
    Ok(())
}

fn extract_archive(path: &Path, dst: &Path) -> Result<()> {
    let s = path.to_string_lossy().to_lowercase();
    if s.ends_with(".zip") {
        extract_zip(path, dst)?;
    } else if s.ends_with(".tar.gz") || s.ends_with(".tgz") {
        extract_tar_gz(path, dst)?;
    } else {
        return Err(KamError::UnsupportedArchive(format!("unsupported archive format: {}", path.display())));
    }
    Ok(())
}
