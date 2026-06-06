use self::analysis::{
    ScriptInfo, analyze_script, find_function_calls, is_shell_script, lifecycle_roots,
    resolve_source_path,
};
use self::audit::{insert_audit_preamble_after_shebang, obfuscation_audit_preamble};
use self::kamfw_semantics::{
    is_kamfw_shell_path, semantic_function_roots, semantic_sources, should_keep_all_kamfw,
};
use self::rewrite::{obfuscate_body, remove_unused_functions};
use super::args::BuildArgs;
use crate::errors::kam::KamError;

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

mod analysis;
mod audit;
mod kamfw_semantics;
mod rewrite;

#[derive(Clone)]
pub(super) struct PackageFile {
    pub(super) rel_path: String,
    pub(super) content: Vec<u8>,
}

pub(super) struct ShellPackageOptimizer {
    args: BuildArgs,
    scripts: BTreeMap<String, ScriptInfo>,
    module_id: String,
}

impl ShellPackageOptimizer {
    pub(super) fn new(args: &BuildArgs, module_id: &str) -> Self {
        Self {
            args: args.clone(),
            scripts: BTreeMap::new(),
            module_id: module_id.to_string(),
        }
    }

    pub(super) fn enabled(&self) -> bool {
        self.args.trim_shell || self.args.obfuscate_shell
    }

    pub(super) fn prepare(&mut self, files: &[PackageFile]) -> Result<(), KamError> {
        self.scripts.clear();
        for file in files {
            if !is_shell_script(&file.rel_path, &file.content) {
                continue;
            }
            let Ok(content) = String::from_utf8(file.content.clone()) else {
                continue;
            };
            let info = analyze_script(&file.rel_path, &content)?;
            self.scripts.insert(file.rel_path.clone(), info);
        }
        Ok(())
    }

    pub(super) fn should_package(&self, rel_path: &str) -> bool {
        if !self.args.trim_shell || !self.scripts.contains_key(rel_path) {
            return true;
        }
        self.reachable_scripts().contains(rel_path)
    }

    pub(super) fn transform_file(&self, file: &PackageFile) -> Result<Vec<u8>, KamError> {
        let Some(info) = self.scripts.get(&file.rel_path) else {
            return Ok(file.content.clone());
        };
        let mut content = info.content.clone();
        let reachable_functions = self.reachable_functions();
        if self.args.trim_shell_functions
            && !info.dynamic
            && !info.parse_failed
            && let Some(used) = reachable_functions.get(&info.rel_path)
        {
            content = remove_unused_functions(&content, info, used);
        }
        if self.args.obfuscate_shell {
            let audit_preamble = obfuscation_audit_preamble(&info.rel_path, &content)?;
            let obfuscated = obfuscate_body(&content, info, &self.module_id)?;
            content = insert_audit_preamble_after_shebang(&obfuscated, &audit_preamble);
        }
        Ok(content.into_bytes())
    }

    fn reachable_scripts(&self) -> BTreeSet<String> {
        if !self.args.trim_shell {
            return self.scripts.keys().cloned().collect();
        }

        let mut reachable = BTreeSet::new();
        let mut queue = VecDeque::new();
        for root in self.entry_scripts() {
            if self.scripts.contains_key(&root) && reachable.insert(root.clone()) {
                queue.push_back(root);
            }
        }

        while let Some(rel) = queue.pop_front() {
            let Some(info) = self.scripts.get(&rel) else {
                continue;
            };
            for sourced in self.resolve_sources(info) {
                if self.scripts.contains_key(&sourced) && reachable.insert(sourced.clone()) {
                    queue.push_back(sourced);
                }
            }
        }

        reachable
    }

