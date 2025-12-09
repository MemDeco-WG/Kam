use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ExportArgs {
    /// 导出格式：prop, json, update, repo, track, config
    #[arg(value_enum)]
    pub format: Option<ExportFormat>,
    /// 输出文件路径（默认打印到 stdout）
    pub output: Option<PathBuf>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ExportFormat {
    Prop,
    Json,
    Repo,
    Track,
    Config,
    Update,
}
