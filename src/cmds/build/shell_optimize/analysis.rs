use crate::errors::kam::KamError;

use regex::Regex;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use tree_sitter::{Node, Parser};

#[derive(Clone)]
pub(super) struct ScriptInfo {
    pub(super) rel_path: String,
    pub(super) content: String,
    pub(super) functions: BTreeMap<String, FunctionDef>,
    pub(super) references: BTreeSet<String>,
    pub(super) imports: BTreeSet<String>,
    pub(super) sources: BTreeSet<String>,
    pub(super) dynamic: bool,
    pub(super) parse_failed: bool,
}

#[derive(Clone)]
pub(super) struct FunctionDef {
    pub(super) name: String,
    pub(super) start: usize,
    pub(super) end: usize,
}

pub(super) fn analyze_script(rel_path: &str, content: &str) -> Result<ScriptInfo, KamError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter::Language::new(tree_sitter_bash::LANGUAGE))
        .map_err(|e| KamError::CommandFailed(format!("Failed to initialize bash parser: {e}")))?;
    let tree = parser.parse(content, None);
    let parse_failed = tree
        .as_ref()
        .is_none_or(|tree| tree.root_node().has_error());
    let mut info = ScriptInfo {
        rel_path: rel_path.to_string(),
        content: content.to_string(),
        functions: BTreeMap::new(),
        references: BTreeSet::new(),
        imports: BTreeSet::new(),
        sources: BTreeSet::new(),
        dynamic: content_contains_dynamic_shell(content),
        parse_failed,
    };

    if let Some(tree) = tree {
        walk_tree(tree.root_node(), content, &mut info);
    }

    find_regex_fallbacks(content, &mut info);
    Ok(info)
}

pub(super) fn find_function_calls(
    body: &str,
    function_index: &HashMap<String, String>,
) -> Result<BTreeSet<String>, KamError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter::Language::new(tree_sitter_bash::LANGUAGE))
        .map_err(|e| KamError::CommandFailed(format!("Failed to initialize bash parser: {e}")))?;
    let Some(tree) = parser.parse(body, None) else {
        return Ok(BTreeSet::new());
    };
    let mut calls = BTreeSet::new();
    collect_calls(tree.root_node(), body, function_index, &mut calls);
    Ok(calls)
}

pub(super) fn is_shell_script(rel_path: &str, content: &[u8]) -> bool {
    has_shell_extension(rel_path)
        || rel_path == "cli"
        || content.starts_with(b"#!/system/bin/sh")
        || content.starts_with(b"#!/sbin/sh")
        || content.starts_with(b"#!/bin/sh")
}

pub(super) fn resolve_source_path(current_rel: &str, raw: &str, module_id: &str) -> Option<String> {
    let raw = raw
        .replace("${MODPATH}", "")
        .replace("$MODPATH", "")
        .replace("${KAM_MODULE_ROOT}", "")
        .replace("$KAM_MODULE_ROOT", "")
        .replace("{{id}}", module_id)
        .trim_start_matches('/')
        .to_string();
    if raw.is_empty() || raw.contains('$') || raw.contains('{') || raw.contains('}') {
        return None;
    }
    if has_shell_extension(&raw) || raw.contains('/') {
        return Some(normalize_rel_path(&parent_dir(current_rel), &raw));
    }
    Some(format!("lib/kamfw/{raw}.sh"))
}

pub(super) fn has_shell_extension(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sh"))
}

pub(super) fn parent_dir(rel: &str) -> String {
    Path::new(rel)
        .parent()
        .and_then(Path::to_str)
        .unwrap_or("")
        .to_string()
}

pub(super) fn lifecycle_roots(rel: &str) -> &'static [&'static str] {
    match rel {
        "customize.sh" => &["kamfw_phase_customize", "customize", "main"],
        "post-fs-data.sh" => &["kamfw_phase_post_fs_data", "post_fs_data", "main"],
        "service.sh" => &["kamfw_phase_service", "service", "main"],
        "uninstall.sh" => &["kamfw_phase_uninstall", "uninstall", "main"],
        "action.sh" => &["kamfw_phase_action", "action", "main"],
        _ => &["main"],
    }
}

