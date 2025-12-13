use clap::Args;

// init命令的参数
#[derive(Args, Debug)]
pub struct InitArgs {
    // 项目路径
    // 交互模式下可以省略，会提示用户输入
    #[arg(value_name = "PATH", required_unless_present = "interactive")]
    pub name: Option<String>,

    // 项目ID（默认：文件夹名）
    #[arg(long)]
    pub id: Option<String>,

    // 项目名称（默认："Example Module Name"）
    #[arg(long)]
    pub project_name: Option<String>,

    // 项目版本（默认："1.0.0"）
    #[arg(long)]
    pub version: Option<String>,

    // 作者名字（默认："Your Name"）
    #[arg(long)]
    pub author: Option<String>,

    // Update JSON URL（默认：从git自动生成）
    #[arg(long)]
    pub update_json: Option<String>,

    // 描述（默认："Describe your module here"）
    #[arg(long)]
    pub description: Option<String>,

    // 强制覆盖已存在的文件
    #[arg(short, long)]
    pub force: bool,

    // 交互模式运行，会询问必需的值
    #[arg(short = 'i', long = "interactive")]
    pub interactive: bool,

    // 已废弃：实现模板源（本地路径、URL或git仓库）
    // 注意：这个选项已经从CLI移除了，用-t/--template和--tmpl来选择内置模板
    #[arg(skip)]
    pub r#impl: Option<String>,

    // 模板变量，key=value格式
    #[arg(long)]
    pub var: Vec<String>,

    // 要使用的模板（内置ID或本地路径）
    #[arg(short, long)]
    pub template: Option<String>,

    // 创建模板项目
    // 模板ID："tmpl_template"
    #[arg(long)]
    pub tmpl: bool,
}
