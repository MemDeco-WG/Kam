use crate::errors::KamError;
use std::fs;
use std::path::Path;

#[derive(Debug, Default, serde::Deserialize)]
struct HookBasesManifest {
    #[serde(default)]
    base: Vec<HookBaseEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct HookBaseEntry {
    name: Option<String>,
    path: Option<String>,
    include: Option<Vec<String>>,
}

#[derive(Debug, Default)]
pub(super) struct HookBaseFilter {
    filenames: Option<Vec<String>>,
}

impl HookBaseFilter {
    #[cfg(test)]
    pub(super) fn from_filenames(filenames: Vec<String>) -> Self {
        Self {
            filenames: Some(filenames),
        }
    }

    pub(super) fn from_project(project_root: &Path, stage: &str) -> Result<Self, KamError> {
        let manifest_path = project_root.join(".kam").join("bases.toml");
        if !manifest_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&manifest_path).map_err(KamError::Io)?;
        let manifest: HookBasesManifest = toml::from_str(&content).map_err(|e| {
            KamError::Toml(format!("Failed to parse {}: {e}", manifest_path.display()))
        })?;

        let Some(hooks_base) = manifest
            .base
            .iter()
            .find(|base| is_hooks_base(base.name.as_deref(), base.path.as_deref()))
        else {
            return Ok(Self::default());
        };

        let Some(include) = hooks_base.include.as_ref() else {
            return Ok(Self::default());
        };

        Ok(Self {
            filenames: Some(stage_filenames(stage, include)),
        })
    }

    pub(super) fn allows(&self, file_name: &str) -> bool {
        self.filenames
            .as_ref()
            .is_none_or(|filenames| filenames.iter().any(|allowed| allowed == file_name))
    }
}

fn is_hooks_base(name: Option<&str>, path: Option<&str>) -> bool {
    name == Some("hooks") || path == Some(".kam/bases/hooks")
}

fn stage_filenames(stage: &str, includes: &[String]) -> Vec<String> {
    let stage_prefix = format!("{stage}/");
    includes
        .iter()
        .filter_map(|entry| entry.strip_prefix(&stage_prefix))
        .map(ToString::to_string)
        .collect()
}
