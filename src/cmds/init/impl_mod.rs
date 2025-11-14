use crate::cmds::init::status::{StatusType, print_status};
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::sections::TmplSection;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use tera::{Context, Tera};
// toml_edit not needed here; use toml::Value for mutation

pub fn init_impl(
    path: &Path,
    id: &str,
    name_map: HashMap<String, String>,
    version: &str,
    author: &str,
    description_map: HashMap<String, String>,
    impl_source: &str,
    template_vars: &mut HashMap<String, String>,
) -> Result<(), KamError> {
    // Parse the template source specification
    let source = crate::types::source::Source::parse(impl_source).map_err(|e| {
        KamError::FetchFailed(format!(
            "Failed to parse template source '{}': {}",
            impl_source, e
        ))
    })?;

    // Create a dummy KamToml for the module (we'll load the real one from the template)
    let dummy_toml = KamToml::new_with_current_timestamp(
        "template".to_string(),
        [("en".to_string(), "Template".to_string())].into(),
        "1.0.0".to_string(),
        "Template Author".to_string(),
        [("en".to_string(), "Template description".to_string())].into(),
        None,
        None,
    );

    // Create KamModule and fetch the template
    let module = crate::types::modules::base::KamModule::new(dummy_toml, Some(source));
    let template_path = module.fetch_to_temp()?;

    // Determine archive_id from the template path
    let archive_id = template_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("template")
        .to_string();

    // Load template variables and insert defaults (refactored to helper to avoid deep nesting)
    let template_kam_path = template_path.join("kam.toml");
    if template_kam_path.exists() {
        fn merge_template_defaults(
            kt_path: &std::path::Path,
            template_vars: &mut HashMap<String, String>,
        ) -> Result<(), KamError> {
            let kt_template = KamToml::load_from_file(kt_path)?;
            if let Some(tmpl) = &kt_template.kam.tmpl {
                for (var_name, var_def) in &tmpl.variables {
                    if template_vars.contains_key(var_name.as_str()) {
                        continue;
                    }

                    if var_def.required {
                        if let Some(default) = &var_def.default {
                            template_vars.insert(var_name.to_string(), default.clone());
                            continue;
                        }
                        if let Some(note) = &var_def.note {
                            return Err(KamError::TemplateVarRequired(format!(
                                "Required template variable '{}' not provided: {}",
                                var_name, note
                            )));
                        }
                        return Err(KamError::TemplateVarRequired(format!(
                            "Required template variable '{}' not provided",
                            var_name
                        )));
                    }

                    if let Some(default) = &var_def.default {
                        template_vars.insert(var_name.to_string(), default.clone());
                    }
                }
            }
            Ok(())
        }

        merge_template_defaults(&template_kam_path, template_vars)?;
    }

    let name_map_btree: BTreeMap<_, _> = name_map.into_iter().collect();
    let description_map_btree: BTreeMap<_, _> = description_map.into_iter().collect();

    let kam_toml_rel = "kam.toml".to_string();
    print_status(StatusType::Add, &kam_toml_rel, false);

    let mut kt = KamToml::new_with_current_timestamp(
        id.to_string(),
        name_map_btree,
        version.to_string(),
        author.to_string(),
        description_map_btree,
        None,
        None,
    );
    kt.kam.tmpl = Some(TmplSection {
        used_template: Some(archive_id.clone()),
        variables: BTreeMap::new(),
    });

    // Apply any template variables that target kam.toml itself. Variables
    // intended to modify kam.toml must start with a leading '#', e.g.
    // `#prop.name.en` or `#prop.version`. These are applied to the KamToml
    // structure via toml_edit so nested fields can be set.
    let mut kam_vars: Vec<(String, String)> = Vec::new();
    let mut normal_vars: Vec<String> = Vec::new();
    for k in template_vars.keys() {
        if k.starts_with('#') {
            kam_vars.push((k.to_string(), template_vars.get(k).unwrap().clone()));
        } else {
            normal_vars.push(k.to_string());
        }
    }
    // Remove kam vars from template_vars so they won't be applied to file contents later
    for k in &kam_vars {
        template_vars.remove(&k.0);
    }

    if !kam_vars.is_empty() {
        kt.apply_vars(kam_vars)?;
    }
    kt.write_to_dir(path)?;

    // Apply template variables to kam.toml as well
    let kam_toml_path = path.join("kam.toml");
    if kam_toml_path.exists() {
        let mut content = std::fs::read_to_string(&kam_toml_path)?;
        let mut context = Context::new();
        for (k, v) in template_vars.iter() {
            context.insert(k, v);
        }
        let mut tera = Tera::default();
        content = tera.render_str(&content, &context).map_err(|e| KamError::TemplateRenderError(e.to_string()))?;
        std::fs::write(&kam_toml_path, content)?;
    }

    // Copy src from template with tera templating.
    if template_path.exists() {
        let src_dir_placeholder = "{{id}}";
        let mut context = Context::new();
        for (k, v) in template_vars.iter() {
            context.insert(k, v);
        }
        let mut tera = Tera::default();
        let src_dir_replaced = tera.render_str(src_dir_placeholder, &context).map_err(|e| KamError::TemplateRenderError(e.to_string()))?;
        let src_temp = template_path.join("src").join(&src_dir_replaced);

        if src_temp.exists() {
            let src_dir = path.join("src").join(id);
            let src_rel = format!("src/{}/", id);
            print_status(StatusType::Add, &src_rel, true);
            std::fs::create_dir_all(&src_dir)?;
            for entry in std::fs::read_dir(&src_temp)? {
                let entry = entry?;
                let filename = entry.file_name();
                let file_name_str = filename.to_string_lossy().to_string();
                let replaced_name = tera.render_str(&file_name_str, &context).map_err(|e| KamError::TemplateRenderError(e.to_string()))?;
                let mut content = std::fs::read_to_string(entry.path())?;
                content = tera.render_str(&content, &context).map_err(|e| KamError::TemplateRenderError(e.to_string()))?;
                let dest_file = src_dir.join(&replaced_name);
                let file_rel = format!("src/{}/{}", id, replaced_name);
                print_status(StatusType::Add, &file_rel, false);
                std::fs::write(&dest_file, content)?;
            }
        } else {
            return Err(KamError::TemplateNotFound(
                "Template source directory not found".to_string(),
            ));
        }
    } else {
        return Err(KamError::TemplateNotFound("Template not found".to_string()));
    }

    Ok(())
}
