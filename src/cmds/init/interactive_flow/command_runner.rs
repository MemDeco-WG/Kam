fn save_global_config(
    author: Option<&str>,
    name: Option<&str>,
    version: Option<&str>,
) -> Result<(), KamError> {
    // Use `toml` to update or create a global config at Kam's home directory (defaults to ~/.kam/config.toml).
    // The `KAM_HOME` environment variable can override the Kam home directory.
    let config_dir = crate::utils::kam_home_dir()?;
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(KamError::Io)?;
    }
    let cfg_path = config_dir.join("config.toml");

    // Parse existing TOML if present; else start with an empty table
    let mut doc_table: toml::value::Table = if cfg_path.exists() {
        let s = fs::read_to_string(&cfg_path).map_err(KamError::Io)?;
        let val: toml::Value = toml::from_str(&s).map_err(|e| {
            KamError::CommandFailed(format!("Failed to parse existing config: {e}"))
        })?;
        match val {
            toml::Value::Table(t) => t,
            _ => toml::value::Table::new(),
        }
    } else {
        toml::value::Table::new()
    };

    // Ensure we have a [prop] table and update keys using get_mut to avoid Entry type mismatches
    if let Some(val) = doc_table.get_mut("prop") {
        if let toml::Value::Table(prop_table) = val {
            if let Some(a) = author {
                prop_table.insert("author".to_string(), toml::Value::String(a.to_string()));
            }
            if let Some(n) = name {
                prop_table.insert("name".to_string(), toml::Value::String(n.to_string()));
            }
            if let Some(v) = version {
                prop_table.insert("version".to_string(), toml::Value::String(v.to_string()));
            }
        } else {
            // Replace non-table 'prop' value with a proper table containing our values
            let mut prop_tbl = toml::value::Table::new();
            if let Some(a) = author {
                prop_tbl.insert("author".to_string(), toml::Value::String(a.to_string()));
            }
            if let Some(n) = name {
                prop_tbl.insert("name".to_string(), toml::Value::String(n.to_string()));
            }
            if let Some(v) = version {
                prop_tbl.insert("version".to_string(), toml::Value::String(v.to_string()));
            }
            *val = toml::Value::Table(prop_tbl);
        }
    } else {
        let mut prop_tbl = toml::value::Table::new();
        if let Some(a) = author {
            prop_tbl.insert("author".to_string(), toml::Value::String(a.to_string()));
        }
        if let Some(n) = name {
            prop_tbl.insert("name".to_string(), toml::Value::String(n.to_string()));
        }
        if let Some(v) = version {
            prop_tbl.insert("version".to_string(), toml::Value::String(v.to_string()));
        }
        doc_table.insert("prop".to_string(), toml::Value::Table(prop_tbl));
    }

    // Serialize and write back to file
    let final_value = toml::Value::Table(doc_table);
    let out = toml::to_string_pretty(&final_value)
        .map_err(|e| KamError::CommandFailed(format!("Failed to serialize config: {e}")))?;
    fs::write(cfg_path, out).map_err(KamError::Io)?;
    Ok(())
}

