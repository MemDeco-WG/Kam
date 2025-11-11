use serde::{Deserialize, Serialize, Serializer, Deserializer};
use serde::de::{self, Visitor};
use std::fmt;
use std::collections::BTreeMap;
use crate::types::kam_toml::dependency::{DependencySection};

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
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
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
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArchVisitor;

        impl<'de> Visitor<'de> for ArchVisitor {
            type Value = SupportedArch;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a CPU architecture string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
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
