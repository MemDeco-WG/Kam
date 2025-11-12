use serde::{Serialize, Deserialize};

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
