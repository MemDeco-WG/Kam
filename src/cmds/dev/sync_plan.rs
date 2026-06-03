use glob::Pattern;
use std::path::{Path, PathBuf};

use crate::errors::KamError;
use crate::types::kam_toml::sections::DevSyncSection;

use super::context::DevContext;

#[derive(Debug, Clone)]
pub(super) struct SyncPolicy {
    pub(super) stage_dir: String,
    mirror: Vec<String>,
    preserve: Vec<String>,
    ignore: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SyncMode {
    Mirror,
    Overlay,
}

impl SyncPolicy {
    pub(super) fn from_section(section: &DevSyncSection) -> Self {
        Self {
            stage_dir: section
                .stage_dir
                .clone()
                .unwrap_or_else(|| "/data/local/tmp/kam-dev/{{id}}".to_string()),
            mirror: section.mirror.clone().unwrap_or_default(),
            preserve: section.preserve.clone().unwrap_or_default(),
            ignore: section.ignore.clone().unwrap_or_default(),
        }
    }

    pub(super) fn rendered_stage_dir(&self, module_id: &str) -> String {
        self.stage_dir.replace("{{id}}", module_id)
    }
}

pub(super) fn sync_mode(ctx: &DevContext, file: &Path) -> Result<Option<SyncMode>, KamError> {
    let rel = module_relative(ctx, file)?;
    sync_mode_for_rel(&ctx.sync_policy, &rel)
}

fn sync_mode_for_rel(policy: &SyncPolicy, rel: &str) -> Result<Option<SyncMode>, KamError> {
    if matches_any(&policy.ignore, rel)? || matches_any(&policy.preserve, rel)? {
        return Ok(None);
    }
    if matches_any(&policy.mirror, rel)? {
        return Ok(Some(SyncMode::Mirror));
    }
    Ok(Some(SyncMode::Overlay))
}

pub(super) fn mirror_roots_for_patterns(ctx: &DevContext, patterns: &[&str]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for pattern in &ctx.sync_policy.mirror {
        if !patterns
            .iter()
            .any(|requested| pattern_matches_requested(pattern, requested))
        {
            continue;
        }
        let Some(root) = mirror_root(pattern) else {
            continue;
        };
        let local = ctx.module_root.join(&root);
        if local.exists() {
            roots.push(local);
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

pub(super) fn all_mirror_roots(ctx: &DevContext) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for pattern in &ctx.sync_policy.mirror {
        let Some(root) = mirror_root(pattern) else {
            continue;
        };
        let local = ctx.module_root.join(root);
        if local.exists() {
            roots.push(local);
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

pub(super) fn is_under_mirror_root(ctx: &DevContext, file: &Path) -> Result<bool, KamError> {
    let rel = module_relative(ctx, file)?;
    for pattern in &ctx.sync_policy.mirror {
        let Some(root) = mirror_root(pattern) else {
            continue;
        };
        if rel == root || rel.starts_with(&format!("{root}/")) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn module_relative(ctx: &DevContext, file: &Path) -> Result<String, KamError> {
    let rel = file.strip_prefix(&ctx.module_root).map_err(|_| {
        KamError::InvalidDirectory(format!(
            "{} is outside {}",
            file.display(),
            ctx.module_root.display()
        ))
    })?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn matches_any(patterns: &[String], rel: &str) -> Result<bool, KamError> {
    for pattern in patterns {
        let compiled = Pattern::new(pattern).map_err(|err| {
            KamError::CommandFailed(format!("Invalid dev sync pattern '{pattern}': {err}"))
        })?;
        if compiled.matches(rel) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn mirror_root(pattern: &str) -> Option<String> {
    let root = pattern
        .split(['*', '?', '['])
        .next()
        .unwrap_or_default()
        .trim_end_matches('/');
    if root.is_empty() {
        None
    } else {
        Some(root.to_string())
    }
}

fn pattern_matches_requested(mirror: &str, requested: &str) -> bool {
    mirror == requested
        || mirror.starts_with(requested.trim_end_matches("**").trim_end_matches('/'))
        || requested.starts_with(mirror.trim_end_matches("**").trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::kam_toml::sections::DevSyncSection;

    fn default_policy() -> SyncPolicy {
        SyncPolicy::from_section(&DevSyncSection::default())
    }

    #[test]
    fn default_sync_policy_mirrors_webroot() {
        assert_eq!(
            sync_mode_for_rel(&default_policy(), "webroot/assets/index.js").expect("sync mode"),
            Some(SyncMode::Mirror)
        );
    }

    #[test]
    fn default_sync_policy_preserves_runtime_config() {
        for rel in [
            ".config/subscription.json",
            "config.yaml",
            "config.yml",
            "subscriptions/main.txt",
            "mihomo.user.yaml",
        ] {
            assert_eq!(
                sync_mode_for_rel(&default_policy(), rel).expect("sync mode"),
                None,
                "{rel} should be preserved"
            );
        }
    }

    #[test]
    fn default_sync_policy_ignores_logs_and_cache() {
        for rel in [
            "logs/service.log",
            ".log/install.log",
            "cache/state",
            "old.bak",
        ] {
            assert_eq!(
                sync_mode_for_rel(&default_policy(), rel).expect("sync mode"),
                None,
                "{rel} should be ignored"
            );
        }
    }

    #[test]
    fn default_sync_policy_overlays_scripts() {
        assert_eq!(
            sync_mode_for_rel(&default_policy(), "service.sh").expect("sync mode"),
            Some(SyncMode::Overlay)
        );
    }
}
