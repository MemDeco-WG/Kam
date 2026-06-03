use super::args::BuildArgs;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) struct HookEnv {
    pub(super) hooks_dir: PathBuf,
    pub(super) vars: Vec<(String, String)>,
}

pub(super) fn build_hook_env(
    project_root: &Path,
    kam_toml: &KamToml,
    output_dir: &Path,
    stage: &str,
    args: &BuildArgs,
) -> HookEnv {
    let parsed_env = parse_env_file(project_root);
    let hooks_dir_name = kam_toml
        .kam
        .build
        .as_ref()
        .and_then(|b| b.hooks_dir.as_ref())
        .map_or("hooks", |s| s.as_str());
    let hooks_root = project_root.join(hooks_dir_name);
    let hooks_dir = hooks_root.join(stage);
    let module_root = module_root(project_root, kam_toml);
    let web_root = module_root.join("webroot");
    let (detected_repo, detected_ref) = detect_repo(project_root, kam_toml, &parsed_env);

    let mut vars = Vec::new();
    let mut keys = HashSet::new();
    let mut add_env = |k: &str, value: String| {
        if keys.insert(k.to_string()) {
            vars.push((k.to_string(), value));
        }
    };

    for (k, v) in &parsed_env {
        add_env(k, v.clone());
    }
    add_env(
        "KAM_PROJECT_ROOT",
        project_root.to_string_lossy().to_string(),
    );
    add_env("KAM_HOOKS_ROOT", hooks_root.to_string_lossy().to_string());
    add_env("KAM_MODULE_ROOT", module_root.to_string_lossy().to_string());
    add_env("KAM_WEB_ROOT", web_root.to_string_lossy().to_string());
    add_env("KAM_DIST_DIR", output_dir.to_string_lossy().to_string());
    if stage.starts_with("dev-") {
        add_env(
            "KAM_DEV_SESSION_LOG",
            project_root
                .join(".kam")
                .join("dev")
                .join("last-session.log")
                .to_string_lossy()
                .to_string(),
        );
    }
    add_module_env(kam_toml, &mut add_env);
    add_build_flag_env(args, &mut add_env);
    add_env("KAM_STAGE", stage.to_string());
    add_env(
        "KAM_GIT_REPO",
        kam_toml
            .mmrl
            .as_ref()
            .and_then(|m| m.repo.as_ref())
            .and_then(|r| r.repository.as_ref())
            .cloned()
            .unwrap_or_default(),
    );
    add_env("KAM_GITHUB_REPO", detected_repo.clone());
    add_env("KAM_REPO", detected_repo);
    add_env("KAM_REPO_REF", detected_ref);
    add_env("KAM_RELEASE_TAG", kam_toml.prop.version.clone());
    add_prop_env(kam_toml, &mut add_env);
    add_tmpl_env(kam_toml, &mut add_env);
    add_flattened_kam_env(kam_toml, &mut add_env);

    HookEnv { hooks_dir, vars }
}

fn parse_env_file(project_root: &Path) -> HashMap<String, String> {
    let env_path = project_root.join(".env");
    let mut parsed_env = HashMap::new();
    let Ok(content) = fs::read_to_string(&env_path) else {
        return parsed_env;
    };

    for (line_num, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").map_or(line, str::trim);
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if key.is_empty() || !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                Utils::warn(&trf!(
                    "hooks.invalid_env_variable_name",
                    key,
                    line_num + 1,
                    env_path.display()
                ));
                continue;
            }
            parsed_env.insert(key.to_string(), unquote_env_value(value.trim()).to_string());
        } else {
            Utils::warn(&trf!(
                "hooks.malformed_env_line",
                line_num + 1,
                env_path.display(),
                line
            ));
        }
    }
    parsed_env
}

