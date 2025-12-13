use clap::Args;

// about命令的参数
// 这个命令只是显示信息，不会修改任何文件
// 就是打印个好看的about框（虽然可能没啥用）
#[derive(Args, Debug)]
pub struct AboutArgs {}
