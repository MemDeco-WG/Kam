//! `kam env` - 输出所有以 `KAM_` 前缀开头的环境变量（便于调试）
//!
//! 设计原则：简单、确定性输出（按键名排序），便于 pipe / grep / CI 查看。
//!
//! 新增：支持 `--describe` / `-d` 标志，打印 Kam 依赖的环境变量文档（支持 i18n）。
//!
//! 目前实现：
//! - 无需额外参数，直接打印所有 `KAM_` 前缀的环境变量，格式为 `KEY=VALUE`
//! - 使用 `kam env --describe` 打印已知环境变量的说明（从 i18n 中读取 `env.docs.<var>`，键名为小写）
//! - 提供 `collect_kam_env()` 供测试或其它代码重用
use clap::Args;

use crate::errors::KamError;
use crate::utils::Utils;

/// 参数
/// - `--describe, -d`：打印 Kam 使用的已知环境变量文档（从 i18n 读取 `env.docs.<VARNAME>`）
#[derive(Args, Debug)]
pub struct EnvArgs {
    /// Show documentation for known environment variables (i18n)
    #[arg(short = 'd', long = "describe")]
    pub describe: bool,
}

/// 收集所有以 `KAM_` 开头的环境变量，按键名排序后返回 Vec<(key, value)>
/// 通过 `collect_from_iter` 实现，方便在测试中传入自定义迭代器而不修改进程环境
pub fn collect_kam_env() -> Vec<(String, String)> {
    collect_from_iter(std::env::vars())
}

/// 从任意 (String, String) 迭代器收集以 `KAM_` 开头的环境变量并按键排序，供测试复用
pub fn collect_from_iter<I>(iter: I) -> Vec<(String, String)>
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut vars: Vec<(String, String)> = iter
        .into_iter()
        .filter(|(k, _)| k.starts_with("KAM_"))
        .collect();
    vars.sort_by(|a, b| a.0.cmp(&b.0));
    vars
}

/// 执行 `kam env` 命令：根据 args 的不同打印环境变量或说明
pub fn run(args: EnvArgs) -> Result<(), KamError> {
    // Describe known env vars with i18n-backed descriptions
    if args.describe {
        // Header / intro (i18n)
        Utils::section(&crate::i18n::tr_key("env.docs.header"));
        let intro = crate::i18n::tr_key("env.docs.intro");
        if !intro.is_empty() {
            println!("{}", intro);
            println!();
        }

        // A curated list of environment variables used by Kam.
        // Each description should be provided in i18n under keys like `env.docs.kam_home` (lowercase).
        let known_vars = [
            "KAM_HOME",
            "KAM_CACHE_DIR",
            "KAM_ROOT_MANAGER",
            "KAM_UI_LANGUAGE",
            "KAM_LANG",
            "KAM_AUTHOR_EMAIL",
            "KAM_BUMP_ENABLED",
            "KAM_RELEASE_ENABLED",
            "KAM_SIGN_ENABLED",
            "KAM_PRE_RELEASE",
            "KAM_INTERACTIVE",
            "KAM_PROJECT_ROOT",
            "KAM_HOOKS_ROOT",
            "KAM_MODULE_ROOT",
            "KAM_WEB_ROOT",
            "KAM_DIST_DIR",
            "KAM_MODULE_ID",
            "KAM_MODULE_VERSION",
            "KAM_MODULE_VERSION_CODE",
            "KAM_MODULE_NAME",
            "KAM_MODULE_AUTHOR",
            "KAM_MODULE_DESCRIPTION",
            "KAM_MODULE_UPDATE_JSON",
            "KAM_STAGE",
            "KAM_GIT_REPO",
            "KAM_GITHUB_REPO",
            "KAM_REPO",
            "KAM_REPO_REF",
            "KAM_RELEASE_TAG",
            "KAM_PROP_ID",
            "KAM_PROP_NAME",
            "KAM_PROP_VERSION",
            "KAM_PROP_VERSION_CODE",
            "KAM_PROP_AUTHOR",
            "KAM_PROP_DESCRIPTION",
            "KAM_FORCE_INDEX_REFRESH",
            "KAM_REPO_CONCURRENCY",
        ];

        for &v in known_vars.iter() {
            let desc_key = format!("env.docs.{}", v.to_ascii_lowercase());
            let desc = crate::i18n::tr_key(&desc_key);
            let val = std::env::var(v).unwrap_or_else(|_| "<not set>".to_string());
            println!("{:28} {}  ({})", v, desc, val);
        }

        // Note about template variables
        let tmpl_note = crate::i18n::tr_key("env.docs.kam_tmpl_note");
        if !tmpl_note.is_empty() {
            println!();
            println!("{}", tmpl_note);
        }

        return Ok(());
    }

    // Default behavior: list runtime KAM_ environment variables
    let vars = collect_kam_env();

    if vars.is_empty() {
        // Localized message for empty KAM_ environment
        Utils::info(crate::i18n::tr_key("env.no_kam_vars"));
        return Ok(());
    }

    Utils::section("KAM_ environment variables");
    for (k, v) in vars {
        println!("{}={}", k, v);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_from_iter_filters_and_sorts() {
        let input = vec![
            ("KAM_ENV_UNIT_TEST_B".to_string(), "bbb".to_string()),
            ("X_IGNORE".to_string(), "no".to_string()),
            ("KAM_ENV_UNIT_TEST_A".to_string(), "aaa".to_string()),
        ];
        let result = collect_from_iter(input);
        assert_eq!(
            result,
            vec![
                ("KAM_ENV_UNIT_TEST_A".to_string(), "aaa".to_string()),
                ("KAM_ENV_UNIT_TEST_B".to_string(), "bbb".to_string())
            ]
        );
    }

    #[test]
    fn collect_kam_env_delegates_to_iter() {
        // Ensure collect_kam_env() compiles and runs with the real environment iterator.
        // We avoid mutating the process env in tests.
        let _ = collect_kam_env();
    }

    #[test]
    fn run_returns_ok() {
        // run should complete successfully even if there are no KAM_ env vars in the process env.
        assert!(run(EnvArgs {}).is_ok());
    }

    #[test]
    fn run_describe_returns_ok() {
        // `--describe` should complete successfully even without particular env variables set.
        assert!(run(EnvArgs { describe: true }).is_ok());
    }
}
