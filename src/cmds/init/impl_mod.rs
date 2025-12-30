use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;
use crate::types::kam_toml::sections::TmplSection;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use tera::{Context, Tera};

// 解压压缩包的辅助函数
// 支持tar.gz和zip，根据扩展名判断（简单粗暴）
fn extract_archive(path: &Path, dst: &Path) -> Result<(), KamError> {
    let file = fs::File::open(path).map_err(KamError::Io)?;
    // 转小写再判断，避免大小写问题
    let path_str = path.to_string_lossy().to_lowercase();
    if path_str.ends_with(".tar.gz") || path_str.ends_with(".tgz") {
        let tar = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(dst).map_err(KamError::Io)?;
    } else if path_str.ends_with(".zip") {
        let mut archive = zip::ZipArchive::new(file).map_err(KamError::Zip)?;
        archive.extract(dst).map_err(KamError::Zip)?;
    } else {
        return Err(KamError::UnsupportedArchive(format!(
            "Unsupported archive: {}",
            path.display()
        )));
    }
    Ok(())
}

fn merge_template_defaults(
    kt_path: &Path,
    template_vars: &mut HashMap<String, String>,
) -> Result<(), KamError> {
    if !kt_path.exists() {
        return Ok(());
    }
    let kt = KamToml::load_from_file(kt_path)?;
    if let Some(tmpl) = kt.tmpl {
        for (k, v) in tmpl.variables {
            if let Some(default_val) = v.default {
                template_vars.entry(k).or_insert(default_val);
            }
        }
    }
    Ok(())
}

pub struct InitImplParams<'a> {
    pub id: &'a str,
    pub name: String,
    pub version: &'a str,
    pub author: &'a str,
    pub description: String,
    pub impl_source: &'a Path,
    pub template_vars: &'a mut HashMap<String, String>,
    pub force: bool,
    pub explicit_template_id: Option<&'a str>,
}

// Parameters for init_template grouped into a struct to satisfy Clippy's too_many_arguments lint.
pub struct InitTemplateParams<'a> {
    pub id: &'a str,
    pub name: String,
    pub version: &'a str,
    pub author: &'a str,
    pub description: String,
    pub var: &'a [String],
    pub impl_template: Option<String>,
    pub force: bool,
    pub module_type: ModuleType,
    pub update_json: Option<String>,
}

