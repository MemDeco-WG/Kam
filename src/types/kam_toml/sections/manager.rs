
use serde::{Serialize, Deserialize};

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
