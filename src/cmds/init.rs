use crate::errors::KamError;

pub mod args;
pub mod impl_mod;
pub mod post_init;
pub mod pre_init;
pub use args::InitArgs;

/// Run the init command
pub fn run(args: InitArgs) -> Result<(), KamError> {
    // Prepare initialization data
    // Make `data` mutable because we'll pass a mutable reference to the template vars later
    let data = pre_init::prepare_init(&args)?;

    // Merge default template variables from `pre_init` with any CLI-provided vars.
    // `pre_init` already parsed CLI `--var` into `template_vars` and merged defaults,
    // so simply convert the resulting `HashMap` into `key=value` strings to feed the
    // `init_template` function which expects `&[String]`.
    let mut merged_var_vec: Vec<String> = data
        .template_vars
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    // Keep ordering deterministic for reproducibility
    merged_var_vec.sort();

    // Initialize using template with merged variables (pass clones to avoid moving `data`)
    impl_mod::init_template(
        &data.path,
        &data.id,
        data.name.clone(),
        &data.version,
        &data.author,
        data.description.clone(),
        &merged_var_vec,
        Some(data.impl_template.clone()),
        args.force,
        data.module_type,
        data.update_json.clone(),
    )?;

    // Post-process
    post_init::post_process(&data.path)?;

    Ok(())
}
