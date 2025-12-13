//
// keep it simple and stupid.
// I try to keep it simple.
// 👾
// author/human : LIghtJUNction
//
// TODO: 这个main函数有点长，但暂时不想重构
// FIXME: 错误处理可能可以更优雅一点
use clap::Parser;
use dotenvy::dotenv;
use kam::errors::KamError;
use std::error::Error;
use colored::Colorize;

use kam::cli::{Cli, Commands};

fn main() {
    // 加载.env文件（如果有的话），失败了也不管
    dotenv().ok();
    let cli = Cli::parse();

    let res: Result<(), KamError> = match cli.command {
        Commands::Init(args) => kam::cmds::init::run(args),
        Commands::Build(args) => kam::cmds::build::run(args),
        Commands::Version(args) => kam::cmds::version::run(args),
        Commands::Cache(args) => kam::cmds::cache::run(args),
        Commands::Tmpl(args) => kam::cmds::tmpl::run(args),
        Commands::Validate(args) => kam::cmds::validate::run(args),
        Commands::Completions(args) => kam::cmds::completion::run(args),
        Commands::Secret(args) => kam::cmds::secret::run(args),
        Commands::Sign(args) => kam::cmds::sign::run(args),
        Commands::Verify(args) => kam::cmds::verify::run(args),
        Commands::Check(args) => kam::cmds::check::run(args),
        Commands::Export(args) => kam::cmds::export::run(args),
        Commands::Config(args) => kam::cmds::config::run(args),
        Commands::Toml(args) => kam::cmds::toml::run(args),
        Commands::About(args) => kam::cmds::about::run(args),
    };

    if let Err(e) = res {
        use kam::utils::Utils;
        Utils::error(&format!("{}", e));

        // 打印错误链，这样能看到完整的错误信息
        // Rust的error chain还是挺有用的，虽然有时候会打印很多
        let mut source = e.source();
        while let Some(s) = source {
            eprintln!("  {} {}", "→".dimmed(), s.to_string().dimmed());
            source = s.source();
        }

        std::process::exit(1);  // 非零退出码表示失败
    }
    // 如果一切正常，就静默退出（exit code 0）
}