pub fn init_impl(path: &Path, params: InitImplParams<'_>) -> Result<(), KamError> {
    // 如果需要解压，先准备个临时目录
    let temp_dir = tempfile::tempdir().map_err(KamError::Io)?;
    let mut template_path = params.impl_source.to_path_buf();

    if template_path.exists() {
        if template_path.is_file() {
            // 是文件就解压
            let extract_dst = temp_dir.path().join("extracted");
            fs::create_dir_all(&extract_dst).map_err(KamError::Io)?;
            extract_archive(&template_path, &extract_dst)?;
            template_path = extract_dst;
        }
    } else {
        return Err(KamError::InvalidDirectory(format!(
            "Template source not found: {}",
            params.impl_source.display()
        )));
    }

    // 如果解压后只有一个目录，就进入那个目录
    // 这样用户打包时不用考虑目录层级问题
    if template_path.is_dir() {
        let entries: Vec<_> = fs::read_dir(&template_path)
            .map_err(KamError::Io)?
            .filter_map(|e| e.ok())
            .collect();

        // 只有一个目录就进去，避免多一层嵌套
        if entries.len() == 1 && entries[0].path().is_dir() {
            template_path = entries[0].path();
        }
    }

    // 确定模板ID，如果用户没指定就用默认的"template"
    let archive_id = params
        .explicit_template_id
        .map_or_else(|| "template".to_string(), |eid| eid.to_string());

    // Load template variables from template's kam.toml
    let template_kam_path = template_path.join("kam.toml");
    if template_kam_path.exists() {
        merge_template_defaults(&template_kam_path, params.template_vars)?;
    }

    // Prepare KamToml
    let mut kt = KamToml::new_with_current_timestamp(
        params.id.to_string(),
        params.name.clone(),
        params.version.to_string(),
        Some(params.author.to_string()),
        params.description.clone(),
        None,
        None,
    );
    kt.kam.tmpl = Some(TmplSection {
        used_template: Some(archive_id.clone()),
        variables: BTreeMap::new(),
    });

    // Extract # vars
    let mut kam_vars: Vec<(String, String)> = Vec::new();
    let keys: Vec<String> = params.template_vars.keys().cloned().collect();
    for k in keys {
        if k.starts_with('#')
            && let Some(v) = params.template_vars.get(&k)
        {
            kam_vars.push((k.clone(), v.clone()));
        }
    }
    for (k, _) in &kam_vars {
        params.template_vars.remove(k);
    }

    if !kam_vars.is_empty() {
        kt.apply_vars(kam_vars)?;
    }

    // Ensure dest dir
    if !path.exists() {
        fs::create_dir_all(path).map_err(KamError::Io)?;
    }

    // Merge kt vars into template_vars
    let kt_vars = crate::template::TemplateVariableProcessor::flatten_kam_toml(&kt);
    for (k, v) in kt_vars {
        params.template_vars.entry(k).or_insert(v);
    }

    // Ensure shallow keys
    params
        .template_vars
        .entry("id".to_string())
        .or_insert_with(|| kt.prop.id.clone());
    params
        .template_vars
        .entry("version".to_string())
        .or_insert_with(|| kt.prop.version.clone());
    params
        .template_vars
        .entry("versionCode".to_string())
        .or_insert_with(|| kt.prop.versionCode.to_string());
    // author现在是Option，需要处理None的情况
    params
        .template_vars
        .entry("author".to_string())
        .or_insert_with(|| kt.prop.author.as_ref().unwrap_or(&String::new()).clone());

    params
        .template_vars
        .entry("name".to_string())
        .or_insert_with(|| kt.prop.name.clone());

    params
        .template_vars
        .entry("description".to_string())
        .or_insert_with(|| kt.prop.description.clone());

    // Check if kam.toml exists BEFORE copy_and_replace
    let kam_toml_path = path.join("kam.toml");
    let kam_toml_existed_before_copy = kam_toml_path.exists();

    // Copy files (respect build.include/build.exclude if present)
    let excludes = kt.kam.build.as_ref().and_then(|b| b.exclude.clone());
    let includes = kt.kam.build.as_ref().and_then(|b| b.include.clone());

    crate::template::TemplateManager::copy_and_replace_with_rules(
        &template_path,
        path,
        params.template_vars,
        params.force,
        &archive_id,
        excludes,
        includes,
    )?;

    // If kam.toml doesn't exist (not in template), write the generated one
    if !kam_toml_path.exists() {
        kt.write_to_dir(path)?;
    }

    // Render kam.toml in place if it exists
    // Only process if force is set OR kam.toml was just created (didn't exist before copy_and_replace)
    if kam_toml_path.exists() && (params.force || !kam_toml_existed_before_copy) {
        let content = fs::read_to_string(&kam_toml_path).map_err(KamError::Io)?;
        let mut context = Context::new();
        for (k, v) in params.template_vars.iter() {
            context.insert(k, v);
        }
        let mut tera = Tera::default();
        match tera.render_str(&content, &context) {
            Ok(rendered) => {
                // Parse rendered kam.toml and fix template-specific values
                match toml::from_str::<KamToml>(&rendered) {
                    Ok(mut rendered_kt) => {
                        // Override with user-provided values
                        rendered_kt.prop.id = params.id.to_string();
                        // 更新渲染后的kam.toml，用实际的值替换模板变量
                        rendered_kt.prop.name = params.name.clone();
                        rendered_kt.prop.version = params.version.to_string();
                        rendered_kt.prop.author = Some(params.author.to_string()); // author现在是Option了
                        rendered_kt.prop.description = params.description.clone();
                        rendered_kt.prop.versionCode = kt.prop.versionCode;

                        // Ensure build section exists and populate missing fields with sensible defaults.
                        // Use BuildSection::default() as authoritative defaults so the generated kam.toml
                        // includes explicit, reasonable defaults for all build-related fields.
                        let default_build =
                            crate::types::kam_toml::sections::BuildSection::default();
                        let build = rendered_kt
                            .kam
                            .build
                            .get_or_insert_with(|| default_build.clone());

                        // Apply defaults for common fields when they are not present.
                        if build.target_dir.is_none() {
                            build.target_dir = Some("dist".to_string());
                        }
                        if build.output_file.is_none() {
                            build.output_file = Some("{{id}}".to_string());
                        }
                        if build.hooks_dir.is_none() {
                            build.hooks_dir = Some("hooks".to_string());
                        }
                        if build.exclude.is_none() {
                            build.exclude = default_build.exclude.clone();
                        }
                        if build.include.is_none() {
                            build.include = default_build.include.clone();
                        }
                        if build.respect_gitignore.is_none() {
                            build.respect_gitignore = default_build.respect_gitignore;
                        }

                        // Ensure build.source_dir uses the new module id if missing
                        if build.source_dir.is_none() {
                            build.source_dir = Some(format!("src/{}", rendered_kt.prop.id));
                        }

                        // Reset module_type to Kam (template's kam.toml has module_type = "template")
                        rendered_kt.kam.module_type = ModuleType::Kam;

                        // Reset workspace members (template workspace is not relevant)
                        if let Some(ref mut workspace) = rendered_kt.kam.workspace {
                            workspace.members = Some(vec![".".to_string()]);
                        }

                        // Write the fixed kam.toml
                        let fixed_content = toml::to_string_pretty(&rendered_kt)
                            .unwrap_or_else(|_| rendered.clone());
                        fs::write(&kam_toml_path, fixed_content).map_err(KamError::Io)?;

                        // Replace current in-memory kt with the rendered/finalized kam.toml to ensure
                        // subsequent logic (e.g. env file writing) reflects the final state
                        kt = rendered_kt;
                    }
                    Err(_) => {
                        // Fallback: just write the rendered content
                        fs::write(&kam_toml_path, rendered).map_err(KamError::Io)?;
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to render kam.toml template: {}", e);
            }
        }
    }

    // 把模板变量写到.kam/template-vars.env.init文件里
    // 这样hooks脚本就可以source这个文件来获取变量了
    // 变量名会转成KAM_XXX的格式，点号和横线都变成下划线
    let env_dir = path.join(".kam");
    fs::create_dir_all(&env_dir).map_err(KamError::Io)?;
    let env_file_path = env_dir.join("template-vars.env.init");
    let mut env_lines: Vec<String> = Vec::new();

    // 先写一些基本的项目信息
    env_lines.push(format!("KAM_PROJECT_ROOT=\"{}\"", path.to_string_lossy()));
    env_lines.push(format!("KAM_MODULE_ID=\"{}\"", kt.prop.id));
    env_lines.push(format!("KAM_MODULE_VERSION=\"{}\"", kt.prop.version));
    env_lines.push(format!(
        "KAM_MODULE_VERSION_CODE=\"{}\"",
        kt.prop.versionCode
    ));
    env_lines.push(format!("KAM_MODULE_NAME=\"{}\"", kt.prop.get_name()));
    // author可能是None，用空字符串作为默认值
    env_lines.push(format!(
        "KAM_MODULE_AUTHOR=\"{}\"",
        kt.prop.author.as_ref().unwrap_or(&String::new())
    ));
    env_lines.push(format!(
        "KAM_MODULE_DESCRIPTION=\"{}\"",
        kt.prop.get_description()
    ));

    // 把kam.toml的所有键值对扁平化后也加进去
    // 这样hooks就能访问到kam.toml里的所有配置了
    let kt_flatvars = crate::template::TemplateVariableProcessor::flatten_kam_toml(&kt);
    for (k, v) in kt_flatvars.iter() {
        let base = k.to_ascii_uppercase().replace(['.', '-'], "_");
        let key = format!("KAM_{}", base);
        let v_escaped = v.replace('"', "\\\""); // 转义引号
        env_lines.push(format!("{}=\"{}\"", key, v_escaped));
    }

    // 再加上模板定义的变量，用KAM_TMPL_前缀
    // 优先用用户提供的值，没有就用默认值
    if let Some(tmpl_section) = &kt.kam.tmpl {
        for (var_name, var_def) in tmpl_section.variables.iter() {
            let nm = var_name.to_ascii_uppercase().replace(['.', '-'], "_");
            let key = format!("KAM_TMPL_{}", nm);
            let val = params
                .template_vars
                .get(var_name)
                .cloned()
                .or_else(|| var_def.default.clone())
                .unwrap_or_default();
            let val_escaped = val.replace('"', "\\\"");
            env_lines.push(format!("{}=\"{}\"", key, val_escaped));
        }
    }

    // 写文件，覆盖已存在的（如果有的话）
    // 这个文件主要是给hooks用的，所以格式要严格一点
    fs::write(&env_file_path, env_lines.join("\n")).map_err(KamError::Io)?;

    Ok(())
    // 终于写完了，这个函数有点长，但暂时不想拆分
}

/* legacy wrapper removed - use `InitTemplateParams`-based signature:
`init_template(path, InitTemplateParams { ... })` */

pub fn init_template(path: &Path, params: InitTemplateParams<'_>) -> Result<(), KamError> {
    let mut template_vars = crate::template::TemplateManager::parse_template_vars(params.var)?;

    let template_spec = params
        .impl_template
        .as_deref()
        .unwrap_or("kam_template")
        .to_string();

    // Strategy:
    // 1. Check if `template_spec` exists as a local file or directory (relative to CWD).
    // 2. Check if `template_spec` exists in cache or built-in assets.
    // 3. Check if `template_spec` exists in project-local `tmpl/` or `templates/` dirs.
    // 4. If NOT found and `template_spec` does NOT end with `_template` or look like a file archive,
    //    append `_template` and retry steps 2-3 (but not step 1 again, usually).

    let mut potential_names = vec![template_spec.clone()];

    // If it doesn't have an extension and doesn't end in _template, try appending it as a fallback
    let is_archive_or_path = template_spec.contains('/')
        || template_spec.contains('\\')
        || template_spec.ends_with(".tar.gz")
        || template_spec.ends_with(".zip");

    if !template_spec.ends_with("_template") && !is_archive_or_path {
        potential_names.push(format!("{}_template", template_spec));
    }

    for spec in potential_names.iter() {
        // 1. Direct local path (only for the first/raw spec, or if we really want to support 'foo_template' local dir)
        let spec_path = Path::new(spec);
        if spec_path.exists() {
            return init_impl(
                path,
                InitImplParams {
                    id: params.id,
                    name: params.name.clone(),
                    version: params.version,
                    author: params.author,
                    description: params.description.clone(),
                    impl_source: spec_path,
                    template_vars: &mut template_vars,
                    force: params.force,
                    explicit_template_id: Some(spec.as_str()),
                },
            );
        }

        // 2. Built-in assets
        // We typically store built-ins as "kam_template.tar.gz" -> check spec
        // If the spec is "kam", we might be in the second iteration where spec="kam_template"
        let asset_name = format!("{}.tar.gz", spec);
        if let Some(asset) = crate::assets::tmpl::TmplAssets::get(&asset_name) {
            let temp_dir = tempfile::tempdir().map_err(KamError::Io)?;
            let temp_path = temp_dir.path().join(format!("{}.tar.gz", spec));
            fs::write(&temp_path, &asset.data).map_err(KamError::Io)?;

            return init_impl(
                path,
                InitImplParams {
                    id: params.id,
                    name: params.name.clone(),
                    version: params.version,
                    author: params.author,
                    description: params.description.clone(),
                    impl_source: &temp_path,
                    template_vars: &mut template_vars,
                    force: params.force,
                    explicit_template_id: Some(spec.as_str()),
                },
            );
        }

        // 3. Cache
        if let Ok(Some(cached_path)) =
            crate::template::TemplateCacheManager::resolve_template_path(spec)
        {
            return init_impl(
                path,
                InitImplParams {
                    id: params.id,
                    name: params.name.clone(),
                    version: params.version,
                    author: params.author,
                    description: params.description.clone(),
                    impl_source: &cached_path,
                    template_vars: &mut template_vars,
                    force: params.force,
                    explicit_template_id: Some(spec.as_str()),
                },
            );
        }

        // 4. Project-local folder search (tmpl/ or templates/)
        let project_local_dirs: &[&str; 2] = crate::utils::PROJECT_TEMPLATE_DIRS;
        let archive_exts: &[&str; 4] = crate::utils::DEFAULT_ARCHIVE_EXTS;
        let mut candidates: Vec<PathBuf> = Vec::new();

        for d in project_local_dirs {
            let base = Path::new(d);
            candidates.push(base.join(spec));
            for ext in archive_exts {
                candidates.push(base.join(format!("{}{}", spec, ext)));
            }
        }
        // Also check project root for archives
        for ext in archive_exts {
            candidates.push(Path::new(&format!("{}{}", spec, ext)).to_path_buf());
        }

        for candidate in candidates {
            if candidate.exists() {
                return init_impl(
                    path,
                    InitImplParams {
                        id: params.id,
                        name: params.name.clone(),
                        version: params.version,
                        author: params.author,
                        description: params.description.clone(),
                        impl_source: &candidate,
                        template_vars: &mut template_vars,
                        force: params.force,
                        explicit_template_id: Some(spec.as_str()),
                    },
                );
            }
        }
    }

    // Final failure
    Err(KamError::TemplateNotFound(format!(
        "Template '{}' not found in built-in assets, local path, cache, or project directories.",
        template_spec
    )))
}
