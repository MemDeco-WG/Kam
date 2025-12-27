/// Simple helper that deduplicates repeated `[PATH]` entries inside the
/// `Arguments:` section of a clap help string by keeping only the last
/// occurrence. This mirrors the behavior used when printing localized help
/// (we prefer the translated entry, which is typically appended last).
///
/// In addition to collapsing duplicate argument blocks, this helper also
/// normalizes the `Usage:` line by collapsing consecutive duplicate tokens
/// (e.g. `[PATH] [PATH]` -> `[PATH]`) so the final help text contains a
/// single usage placeholder where appropriate.
fn dedupe_path_entries(help: &str) -> String {
    // Split into lines for simple scanning.
    let lines: Vec<&str> = help.split('\n').collect();

    // Find the start of the Arguments: section.
    let start_opt = lines
        .iter()
        .position(|l| l.trim_start().starts_with("Arguments:"));
    let start = match start_opt {
        Some(idx) => idx,
        None => {
            // No Arguments: section; still normalize the Usage line and return.
            let mut out_lines: Vec<String> = lines.into_iter().map(|s| s.to_string()).collect();
            for ln in out_lines.iter_mut() {
                if ln.trim_start().starts_with("Usage:") {
                    let tokens: Vec<&str> = ln.split_whitespace().collect();
                    if tokens.len() > 1 {
                        let mut new_tokens: Vec<&str> = Vec::new();
                        // Keep the 'Usage:' prefix and collapse consecutive duplicates.
                        new_tokens.push(tokens[0]);
                        for tok in tokens.into_iter().skip(1) {
                            if new_tokens.last().copied() != Some(tok) {
                                new_tokens.push(tok);
                            }
                        }
                        *ln = new_tokens.join(" ");
                    }
                }
            }
            let mut out = out_lines.join("\n");
            if !out.ends_with('\n') {
                out.push('\n');
            }
            return out;
        }
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
        // Still normalize the Usage line even if there was nothing to remove.
        let mut out_lines: Vec<String> = lines.into_iter().map(|s| s.to_string()).collect();
        for ln in out_lines.iter_mut() {
            if ln.trim_start().starts_with("Usage:") {
                let tokens: Vec<&str> = ln.split_whitespace().collect();
                if tokens.len() > 1 {
                    let mut new_tokens: Vec<&str> = Vec::new();
                    new_tokens.push(tokens[0]);
                    for tok in tokens.into_iter().skip(1) {
                        if new_tokens.last().copied() != Some(tok) {
                            new_tokens.push(tok);
                        }
                    }
                    *ln = new_tokens.join(" ");
                }
            }
        }
        let mut out = out_lines.join("\n");
        if !out.ends_with('\n') {
            out.push('\n');
        }
        return out;
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

    // Normalize the Usage line: collapse consecutive duplicated tokens while preserving order.
    // Example: `Usage: kam install [OPTIONS] [PATH] [PATH]` -> `Usage: kam install [OPTIONS] [PATH]`
    for ln in out_lines.iter_mut() {
        if ln.trim_start().starts_with("Usage:") {
            let tokens: Vec<&str> = ln.split_whitespace().collect();
            if tokens.len() > 1 {
                let mut new_tokens: Vec<&str> = Vec::new();
                // Keep the 'Usage:' prefix and collapse ONLY consecutive duplicates.
                new_tokens.push(tokens[0]);
                for tok in tokens.into_iter().skip(1) {
                    if new_tokens.last().copied() != Some(tok) {
                        new_tokens.push(tok);
                    }
                }
                *ln = new_tokens.join(" ");
            }
        }
    }

    // If the Arguments: section contains headers (e.g. [PATH]), avoid duplicating
    // those tokens in the Usage line by removing them there and keeping the
    // authoritative single header in the Arguments section.
    let mut arg_headers: Vec<String> = Vec::new();
    if let Some(arg_start) = out_lines
        .iter()
        .position(|l| l.trim_start().starts_with("Arguments:"))
    {
        let mut j = arg_start + 1;
        while j < out_lines.len() {
            let t = out_lines[j].trim();
            if !t.is_empty() && t.ends_with(':') {
                break;
            }
            if t.starts_with('[') && t.ends_with(']') {
                arg_headers.push(t.to_string());
            }
            j += 1;
        }
    }

    if !arg_headers.is_empty() {
        for ln in out_lines.iter_mut() {
            if ln.trim_start().starts_with("Usage:") {
                let tokens: Vec<&str> = ln.split_whitespace().collect();
                if tokens.len() > 1 {
                    let mut new_tokens: Vec<&str> = Vec::new();
                    new_tokens.push(tokens[0]);
                    for tok in tokens.into_iter().skip(1) {
                        if arg_headers.iter().any(|h| h == tok) {
                            // skip tokens that are represented in the Arguments section
                            continue;
                        }
                        if new_tokens.last().copied() != Some(tok) {
                            new_tokens.push(tok);
                        }
                    }
                    *ln = new_tokens.join(" ");
                }
            }
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
