use clap::Args;

// 版本命令的参数
#[derive(Args, Debug)]
pub struct VersionArgs {
    // 新版本号（如 1.0.1）或bump类型（major, minor, patch）
    #[arg(value_name = "VERSION")]
    pub version: Option<String>,
}
