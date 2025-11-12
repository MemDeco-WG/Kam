use serde::{Serialize, Deserialize};

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
