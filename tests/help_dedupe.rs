use clap::{Arg, Command};

/// Simple helper that deduplicates repeated `[PATH]` entries inside the
/// `Arguments:` section of a clap help string by keeping only the last
/// occurrence. This mirrors the behavior used when printing localized help
/// (we prefer the translated entry, which is typically appended last).
fn dedupe_path_entries(help: &str) -> String {
    // Split into lines for simple scanning.
    let mut lines: Vec<&str> = help.split('\n').collect();

    // Find the start of the Arguments: section.
    let start_opt = lines
        .iter()
        .position(|l| l.trim_start().starts_with("Arguments:"));
    let start = match start_opt {
        Some(idx) => idx,
        None => return help.to_string(),
    };

    // Find the end of the section: the next top-level header (line that ends with ':')
    // or EOF.
    let mut end = lines.len();
    for i in (start + 1)..lines.len() {
        let t = lines[i].trim();
        if !t.is_empty() && t.ends_with(':') {
            end = i;
            break;
        }
    }

    // Find header indices where the trimmed header equals "[PATH]".
    let mut header_indices: Vec<usize> = Vec::new();
    for i in (start + 1)..end {
        if lines[i].trim() == "[PATH]" {
            header_indices.push(i);
        }
    }

    // Nothing to dedupe if zero or one occurrence.
    if header_indices.len() <= 1 {
        return help.to_string();
    }

    // For all occurrences except the last, compute the group range (header .. group_end).
    // A group's end is determined by the next blank line or the end of the section.
    let mut remove_ranges: Vec<(usize, usize)> = Vec::new();
    for &h in &header_indices[..header_indices.len() - 1] {
        let mut gend = h + 1;
        while gend < end && !lines[gend].trim().is_empty() {
            gend += 1;
        }
        remove_ranges.push((h, gend));
    }

    // Rebuild lines skipping removed ranges.
    let mut out_lines: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        // If this index is the start of a removal range, skip to the range end.
        let mut skipped = false;
        for &(s, e) in &remove_ranges {
            if i == s {
                i = e;
                skipped = true;
                break;
            }
        }
        if !skipped {
            out_lines.push(lines[i].to_string());
            i += 1;
        }
    }

    // Rejoin and ensure trailing newline (clap help typically ends with newline).
    let mut out = out_lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[test]
fn help_dedupe_removes_duplicate_path_entries() {
    // Use a synthetic help string that mimics clap's printed help for the
    // `install` subcommand which contains duplicated `[PATH]` argument entries
    // (English + Chinese). Building a `Command` with duplicate argument names
    // is invalid in clap, so using a synthetic string avoids that problem while
    // still testing the dedupe behavior.
    let raw = r#"Install a module (test stub)

Usage: kam install [OPTIONS] [PATH] [PATH]

Arguments:
  [PATH]
          Path to module package (.zip) to install. If omitted, attempts to find the artifact in the project (dist) output directory

  [PATH]
          模块包路径（.zip）。若省略，Kam 会在项目的 dist/ 输出目录尝试查找产物

Options:
  -v, --verbose
          Verbose output showing install command output (stdout/stderr)
"#;

    // Sanity: ensure the raw help actually contains duplicates of [PATH].
    let raw_count = raw.matches("[PATH]").count();
    assert!(
        raw_count > 1,
        "expected duplicated [PATH] in raw help, got:\n{}",
        raw
    );

    // Run the dedupe helper and verify duplicate [PATH] occurrences are reduced to 1.
    let cleaned = dedupe_path_entries(&raw);
    let cleaned_count = cleaned.matches("[PATH]").count();
    assert_eq!(
        cleaned_count, 1,
        "expected single [PATH] after dedupe; raw:\n{}\n\ndeduped:\n{}",
        raw, cleaned
    );

    // Verify the (translated) Chinese description is still present (we prefer keeping the last entry).
    assert!(
        cleaned.contains("模块包路径"),
        "expected translated description to be preserved in deduped help; got:\n{}",
        cleaned
    );
}
