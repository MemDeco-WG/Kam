use super::analysis::ScriptInfo;
use crate::errors::kam::KamError;

use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};

struct RewritePlan {
    function_renames: HashMap<String, String>,
    variable_renames: HashMap<String, String>,
}

pub(super) fn remove_unused_functions(
    content: &str,
    info: &ScriptInfo,
    used: &BTreeSet<String>,
) -> String {
    let mut ranges = info
        .functions
        .values()
        .filter(|def| !used.contains(&def.name))
        .map(|def| (def.start, trailing_newline_end(content, def.end)))
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| range.0);
    let mut out = String::with_capacity(content.len());
    let mut last = 0;
    for (start, end) in ranges {
        if start > last {
            out.push_str(&content[last..start]);
        }
        last = end.max(last);
    }
    out.push_str(&content[last..]);
    out
}

pub(super) fn obfuscate_body(
    content: &str,
    info: &ScriptInfo,
    module_id: &str,
) -> Result<String, KamError> {
    let plan = build_rewrite_plan(content, info, module_id);
    let mut out = content.to_string();
    for (from, to) in &plan.function_renames {
        out = replace_shell_identifier(&out, from, to)?;
    }
    for (from, to) in &plan.variable_renames {
        out = replace_shell_identifier(&out, from, to)?;
    }
    Ok(strip_comments_and_blank_lines(&out))
}

fn build_rewrite_plan(content: &str, info: &ScriptInfo, module_id: &str) -> RewritePlan {
    let seed = digest_prefix(module_id, &info.rel_path, content);
    let mut function_renames = HashMap::new();
    if !info.dynamic && !info.parse_failed {
        for name in info.functions.keys() {
            if !is_public_shell_name(name) {
                let next = format!("_k{}f{}", &seed[..8], function_renames.len());
                function_renames.insert(name.clone(), next);
            }
        }
    }

    let mut variable_renames = HashMap::new();
    let var_re = Regex::new(r"\b(local|readonly|export)?\s*(_[A-Za-z][A-Za-z0-9_]*)=").unwrap();
    for caps in var_re.captures_iter(content) {
        let Some(name) = caps.get(2) else {
            continue;
        };
        let name = name.as_str();
        if is_reserved_variable(name) {
            continue;
        }
        let next = format!("_k{}v{}", &seed[..8], variable_renames.len());
        variable_renames.entry(name.to_string()).or_insert(next);
    }

    RewritePlan {
        function_renames,
        variable_renames,
    }
}

fn replace_shell_identifier(content: &str, from: &str, to: &str) -> Result<String, KamError> {
    let re = Regex::new(&format!(r"\b{}\b", regex::escape(from)))
        .map_err(|e| KamError::CommandFailed(format!("Invalid rewrite pattern: {e}")))?;
    Ok(re.replace_all(content, to).to_string())
}

fn strip_comments_and_blank_lines(content: &str) -> String {
    let mut out = String::new();
    for (idx, line) in content.lines().enumerate() {
        if idx == 0 && line.starts_with("#!") {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        out.push_str(trimmed);
        out.push('\n');
    }
    out
}

fn trailing_newline_end(content: &str, mut end: usize) -> usize {
    while end < content.len() && content.as_bytes()[end].is_ascii_whitespace() {
        if content.as_bytes()[end] == b'\n' {
            end += 1;
            break;
        }
        end += 1;
    }
    end
}

fn digest_prefix(module_id: &str, rel_path: &str, content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(module_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(rel_path.as_bytes());
    hasher.update(b"\0");
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

fn is_public_shell_name(name: &str) -> bool {
    name.starts_with("kamfw_phase_")
        || matches!(
            name,
            "main"
                | "print_modname"
                | "on_install"
                | "set_permissions"
                | "service"
                | "post_fs_data"
                | "uninstall"
        )
}

fn is_reserved_variable(name: &str) -> bool {
    matches!(
        name,
        "_" | "_pid" | "_rc" | "_status" | "_tmp" | "_tmpdir" | "_KAM_SOURCE"
    )
}