fn walk_tree(node: Node<'_>, src: &str, info: &mut ScriptInfo) {
    if node.kind() == "function_definition"
        && let Some(def) = parse_function_def(node, src)
    {
        info.functions.insert(def.name.clone(), def);
    }
    if node.kind() == "command" {
        inspect_command(node, src, info);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree(child, src, info);
    }
}

fn parse_function_def(node: Node<'_>, src: &str) -> Option<FunctionDef> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "word" | "function_name") {
            let name = child.utf8_text(src.as_bytes()).ok()?.to_string();
            return Some(FunctionDef {
                name,
                start: node.start_byte(),
                end: node.end_byte(),
            });
        }
    }
    None
}

fn inspect_command(node: Node<'_>, src: &str, info: &mut ScriptInfo) {
    let words = command_words(node, src);
    let Some(first) = words.first() else {
        return;
    };
    if first == "." || first == "source" {
        if let Some(target) = words.get(1) {
            info.sources.insert(strip_shell_quotes(target));
        }
        return;
    }
    if first == "import" {
        for target in words.iter().skip(1) {
            info.imports.insert(strip_shell_quotes(target));
        }
        return;
    }
    if first == "eval" || first == "alias" || first.contains('$') {
        info.dynamic = true;
        return;
    }
    info.references.insert(first.clone());
}

fn command_words(node: Node<'_>, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "word" | "command_name" | "string" | "raw_string"
        ) && let Ok(text) = child.utf8_text(src.as_bytes())
        {
            out.push(strip_shell_quotes(text));
        }
    }
    out
}

fn find_regex_fallbacks(content: &str, info: &mut ScriptInfo) {
    let fn_re = Regex::new(r"(?m)^\s*([A-Za-z_][A-Za-z0-9_]*)\s*\(\s*\)\s*\{").unwrap();
    for caps in fn_re.captures_iter(content) {
        let Some(m) = caps.get(0) else {
            continue;
        };
        let Some(name) = caps.get(1) else {
            continue;
        };
        info.functions
            .entry(name.as_str().to_string())
            .or_insert_with(|| FunctionDef {
                name: name.as_str().to_string(),
                start: m.start(),
                end: find_function_end(content, m.end()).unwrap_or(m.end()),
            });
    }

    let source_re = Regex::new(r#"(?m)(?:^|[;&]\s*)(?:\.|source)\s+["']?([^"'\s;]+)"?"#).unwrap();
    for caps in source_re.captures_iter(content) {
        if let Some(m) = caps.get(1) {
            info.sources.insert(strip_shell_quotes(m.as_str()));
        }
    }

    let import_re = Regex::new(r"(?m)(?:^|[;&]\s*)import\s+([A-Za-z0-9_./*-]+)").unwrap();
    for caps in import_re.captures_iter(content) {
        if let Some(m) = caps.get(1) {
            info.imports.insert(strip_shell_quotes(m.as_str()));
        }
    }
}

fn find_function_end(content: &str, body_start: usize) -> Option<usize> {
    let mut depth = 1i32;
    let mut in_single = false;
    let mut in_double = false;
    let bytes = content.as_bytes();
    let mut idx = body_start;
    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '{' if !in_single && !in_double => depth += 1,
            '}' if !in_single && !in_double => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx + 1);
                }
            }
            _ => {}
        }
        idx += 1;
    }
    None
}

fn collect_calls(
    node: Node<'_>,
    src: &str,
    function_index: &HashMap<String, String>,
    calls: &mut BTreeSet<String>,
) {
    if node.kind() == "command" {
        let words = command_words(node, src);
        if let Some(first) = words.first()
            && function_index.contains_key(first)
        {
            calls.insert(first.clone());
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls(child, src, function_index, calls);
    }
}

fn normalize_rel_path(parent: &str, raw: &str) -> String {
    let mut parts = Vec::new();
    let path = if raw.starts_with("./") || raw.starts_with("../") {
        format!("{parent}/{raw}")
    } else {
        raw.to_string()
    };
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn content_contains_dynamic_shell(content: &str) -> bool {
    content.contains("eval ")
        || content.contains(" eval")
        || content.contains("command ")
        || content.contains("$(printf")
        || content.contains("${!")
}

fn strip_shell_quotes(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}
