
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;

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
