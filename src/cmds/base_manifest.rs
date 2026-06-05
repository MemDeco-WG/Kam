use crate::errors::KamError;
use crate::utils::Utils;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, serde::Deserialize)]
pub struct KamBasesManifest {
    #[serde(default)]
    pub base: Vec<KamBase>,
}

#[derive(Debug, serde::Deserialize)]
pub struct KamBase {
    pub name: Option<String>,
    pub path: String,
    pub url: String,
    pub branch: Option<String>,
    pub kind: Option<String>,
    #[serde(default)]
    pub include: Vec<String>,
    pub subdir: Option<String>,
    pub overlay: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BaseSyncOptions {
    pub dry_run: bool,
    pub check: bool,
    pub update_remote: bool,
}

pub fn restore_project_bases(project_root: &Path) -> Result<(), KamError> {
    sync_project_bases(project_root, BaseSyncOptions::default())
}

pub fn sync_project_bases(project_root: &Path, options: BaseSyncOptions) -> Result<(), KamError> {
    let Some(manifest) = load_project_manifest(project_root)? else {
        return Ok(());
    };

    if manifest.base.is_empty() {
        return Ok(());
    }

    if options.dry_run || options.check {
        for base in &manifest.base {
            Utils::info(format!(
                "Would sync base {} at {} from {}",
                base_label(base),
                base.path,
                base.url
            ));
        }
        return Ok(());
    }

    ensure_git_repo(project_root)?;
    for base in &manifest.base {
        sync_one_base(project_root, base, options.update_remote)?;
    }

    run_git(
        project_root,
        &["submodule", "update", "--init", "--recursive"],
    )?;
    Ok(())
}

pub fn load_project_manifest(project_root: &Path) -> Result<Option<KamBasesManifest>, KamError> {
    let manifest_path = project_root.join(".kam").join("bases.toml");
    if !manifest_path.exists() {
        return Ok(None);
    }
    load_manifest(&manifest_path).map(Some)
}

pub fn load_template_manifest_rendered(
    template_path: &Path,
    template_vars: &std::collections::HashMap<String, String>,
) -> Result<Option<KamBasesManifest>, KamError> {
    let manifest_path = template_path.join(".kam").join("bases.toml");
    if !manifest_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&manifest_path).map_err(KamError::Io)?;
    let rendered = render_manifest_text(&content, template_vars, &manifest_path)?;
    toml::from_str(&rendered)
        .map(Some)
        .map_err(|e| KamError::Toml(format!("Failed to parse {}: {e}", manifest_path.display())))
}

pub fn managed_base_excludes_from_template(
    template_path: &Path,
    template_vars: &std::collections::HashMap<String, String>,
) -> Result<Vec<String>, KamError> {
    let Some(manifest) = load_template_manifest_rendered(template_path, template_vars)? else {
        return Ok(Vec::new());
    };

    Ok(manifest
        .base
        .iter()
        .filter(|base| base.kind.as_deref().unwrap_or("submodule") == "submodule")
        .map(|base| directory_exclude_pattern(&base.path))
        .collect())
}

pub fn materialize_workflow_bases(project_root: &Path, dry_run: bool) -> Result<bool, KamError> {
    let Some(manifest) = load_project_manifest(project_root)? else {
        return Ok(false);
    };
    let Some(base) = manifest
        .base
        .iter()
        .find(|base| base.name.as_deref() == Some("workflows"))
    else {
        return Ok(false);
    };

    let source_root = base_source_root(project_root, base)?;
    let overlay = base
        .overlay
        .as_deref()
        .map(validate_base_path)
        .transpose()?
        .unwrap_or_else(|| PathBuf::from(".github/workflows"));
    let target_root = project_root.join(overlay);
    let includes = workflow_includes(base, &source_root)?;

    for include in includes {
        let relative = validate_base_path(&include)?;
        let source = source_root.join(&relative);
        let target = target_root.join(relative);
        if dry_run {
            Utils::info(format!(
                "Would sync workflow {} -> {}",
                source.display(),
                target.display()
            ));
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(KamError::Io)?;
        }
        fs::copy(&source, &target).map_err(KamError::Io)?;
    }

    Ok(true)
}

fn load_manifest(manifest_path: &Path) -> Result<KamBasesManifest, KamError> {
    let content = fs::read_to_string(manifest_path).map_err(KamError::Io)?;
    toml::from_str(&content)
        .map_err(|e| KamError::Toml(format!("Failed to parse {}: {e}", manifest_path.display())))
}

fn render_manifest_text(
    content: &str,
    vars: &std::collections::HashMap<String, String>,
    manifest_path: &Path,
) -> Result<String, KamError> {
    let mut text = content.to_string();
    for (key, value) in vars {
        text = text.replace(&format!("{{{{{key}}}}}"), value);
    }

    if !text.contains("{{") {
        return Ok(text);
    }

    let mut context = tera::Context::new();
    for (key, value) in vars {
        insert_nested_context_value(&mut context, key, value);
    }
    tera::Tera::default()
        .render_str(&text, &context)
        .map_err(|e| {
            KamError::TemplateRenderError(format!(
                "Failed to render {}: {e}",
                manifest_path.display()
            ))
        })
}

