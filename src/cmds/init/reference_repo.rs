use crate::errors::KamError;
use crate::types::kam_toml::sections::repo::{Maintainer, MaintainerEntry};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

use super::pre_init::PreInitData;

fn write_file(path: &Path, content: &str, force: bool) -> Result<(), KamError> {
    if path.exists() && !force {
        return Err(KamError::InvalidConfig(format!(
            "{} already exists. Pass --force to overwrite it.",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(KamError::Io)?;
    }
    fs::write(path, content).map_err(KamError::Io)
}

fn author_entry(data: &PreInitData) -> Option<Value> {
    let author = data.author.trim();
    if author.is_empty() || author == "Your Name" {
        return None;
    }

    let mut obj = Map::new();
    obj.insert("type".to_string(), Value::String("add".to_string()));
    obj.insert("name".to_string(), Value::String(author.to_string()));
    Some(Value::Object(obj))
}

fn maintainer_entry(entry: &MaintainerEntry) -> Value {
    let mut obj = Map::new();
    match entry {
        MaintainerEntry::Name(name) => {
            obj.insert("type".to_string(), Value::String("add".to_string()));
            obj.insert("name".to_string(), Value::String(name.clone()));
        }
        MaintainerEntry::Object(Maintainer { r#type, name, link }) => {
            obj.insert(
                "type".to_string(),
                Value::String(r#type.clone().unwrap_or_else(|| "add".to_string())),
            );
            obj.insert("name".to_string(), Value::String(name.clone()));
            if let Some(link) = link.as_ref().filter(|link| !link.trim().is_empty()) {
                obj.insert("link".to_string(), Value::String(link.clone()));
            }
        }
    }
    Value::Object(obj)
}

fn additional_authors(data: &PreInitData) -> Vec<Value> {
    let mut authors = Vec::new();
    if let Some(author) = author_entry(data) {
        authors.push(author);
    }

    if let Some(repo) = data
        .kam_toml
        .mmrl
        .as_ref()
        .and_then(|mmrl| mmrl.repo.as_ref())
        && let Some(maintainers) = &repo.maintainers
    {
        authors.extend(maintainers.iter().map(maintainer_entry));
    }

    authors
}

fn clean_summary(summary: &str, fallback_name: &str) -> String {
    let trimmed = summary.trim();
    let lower = trimmed.to_ascii_lowercase();
    let invalid = trimmed.is_empty()
        || lower.starts_with("<div")
        || lower.starts_with("</div")
        || lower.starts_with("<p")
        || lower.starts_with("<img")
        || lower.starts_with("<a ")
        || lower.contains("img.shields.io")
        || lower.contains("badge");

    if invalid {
        format!("{fallback_name} KernelSU module")
    } else {
        trimmed.to_string()
    }
}

fn build_module_json(data: &PreInitData) -> Value {
    let mut root = Map::new();
    root.insert(
        "metamodule".to_string(),
        Value::Bool(data.kam_toml.prop.metamodule),
    );
    root.insert(
        "summary".to_string(),
        Value::String(clean_summary(&data.description, &data.name)),
    );
    root.insert(
        "sourceUrl".to_string(),
        data.source_url
            .as_ref()
            .map_or(Value::Null, |url| Value::String(url.clone())),
    );
    root.insert(
        "additionalAuthors".to_string(),
        Value::Array(additional_authors(data)),
    );
    Value::Object(root)
}

fn readme(data: &PreInitData) -> String {
    let source_url = data
        .source_url
        .as_deref()
        .unwrap_or("Set --source-url or edit module.json before publishing.");

    format!(
        r"# {name}

{description}

## KernelSU Modules Repo

This repository is a metadata-only module listing for KernelSU Modules Repo.
The module source code lives at:

{source_url}

## Release Requirements

- The GitHub repository name must match `module.prop` `id`: `{id}`.
- Releases must be immutable, non-draft GitHub Releases.
- Each release must upload a module ZIP asset.
- The ZIP root must contain `module.prop`.
- `module.prop` must contain `id`, `version`, and `versionCode`.
- `versionCode` must increase for newer releases.

## Metadata

- Repository description is used as the module name in the module list.
- `module.json` provides `summary`, `sourceUrl`, `additionalAuthors`, and `metamodule`.
- `README.md` is used as the module detail description.
",
        id = data.id,
        name = data.name,
        description = data.description,
        source_url = source_url
    )
}

/// Initialize a KernelSU Modules Repo metadata-only repository.
///
/// # Errors
/// Returns an error when the destination exists without `--force` or files cannot be written.
pub fn init_reference_repo(data: &PreInitData, force: bool) -> Result<(), KamError> {
    fs::create_dir_all(&data.path).map_err(KamError::Io)?;

    let module_json = serde_json::to_string_pretty(&build_module_json(data))?;
    write_file(&data.path.join("module.json"), &(module_json + "\n"), force)?;
    write_file(&data.path.join("README.md"), &readme(data), force)?;

    crate::utils::Utils::success(format!(
        "Initialized KernelSU reference repository in {}",
        data.path.display()
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::clean_summary;

    #[test]
    fn summary_rejects_html_container_lines() {
        assert_eq!(
            clean_summary(r#"<div align="center">"#, "MagicNet"),
            "MagicNet KernelSU module"
        );
    }

    #[test]
    fn summary_keeps_plain_text() {
        assert_eq!(
            clean_summary("A network module for Android.", "MagicNet"),
            "A network module for Android."
        );
    }
}
