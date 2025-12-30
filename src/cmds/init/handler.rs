use crate::errors::KamError;

use super::args::InitArgs;
use super::{impl_mod, interactive, post_init, pre_init};

/// 初始化命令的主入口
///
/// # Errors
///
/// Returns `Err(KamError)` if initialization fails (e.g. preparing init data,
/// running template initialization, or post-processing).
pub fn run(args: &InitArgs) -> Result<(), KamError> {
    // 如果是交互模式，直接丢给交互式流程处理
    if args.interactive {
        return interactive::run(args);
    }

    // 准备初始化数据
    // TODO: 这里可能可以优化一下，但先这样吧
    let data = pre_init::prepare_init(args)?;

    // 合并模板变量，把HashMap转成key=value的字符串数组
    // 因为init_template函数需要这种格式（历史遗留，懒得改了）
    let mut merged_var_vec: Vec<String> = data
        .template_vars
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    // 排序一下，保证输出顺序一致（虽然其实没啥用，但看着舒服）
    merged_var_vec.sort();

    // 用模板初始化项目，传clone避免move（Rust的ownership真是...）
    impl_mod::init_template(
        &data.path,
        &impl_mod::InitTemplateParams {
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