fn insert_nested_context_value(context: &mut tera::Context, key: &str, value: &str) {
    if let Some((root, field)) = key.split_once('.') {
        let mut map = std::collections::BTreeMap::new();
        map.insert(field.to_string(), value.to_string());
        context.insert(root, &map);
    } else {
        context.insert(key, value);
    }
}

fn directory_exclude_pattern(path: &str) -> String {
    if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{path}/")
    }
}

fn ensure_git_repo(project_root: &Path) -> Result<(), KamError> {
    if project_root.join(".git").exists() {
        return Ok(());
    }
    run_git(project_root, &["init"])
}

fn sync_one_base(project_root: &Path, base: &KamBase, update_remote: bool) -> Result<(), KamError> {
    if base.kind.as_deref().unwrap_or("submodule") != "submodule" {
        return Err(KamError::InvalidConfig(format!(
            "Unsupported .kam base kind for {}: {}",
            base_label(base),
            base.kind.as_deref().unwrap_or_default()
        )));
    }

    let path = validate_base_path(&base.path)?;
    let target = project_root.join(&path);
    if !target.exists() {
        add_submodule(project_root, base)?;
    }

    let mut args = vec!["submodule", "update", "--init", "--recursive"];
    if update_remote {
        args.push("--remote");
    }
    args.push(&base.path);
    run_git(project_root, &args)
}

fn add_submodule(project_root: &Path, base: &KamBase) -> Result<(), KamError> {
    let path = validate_base_path(&base.path)?;
    if let Some(parent) = project_root.join(&path).parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }

    let mut args = vec!["submodule", "add", "-f"];
    if let Some(branch) = base.branch.as_deref().filter(|branch| !branch.is_empty()) {
        args.push("-b");
        args.push(branch);
    }
    args.push(&base.url);
    args.push(&base.path);
    run_git(project_root, &args)
}

fn validate_base_path(path: &str) -> Result<PathBuf, KamError> {
    let path_buf = PathBuf::from(path);
    if path_buf.is_absolute()
        || path_buf
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(KamError::InvalidConfig(format!(
            ".kam base path must be relative and stay inside the project: {path}"
        )));
    }
    Ok(path_buf)
}

fn base_source_root(project_root: &Path, base: &KamBase) -> Result<PathBuf, KamError> {
    let mut root = project_root.join(validate_base_path(&base.path)?);
    if let Some(subdir) = &base.subdir {
        root = root.join(validate_base_path(subdir)?);
    }
    Ok(root)
}

fn workflow_includes(base: &KamBase, source_root: &Path) -> Result<Vec<String>, KamError> {
    if !base.include.is_empty() {
        return Ok(base.include.clone());
    }
    let mut includes = Vec::new();
    for entry in fs::read_dir(source_root).map_err(KamError::Io)? {
        let path = entry.map_err(KamError::Io)?.path();
        if path.is_file()
            && let Some(name) = path.file_name().and_then(|value| value.to_str())
        {
            includes.push(name.to_string());
        }
    }
    includes.sort();
    Ok(includes)
}

fn run_git(project_root: &Path, args: &[&str]) -> Result<(), KamError> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(project_root)
        .output()
        .map_err(KamError::Io)?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(KamError::CommandFailed(format!(
        "git {} failed in {}: {}{}",
        args.join(" "),
        project_root.display(),
        stdout.trim(),
        stderr.trim()
    )))
}

fn base_label(base: &KamBase) -> &str {
    base.name.as_deref().unwrap_or(&base.path)
}

#[cfg(test)]
mod tests {
    use super::{KamBase, managed_base_excludes_from_template, validate_base_path};

    #[test]
    fn rejects_parent_base_path() {
        assert!(validate_base_path("../kamfw").is_err());
    }

    #[test]
    fn accepts_nested_base_path() {
        assert!(validate_base_path(".kam/bases/hooks").is_ok());
    }

    #[test]
    fn renders_template_base_excludes() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let manifest_dir = temp_dir.path().join(".kam");
        std::fs::create_dir_all(&manifest_dir).expect("mkdir");
        std::fs::write(
            manifest_dir.join("bases.toml"),
            r#"
[[base]]
name = "kamfw"
path = "src/{{prop.id}}/lib/kamfw"
url = "https://example.invalid/kamfw.git"
"#,
        )
        .expect("write manifest");

        let excludes = managed_base_excludes_from_template(
            temp_dir.path(),
            &std::collections::HashMap::from([(
                "prop.id".to_string(),
                "org.example.module".to_string(),
            )]),
        )
        .expect("excludes");

        assert_eq!(excludes, vec!["src/org.example.module/lib/kamfw/"]);
    }

    #[test]
    fn uses_path_as_base_label_without_name() {
        let base = KamBase {
            name: None,
            path: "src/example/lib/kamfw".to_string(),
            url: "https://example.invalid/kamfw.git".to_string(),
            branch: None,
            kind: None,
            include: Vec::new(),
            subdir: None,
            overlay: None,
        };
        assert_eq!(super::base_label(&base), "src/example/lib/kamfw");
    }
}
