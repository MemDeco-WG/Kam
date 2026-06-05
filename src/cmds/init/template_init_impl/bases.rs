#[derive(Debug, Default, serde::Deserialize)]
struct KamBasesManifest {
    #[serde(default)]
    base: Vec<KamBase>,
}

#[derive(Debug, serde::Deserialize)]
struct KamBase {
    name: Option<String>,
    path: String,
    url: String,
    branch: Option<String>,
    kind: Option<String>,
}

fn restore_kam_bases(project_root: &Path) -> Result<(), KamError> {
    let manifest_path = project_root.join(".kam").join("bases.toml");
    if !manifest_path.exists() {
        return Ok(());
    }

    let manifest = load_bases_manifest(&manifest_path)?;

    if manifest.base.is_empty() {
        return Ok(());
    }

    ensure_git_repo(project_root)?;
    for base in &manifest.base {
        restore_one_base(project_root, base)?;
    }

    run_git(project_root, &["submodule", "update", "--init", "--recursive"])?;
    Ok(())
}

fn managed_base_excludes_from_template(
    template_path: &Path,
    template_vars: &std::collections::HashMap<String, String>,
) -> Result<Vec<String>, KamError> {
    let manifest_path = template_path.join(".kam").join("bases.toml");
    if !manifest_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&manifest_path).map_err(KamError::Io)?;
    let rendered = render_bases_manifest_text(&content, template_vars, &manifest_path)?;
    let manifest: KamBasesManifest = toml::from_str(&rendered).map_err(|e| {
        KamError::Toml(format!("Failed to parse {}: {e}", manifest_path.display()))
    })?;

    Ok(manifest
        .base
        .iter()
        .filter(|base| base.kind.as_deref().unwrap_or("submodule") == "submodule")
        .map(|base| directory_exclude_pattern(&base.path))
        .collect())
}

fn load_bases_manifest(manifest_path: &Path) -> Result<KamBasesManifest, KamError> {
    let content = fs::read_to_string(manifest_path).map_err(KamError::Io)?;
    toml::from_str(&content)
        .map_err(|e| KamError::Toml(format!("Failed to parse {}: {e}", manifest_path.display())))
}

fn render_bases_manifest_text(
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

    let mut context = Context::new();
    for (key, value) in vars {
        insert_nested_context_value(&mut context, key, value);
    }
    Tera::default().render_str(&text, &context).map_err(|e| {
        KamError::TemplateRenderError(format!(
            "Failed to render {}: {e}",
            manifest_path.display()
        ))
    })
}

fn insert_nested_context_value(context: &mut Context, key: &str, value: &str) {
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

fn restore_one_base(project_root: &Path, base: &KamBase) -> Result<(), KamError> {
    if base.kind.as_deref().unwrap_or("submodule") != "submodule" {
        return Err(KamError::InvalidConfig(format!(
            "Unsupported .kam base kind for {}: {}",
            base_label(base),
            base.kind.as_deref().unwrap_or_default()
        )));
    }

    let path = validate_base_path(&base.path)?;
    let target = project_root.join(&path);
    if target.exists() {
        return Ok(());
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }

    let mut args = vec!["submodule", "add"];
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
mod kam_bases_tests {
    use super::{KamBase, validate_base_path};

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

        let excludes = super::managed_base_excludes_from_template(
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
        };
        assert_eq!(super::base_label(&base), "src/example/lib/kamfw");
    }
}