/// Run the interactive init flow.
///
/// # Errors
///
/// Returns `Err(KamError)` if initialization fails (I/O, rendering, or user input errors).
#[allow(clippy::too_many_lines)]
pub fn run(args: &InitArgs) -> Result<(), KamError> {
    Utils::banner(crate::i18n::tr("init.interactive.title"));
    Utils::info(crate::i18n::tr("init.interactive.press_enter"));
    println!();

    // Prepare defaults (non-interactive sanity pass)
    let mut data = match pre_init::prepare_init(args) {
        Ok(d) => d,
        Err(e) => {
            Utils::error(format!("Failed to prepare initialization defaults: {e}"));
            return Err(e);
        }
    };

    // Template selection
    let chosen_template = choose_template(&data.impl_template)?;
    data.impl_template = chosen_template;

    // Path selection: allow changing target path
    let cur_path_str = data.path.to_string_lossy().to_string();
    let new_path_str = prompt_input(
        &trf!("init.interactive.path_to_create_project"),
        Some(&cur_path_str),
    )?;
    if new_path_str != cur_path_str {
        // Normalize to an absolute path, respecting '.' and relative paths
        let final_path = if new_path_str.trim().is_empty() || new_path_str == "." {
            std::env::current_dir().map_err(KamError::Io)?
        } else {
            let candidate = PathBuf::from(&new_path_str);
            if candidate.is_relative() {
                std::env::current_dir()
                    .map_err(KamError::Io)?
                    .join(candidate)
            } else {
                candidate
            }
        };
        data.path = final_path;

        // If the user didn't pass --id explicitly, recompute `data.id`
        // to match the final folder basename. If the inferred id contains
        // invalid characters, prompt the user to accept a sanitized suggestion
        // or input a valid custom ID.
        if args.id.is_none()
            && let Some(basename) = data.path.file_name().and_then(|s| s.to_str())
        {
            let candidate = basename.to_string();
            // Check ID validity (alphanumeric, '.', '-', '_')
            if candidate
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
            {
                data.id = candidate;
            } else {
                Utils::warn(&trf!("init.interactive.inferred_id_invalid", candidate));

                // Suggest a sanitized version (replace spaces with underscore)
                let suggested = candidate.replace(' ', "_");
                if suggested
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
                {
                    if prompt_confirm(
                        &trf!("init.interactive.use_sanitized_module_id", suggested),
                        true,
                    )? {
                        data.id = suggested;
                    } else {
                        // Ask the user to input a valid ID
                        let id_input = prompt_input(
                            &trf!("init.interactive.module_id_prompt"),
                            Some(&data.id),
                        )?;
                        data.id = id_input;
                        if !data
                            .id
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
                        {
                            return Err(KamError::InvalidConfig(trf!(
                                "init.interactive.invalid_module_id",
                                data.id
                            )));
                        }
                    }
                } else {
                    // If we couldn't create a valid suggestion, force the user to input
                    let id_input = prompt_input("Module ID", Some(&data.id))?;
                    data.id = id_input;
                    if !data
                        .id
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
                    {
                        return Err(KamError::InvalidConfig(format!(
                            "Invalid module ID '{}': ID must contain only alphanumeric characters, dots, dashes, and underscores",
                            data.id
                        )));
                    }
                }
            }
        }
    }

    // Ensure module id matches path name
    let path_basename = data
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&data.id)
        .to_string();
    if path_basename != data.id {
        Utils::warn(&trf!(
            "init.interactive.module_id_mismatch",
            data.id,
            path_basename
        ));
        if prompt_confirm(&trf!("init.interactive.set_module_id_to_basename"), true)? {
            data.id = path_basename;
        } else {
            // Let user input a custom id
            let id_input =
                prompt_input(&trf!("init.interactive.module_id_prompt"), Some(&data.id))?;
            data.id = id_input;
            // re-validate
            if !data
                .id
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
            {
                return Err(KamError::InvalidConfig(trf!(
                    "init.interactive.invalid_module_id",
                    data.id
                )));
            }
        }
    }

    // Base params (name, version, author, description)
    data.name = prompt_input(&trf!("init.interactive.project_name"), Some(&data.name))?;
    data.version = prompt_input(&trf!("init.interactive.version"), Some(&data.version))?;
    data.author = prompt_input(&trf!("init.interactive.author"), Some(&data.author))?;
    data.description = prompt_input(
        &trf!("init.interactive.description"),
        Some(&data.description),
    )?;

    // Ask to persist defaults to global config (~/.kam/config.toml)
    if prompt_confirm(&trf!("init.interactive.save_base_values"), false)? {
        save_global_config(Some(&data.author), Some(&data.name), Some(&data.version))?;
        Utils::success(&trf!("init.interactive.saved_base_values"));
    }

    // Template variables: find template kam.toml then prompt for variables
    let (kt_path, maybe_tmp) = resolve_template_kam_toml(&data.impl_template)?;
    // `maybe_tmp` kept here to keep tempdir alive while we inspect its files
    if let Some(tmp) = &maybe_tmp {
        Utils::info(&trf!(
            "init.interactive.loaded_template_from_temp",
            tmp.path().display()
        ));
    }

    // Ensure data.template_vars contains default fallbacks from pre_init
    // Now prompt for each variable defined in template
    prompt_template_variables(&kt_path, &mut data.template_vars)?;

    // Optional: visualize the template contents to the user
    let template_dir = kt_path.parent().unwrap_or_else(|| Path::new("."));
    let _ = visualize_template(template_dir, 50);

    // Check for gh and cz presence, ask to install if missing
    // GH
    let gh_present = std::process::Command::new("gh")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !gh_present && prompt_confirm(&trf!("init.interactive.gh_not_found"), true)? {
        Utils::info(&trf!("init.interactive.recommend_github_cli"));
        Utils::info(&trf!("init.interactive.helper_script"));
    }

    // cz (commitizen)
    let cz_present = std::process::Command::new("cz")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !cz_present && prompt_confirm(&trf!("init.interactive.cz_not_found"), false)? {
        Utils::info(&trf!("init.interactive.recommend_cz_install"));
        Utils::info(&trf!("init.interactive.helper_script"));
    }

    // Present a summary and confirm before creating files
    println!();
    Utils::section("init.interactive.summary_title");
    Utils::kv(
        "init.interactive.summary.path",
        data.path.display().to_string(),
    );
    Utils::kv("init.interactive.summary.module_id", &data.id);
    Utils::kv("init.interactive.summary.project_name", &data.name);
    Utils::kv("init.interactive.summary.version", &data.version);
    Utils::kv("init.interactive.summary.author", &data.author);
    Utils::kv("init.interactive.summary.template", &data.impl_template);
    if !data.template_vars.is_empty() {
        println!(
            "  {} {}",
            "•".cyan(),
            crate::i18n::tr("init.interactive.template_variables").bold()
        );
        for (k, v) in &data.template_vars {
            println!("    {} {} = {}", "→".blue().dimmed(), k.bold(), v.dimmed());
        }
    }

    if !prompt_confirm(&trf!("init.interactive.confirm_proceed_create"), true)? {
        Utils::warn(&trf!("init.interactive.aborted"));
        return Err(KamError::CommandFailed(
            "User aborted interactive init".to_string(),
        ));
    }

    // Merge into `Vec<String>` for init_template
    let mut merged_var_vec: Vec<String> = data
        .template_vars
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    merged_var_vec.sort();

    // Call the same init_template as non-interactive run
    template_init::init_template(
        &data.path,
        &template_init::InitTemplateParams {
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
            source_url: data.source_url.clone(),
            metamodule: data.kam_toml.prop.metamodule,
        },
    )?;

    // Post-process
    post_init::post_process(&data.path)?;

    Utils::success(crate::i18n::tr("init.interactive.completed_successfully"));
    println!();
    Utils::info(crate::i18n::tr("init.interactive.next_steps"));
    println!("  {} cd {}", "→".blue(), data.path.display());
    println!("  {} kam build", "→".blue());
    Ok(())
}
