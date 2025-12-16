use crate::errors::KamError;

use super::args::InitArgs;
use super::{impl_mod, interactive, post_init, pre_init};

// 初始化命令的主入口
pub fn run(args: InitArgs) -> Result<(), KamError> {
    // 如果是交互模式，直接丢给交互式流程处理
    if args.interactive {
        return interactive::run(args);
    }

    // 准备初始化数据
    // TODO: 这里可能可以优化一下，但先这样吧
    let data = pre_init::prepare_init(&args)?;

    // 合并模板变量，把HashMap转成key=value的字符串数组
    // 因为init_template函数需要这种格式（历史遗留，懒得改了）
    let mut merged_var_vec: Vec<String> = data
        .template_vars
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    // 排序一下，保证输出顺序一致（虽然其实没啥用，但看着舒服）
    merged_var_vec.sort();

    // 用模板初始化项目，传clone避免move（Rust的ownership真是...）
    impl_mod::init_template(
        &data.path,
        impl_mod::InitTemplateParams {
            id: &data.id,
            name: data.name.clone(),
            version: &data.version,
            author: &data.author,
            description: data.description.clone(),
            var: &merged_var_vec,
            impl_template: Some(data.impl_template.clone()),
            force: args.force,
            module_type: data.module_type,
            update_json: data.update_json.clone(),
        },
    )?;

    // 后处理，比如生成一些额外的文件啥的
    post_init::post_process(&data.path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;
    use serial_test::serial;
    use tempfile::tempdir;

    #[test]
    #[serial]
    fn test_init_creates_kam_toml_with_defaults() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();

        let args = InitArgs {
            name: Some("my_test_module".to_string()),
            id: None,
            project_name: None,
            version: None,
            author: None,
            update_json: None,
            description: None,
            force: true,
            r#impl: None,
            var: vec![],
            template: Some("tmpl_template".to_string()),
            tmpl: false,
            interactive: false,
        };

        run(args).unwrap();

        // Expect project directory created
        let kt_path = dir.join("my_test_module").join("kam.toml");
        assert!(kt_path.exists());
        let kt = crate::types::kam_toml::KamToml::load_from_file(&kt_path).unwrap();
        assert_eq!(kt.prop.id, "my_test_module");

        std::env::set_current_dir(orig).unwrap();
    }

    #[test]
    #[serial]
    fn test_init_with_custom_id_and_author() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();

        let args = InitArgs {
            name: Some("custom_mod".to_string()),
            id: Some("custom.id".to_string()),
            project_name: Some("Custom Name".to_string()),
            version: Some("0.1.2".to_string()),
            author: Some("LIghtJUNction".to_string()),
            update_json: None,
            description: Some("A custom module".to_string()),
            force: true,
            r#impl: None,
            var: vec![],
            template: Some("tmpl_template".to_string()),
            tmpl: false,
            interactive: false,
        };

        run(args).unwrap();
        let kt_path = dir.join("custom_mod").join("kam.toml");
        assert!(kt_path.exists());
        let kt = crate::types::kam_toml::KamToml::load_from_file(&kt_path).unwrap();
        assert_eq!(kt.prop.id, "custom.id");
        // author现在是Option了，需要匹配Some(...)
        assert_eq!(kt.prop.author, Some("LIghtJUNction".to_string()));
        assert_eq!(kt.prop.version, "0.1.2");

        std::env::set_current_dir(orig).unwrap();
    }

    #[test]
    fn test_init_parses_short_interactive_flag() {
        // Short alias -i
        let cli = Cli::try_parse_from(["kam", "init", "-i", "my_test_module"]).unwrap();
        match cli.command {
            Some(crate::cli::Commands::Init(args)) => assert!(args.interactive),
            _ => panic!("Expected init command"),
        }
    }

    #[test]
    fn test_init_parses_long_interactive_flag() {
        // Long alias --interactive (explicit PATH provided)
        let cli = Cli::try_parse_from(["kam", "init", "--interactive", "my_test_module"]).unwrap();
        match cli.command {
            Some(crate::cli::Commands::Init(args)) => {
                assert!(args.interactive);
                assert_eq!(args.name, Some("my_test_module".to_string()));
            }
            _ => panic!("Expected init command"),
        }
    }

    #[test]
    fn test_init_parses_short_interactive_without_path() {
        // Short alias -i without PATH should be accepted when interactive mode is used
        let cli = Cli::try_parse_from(["kam", "init", "-i"]).unwrap();
        match cli.command {
            Some(crate::cli::Commands::Init(args)) => {
                assert!(args.interactive);
                assert!(args.name.is_none());
            }
            _ => panic!("Expected init command"),
        }
    }

    #[test]
    fn test_init_parses_long_interactive_without_path() {
        // Long alias --interactive without PATH should be accepted when interactive mode is used
        let cli = Cli::try_parse_from(["kam", "init", "--interactive"]).unwrap();
        match cli.command {
            Some(crate::cli::Commands::Init(args)) => {
                assert!(args.interactive);
                assert!(args.name.is_none());
            }
            _ => panic!("Expected init command"),
        }
    }
}