    fn reachable_functions(&self) -> HashMap<String, BTreeSet<String>> {
        let mut result = HashMap::new();
        let script_roots = self.reachable_scripts();
        let duplicate_functions = self.duplicate_functions();
        let function_index = self.function_index();
        let mut queue = VecDeque::new();

        for rel in &script_roots {
            let Some(info) = self.scripts.get(rel) else {
                continue;
            };
            if info.dynamic || info.parse_failed {
                result.insert(rel.clone(), info.functions.keys().cloned().collect());
                continue;
            }
            let used = result.entry(rel.clone()).or_insert_with(BTreeSet::new);
            for name in info.functions.keys() {
                if duplicate_functions.contains(name) {
                    used.insert(name.clone());
                }
            }
            let mut roots = info.references.clone();
            for sourced in self.resolve_sources(info) {
                if let Some(sourced_info) = self.scripts.get(&sourced) {
                    roots.extend(sourced_info.references.clone());
                }
            }
            roots.extend(semantic_function_roots(info));
            for name in lifecycle_roots(rel) {
                if info.functions.contains_key(*name) {
                    roots.insert((*name).to_string());
                }
            }
            for root in roots {
                if info.functions.contains_key(&root) {
                    result
                        .entry(rel.clone())
                        .or_insert_with(BTreeSet::new)
                        .insert(root.clone());
                    queue.push_back((rel.clone(), root));
                }
            }
        }

        while let Some((rel, function_name)) = queue.pop_front() {
            let Some(info) = self.scripts.get(&rel) else {
                continue;
            };
            let Some(def) = info.functions.get(&function_name) else {
                continue;
            };
            let body = &info.content[def.start..def.end];
            for call in find_function_calls(body, &function_index).unwrap_or_default() {
                let Some(target_rel) = function_index.get(&call).cloned() else {
                    continue;
                };
                if !script_roots.contains(&target_rel) {
                    continue;
                }
                let inserted = result
                    .entry(target_rel.clone())
                    .or_insert_with(BTreeSet::new)
                    .insert(call.clone());
                if inserted {
                    queue.push_back((target_rel, call));
                }
            }
        }

        for rel in script_roots {
            result.entry(rel).or_insert_with(BTreeSet::new);
        }
        result
    }

    fn entry_scripts(&self) -> Vec<String> {
        const ROOTS: &[&str] = &[
            "customize.sh",
            "post-fs-data.sh",
            "service.sh",
            "uninstall.sh",
            "action.sh",
            "boot-completed.sh",
            "post-mount.sh",
            "metainstall.sh",
            "metamount.sh",
            "metauninstall.sh",
            "cli",
        ];
        let mut roots = ROOTS
            .iter()
            .filter(|root| self.scripts.contains_key(**root))
            .map(|root| (*root).to_string())
            .collect::<Vec<_>>();
        if roots.is_empty() {
            roots.extend(self.scripts.keys().cloned());
        }
        roots
    }

    fn resolve_sources(&self, info: &ScriptInfo) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        if should_keep_all_kamfw(info) {
            out.extend(
                self.scripts
                    .keys()
                    .filter(|path| is_kamfw_shell_path(path))
                    .cloned(),
            );
        }
        out.extend(
            semantic_sources(info)
                .into_iter()
                .filter(|path| self.scripts.contains_key(path)),
        );
        for raw in info.sources.iter().chain(info.imports.iter()) {
            if let Some(path) = resolve_source_path(&info.rel_path, raw, &self.module_id) {
                if self.scripts.contains_key(&path) {
                    out.insert(path);
                }
            } else if (raw == "*" || raw.contains('*')) && !analysis::is_dynamic_source(raw) {
                out.extend(
                    self.scripts
                        .keys()
                        .filter(|path| {
                            analysis::has_shell_extension(path)
                                && path.starts_with(analysis::parent_dir(&info.rel_path).as_str())
                        })
                        .cloned(),
                );
            }
        }
        out
    }

    fn function_index(&self) -> HashMap<String, String> {
        let mut counts = HashMap::<String, usize>::new();
        for info in self.scripts.values() {
            for name in info.functions.keys() {
                *counts.entry(name.clone()).or_default() += 1;
            }
        }

        let mut index = HashMap::new();
        for info in self.scripts.values() {
            for name in info.functions.keys() {
                if counts.get(name).copied().unwrap_or_default() == 1 {
                    index.insert(name.clone(), info.rel_path.clone());
                }
            }
        }
        index
    }

    fn duplicate_functions(&self) -> BTreeSet<String> {
        let mut counts = HashMap::<String, usize>::new();
        for info in self.scripts.values() {
            for name in info.functions.keys() {
                *counts.entry(name.clone()).or_default() += 1;
            }
        }
        counts
            .into_iter()
            .filter_map(|(name, count)| (count > 1).then_some(name))
            .collect()
    }
}

#[cfg(test)]
mod tests;