fn unquote_env_value(value: &str) -> &str {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn module_root(project_root: &Path, kam_toml: &KamToml) -> PathBuf {
    kam_toml.kam.build.as_ref().map_or_else(
        || project_root.join("src").join(&kam_toml.prop.id),
        |build| {
            build.source_dir.as_ref().map_or_else(
                || project_root.join("src").join(&kam_toml.prop.id),
                |custom_src| project_root.join(custom_src),
            )
        },
    )
}

fn detect_repo(
    project_root: &Path,
    kam_toml: &KamToml,
    parsed_env: &HashMap<String, String>,
) -> (String, String) {
    let repo = parsed_env
        .get("GITHUB_REPOSITORY")
        .cloned()
        .or_else(|| std::env::var("GITHUB_REPOSITORY").ok())
        .or_else(|| {
            kam_toml
                .mmrl
                .as_ref()
                .and_then(|m| m.repo.as_ref())
                .and_then(|r| r.repository.as_ref())
                .filter(|repo| !repo.is_empty())
                .cloned()
        })
        .unwrap_or_default();

    let git_ref = parsed_env
        .get("GITHUB_REF")
        .cloned()
        .or_else(|| std::env::var("GITHUB_REF").ok())
        .map_or_else(
            || git_branch(project_root).unwrap_or_default(),
            |value| {
                value
                    .strip_prefix("refs/heads/")
                    .unwrap_or(&value)
                    .to_string()
            },
        );

    (repo, git_ref)
}

fn git_branch(project_root: &Path) -> Result<String, KamError> {
    let out = Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(project_root)
        .output()
        .map_err(KamError::Io)?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Ok(String::new())
    }
}

fn add_module_env(kam_toml: &KamToml, add_env: &mut impl FnMut(&str, String)) {
    add_env("KAM_MODULE_ID", kam_toml.prop.id.clone());
    add_env("KAM_MODULE_VERSION", kam_toml.prop.version.clone());
    add_env(
        "KAM_MODULE_VERSION_CODE",
        kam_toml.prop.versionCode.to_string(),
    );
    add_env("KAM_MODULE_NAME", kam_toml.prop.get_name().to_string());
    add_env(
        "KAM_MODULE_AUTHOR",
        kam_toml.prop.author.clone().unwrap_or_default(),
    );
    add_env(
        "KAM_MODULE_DESCRIPTION",
        kam_toml.prop.get_description().to_string(),
    );
    add_env(
        "KAM_MODULE_UPDATE_JSON",
        kam_toml.prop.updateJson.clone().unwrap_or_default(),
    );
}

fn add_build_flag_env(args: &BuildArgs, add_env: &mut impl FnMut(&str, String)) {
    add_env("KAM_BUMP_ENABLED", flag(args.bump));
    add_env("KAM_RELEASE_ENABLED", flag(args.release));
    add_env("KAM_SIGN_ENABLED", flag(args.sign));
    add_env("KAM_PRE_RELEASE", flag(args.pre_release));
    add_env("KAM_INTERACTIVE", flag(args.interactive));
}

fn add_prop_env(kam_toml: &KamToml, add_env: &mut impl FnMut(&str, String)) {
    add_env("KAM_PROP_ID", kam_toml.prop.id.clone());
    add_env("KAM_PROP_NAME", kam_toml.prop.get_name().to_string());
    add_env("KAM_PROP_VERSION", kam_toml.prop.version.clone());
    add_env(
        "KAM_PROP_VERSION_CODE",
        kam_toml.prop.versionCode.to_string(),
    );
    add_env(
        "KAM_PROP_AUTHOR",
        kam_toml.prop.author.clone().unwrap_or_default(),
    );
    add_env(
        "KAM_PROP_DESCRIPTION",
        kam_toml.prop.get_description().to_string(),
    );
}

fn add_tmpl_env(kam_toml: &KamToml, add_env: &mut impl FnMut(&str, String)) {
    if let Some(tmpl_section) = &kam_toml.kam.tmpl {
        for (var_name, var_def) in &tmpl_section.variables {
            let env_key = format!(
                "KAM_TMPL_{}",
                var_name.to_ascii_uppercase().replace(['.', '-'], "_")
            );
            add_env(&env_key, var_def.default.clone().unwrap_or_default());
        }
    }
}

fn add_flattened_kam_env(kam_toml: &KamToml, add_env: &mut impl FnMut(&str, String)) {
    let kt_vars = crate::template::TemplateVariableProcessor::flatten_kam_toml(kam_toml);
    for (k, v) in kt_vars {
        let env_key_base = k.to_ascii_uppercase().replace(['.', '-'], "_");
        add_env(&format!("KAM_{env_key_base}"), v);
    }
}

fn flag(value: bool) -> String {
    if value {
        "1".to_string()
    } else {
        "0".to_string()
    }
}
