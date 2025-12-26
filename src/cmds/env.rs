//! `kam env` - 输出所有以 `KAM_` 前缀开头的环境变量（便于调试）
//!
//! 设计原则：简单、确定性输出（按键名排序），便于 pipe / grep / CI 查看。
//!
//! 目前实现：
//! - 无需额外参数，直接打印所有 `KAM_` 前缀的环境变量，格式为 `KEY=VALUE`
//! - 提供 `collect_kam_env()` 供测试或其它代码重用
use clap::Args;

use crate::errors::KamError;
use crate::utils::Utils;

/// 参数（当前无任何选项，仅作为子命令占位）
#[derive(Args, Debug)]
pub struct EnvArgs {}

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

/// 执行 `kam env` 命令：打印所有以 `KAM_` 开头的环境变量
pub fn run(_args: EnvArgs) -> Result<(), KamError> {
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
        let result = collect_from_iter(input.into_iter());
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
}
