use colored::*;
use std::path::Path;

use crate::errors::KamError;
use crate::types::kam_toml::KamToml;

use super::args::ValidateArgs;

pub fn run(args: ValidateArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);
    let kam_toml_path = project_path.join("kam.toml");

    if !kam_toml_path.exists() {
        use crate::utils::Utils;
        Utils::error(&trf!("kam.toml not found at {}", &kam_toml_path.display()));
        return Ok(());
    }

    use crate::utils::Utils;
    Utils::info(&trf!("Validating {}...", &kam_toml_path.display()));

    let kam_toml = match KamToml::load_from_file(&kam_toml_path) {
        Ok(kt) => kt,
        Err(e) => {
            use crate::utils::Utils;
            Utils::error(&trf!("Failed to parse kam.toml: {}", &e));
            return Ok(());
        }
    };

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // --- [prop] Section ---
    // 检查必需字段和格式
    if kam_toml.prop.id.trim().is_empty() {
        errors.push(crate::i18n::tr_key("validate.prop.id_required").to_string());
    } else if !kam_toml
        .prop
        .id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        // id只能包含字母数字、下划线、横线、点号
        errors.push(crate::i18n::tr_key("validate.prop.id_invalid_characters").to_string());
    }

    if kam_toml.prop.name.trim().is_empty() {
        errors.push(crate::i18n::tr_key("validate.prop.name_required").to_string());
    }

    if kam_toml.prop.version.trim().is_empty() {
        errors.push(crate::i18n::tr_key("validate.prop.version_required").to_string());
    }

    if kam_toml.prop.versionCode <= 0 {
        errors
            .push(crate::i18n::tr_key("validate.prop.version_code_should_be_positive").to_string());
    }

    if kam_toml.prop.description.trim().is_empty() {
        errors.push(crate::i18n::tr_key("validate.prop.description_required").to_string());
    }

    // author是可选的，但建议填写（所以是warning不是error）
    if kam_toml
        .prop
        .author
        .as_ref()
        .map(|a| a.trim().is_empty())
        .unwrap_or(true)
    {
        warnings.push(crate::i18n::tr_key("validate.prop.author_empty_recommended").to_string());
    }

    // --- [mmrl.repo] Section ---
    // mmrl是可选的，有的话才检查
    if let Some(mmrl) = &kam_toml.mmrl {
        if let Some(repo) = &mmrl.repo {
            // 检查推荐字段（license建议填写）
            if repo.license.as_deref().unwrap_or("").is_empty() {
                warnings.push(
                    crate::i18n::tr_key("validate.mmrl.repo.license_recommended").to_string(),
                );
            }

            // 检查文件是否存在（如果配置了但文件不存在就是错误）
            check_file_exists(
                project_path,
                &repo.license_file,
                crate::i18n::tr_key("validate.mmrl.repo.license_file"),
                &mut errors,
            );
            check_file_exists(
                project_path,
                &repo.readme_file,
                crate::i18n::tr_key("validate.mmrl.repo.readme_file"),
                &mut errors,
            );
            check_file_exists(
                project_path,
                &repo.changelog_file,
                crate::i18n::tr_key("validate.mmrl.repo.changelog_file"),
                &mut errors,
            );
        }
    }

    // --- [kam.build] Section ---
    // 检查构建相关配置
    if let Some(build) = &kam_toml.kam.build {
        // 检查源码目录（自定义的或默认的）
        let src_dir = if let Some(custom) = &build.source_dir {
            project_path.join(custom)
        } else {
            project_path.join("src").join(&kam_toml.prop.id) // 默认是 src/<id>
        };

        if !src_dir.exists() {
            // 源码目录不存在只是警告，因为可能还没创建
            warnings.push(trf!(
                "Source directory '{}' does not exist. Build might fail or produce empty module.",
                &src_dir.display()
            ));
        }

        // 检查hooks目录
        if let Some(hooks) = &build.hooks_dir {
            let hooks_path = project_path.join(hooks);
            // 只有用户明确设置了非默认路径但不存在时才警告
            // 默认的"hooks"目录不存在不算问题（可能不需要hooks）
            if !hooks_path.exists() && hooks != "hooks" {
                warnings.push(trf!(
                    "Hooks directory '{}' specified but does not exist",
                    hooks
                ));
            }
        }
    } else {
        // 没有build配置，检查默认源码目录
        let default_src = project_path.join("src").join(&kam_toml.prop.id);
        if !default_src.exists() {
            warnings.push(trf!(
                "Default source directory '{}' does not exist.",
                &default_src.display()
            ));
        }
    }

    // --- 输出结果 ---
    println!();
    if errors.is_empty() && warnings.is_empty() {
        use crate::utils::Utils;
        Utils::success(crate::i18n::tr_key("validate.no_issues_kam_toml_valid"));
        // 完美！没有任何问题
    } else {
        // 有错误或警告，打印出来
        if !errors.is_empty() {
            println!(
                "{}",
                crate::i18n::tr_key("validate.errors.header").red().bold()
            );
            for e in &errors {
                println!("  {} {}", "✗".red().bold(), e.red());
            }
        }
        if !warnings.is_empty() {
            if !errors.is_empty() {
                println!(); // 错误和警告之间空一行
            }
            println!(
                "{}",
                crate::i18n::tr_key("validate.warnings.header")
                    .yellow()
                    .bold()
            );
            for w in &warnings {
                println!("  {} {}", "!".yellow().bold(), w.yellow());
            }
        }

        println!();
        if !errors.is_empty() {
            use crate::utils::Utils;
            Utils::error(crate::i18n::tr_key("validate.failed"));
        } else {
            use crate::utils::Utils;
            Utils::warn(crate::i18n::tr_key("validate.passed_with_warnings"));
            // 只有警告，不算失败，但建议修复
        }
    }

    Ok(())
}

// 检查文件是否存在
// 如果配置了文件路径但文件不存在，就加到错误列表里
fn check_file_exists(base: &Path, file: &Option<String>, name: &str, errors: &mut Vec<String>) {
    if let Some(f) = file {
        if !f.is_empty() && !base.join(f).exists() {
            errors.push(trf!("{} '{}' not found", name, f));
        }
    }
}
