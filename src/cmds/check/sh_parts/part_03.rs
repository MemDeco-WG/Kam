/// Detect unbalanced syntactic shell constructs that are not easily
/// covered by naive bracket counting (and would otherwise false-positive
/// on patterns like `case foo)`).
///
/// We focus on:
/// - command substitutions: `$(` ... `)`
/// - arithmetic expansions: `((` ... `))` (also `$( (` form via `$((`))
/// - backticks: `` `...` ``
///
/// The checks are quote-aware and skip text inside single quotes.
fn detect_unbalanced_shell_constructs(src: &str) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();
    let bytes = src.as_bytes();
    let mut i: usize = 0;
    let mut single = false;
    let mut double = false;
    let mut escaped = false;

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }
        if ch == '\'' && !double {
            single = !single;
            i += 1;
            continue;
        }
        if ch == '"' && !single {
            double = !double;
            i += 1;
            continue;
        }
        if single {
            i += 1;
            continue;
        }

        // Detect $(
        if ch == '$' && i + 1 < bytes.len() && bytes[i + 1] as char == '(' {
            let open_idx = i + 1; // index of '('
            if find_matching_paren(src, open_idx).is_none() {
                errors.push("Unbalanced command substitution detected".to_string());
            } else if let Some(j) = find_matching_paren(src, open_idx) {
                i = j + 1;
                continue;
            }
        }

        // Detect backticks
        if ch == '`' {
            let mut j = i + 1;
            let mut found = false;
            let mut esc = false;
            while j < bytes.len() {
                let chj = bytes[j] as char;
                if esc {
                    esc = false;
                    j += 1;
                    continue;
                }
                if chj == '\\' {
                    esc = true;
                    j += 1;
                    continue;
                }
                if chj == '`' {
                    found = true;
                    break;
                }
                j += 1;
            }
            if found {
                i = j + 1;
                continue;
            }
            errors.push("Unbalanced backticks detected".to_string());
        }

        // Detect arithmetic '(( ... ))' (heuristic: previous char should be whitespace or control)
        if ch == '(' && i + 1 < bytes.len() && bytes[i + 1] as char == '(' {
            let prevc = if i == 0 { '\n' } else { bytes[i - 1] as char };
            if prevc.is_whitespace() || prevc == ';' || prevc == '(' || prevc == '|' || prevc == '&'
            {
                let open_idx = i;
                if find_matching_paren(src, open_idx).is_none() {
                    errors.push("Unbalanced arithmetic expansion detected".to_string());
                } else if let Some(j) = find_matching_paren(src, open_idx) {
                    i = j + 1;
                    continue;
                }
            }
        }

        i += 1;
    }

    // Deduplicate messages
    errors.sort();
    errors.dedup();
    errors
}

/// Find the index of the matching closing `)` for an opening `(` at `open_idx`.
/// This is quote- and escape-aware and supports nesting.
fn find_matching_paren(src: &str, open_idx: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    if open_idx >= bytes.len() || bytes[open_idx] as char != '(' {
        return None;
    }
    let mut depth: i32 = 1;
    let mut i = open_idx + 1;
    let mut single = false;
    let mut double = false;
    let mut escaped = false;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }
        if ch == '\'' && !double {
            single = !single;
            i += 1;
            continue;
        }
        if ch == '"' && !single {
            double = !double;
            i += 1;
            continue;
        }
        if single {
            i += 1;
            continue;
        }
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn detects_unbalanced_command_substitution() -> Result<(), Box<dyn Error>> {
        let mut f = NamedTempFile::new()?;
        writeln!(f, "echo $(date")?;
        let fr = check_sh_custom(f.path(), false)?;
        assert!(!fr.valid);
        assert!(
            fr.errors
                .iter()
                .any(|e| e.contains("Unbalanced command substitution"))
        );
        Ok(())
    }

    #[test]
    fn does_not_flag_case_pattern() -> Result<(), Box<dyn Error>> {
        let mut f = NamedTempFile::new()?;
        write!(f, "case $x in\n  foo)\n    echo ok\n    ;;\nesac\n")?;
        let fr = check_sh_custom(f.path(), false)?;
        assert!(fr.errors.iter().all(|e| !e.contains("Unbalanced")));
        Ok(())
    }

    #[test]
    fn detects_unbalanced_backtick() -> Result<(), Box<dyn Error>> {
        let mut f = NamedTempFile::new()?;
        writeln!(f, "echo `date")?;
        let fr = check_sh_custom(f.path(), false)?;
        assert!(!fr.valid);
        assert!(fr.errors.iter().any(|e| e.contains("Unbalanced backticks")));
        Ok(())
    }

    #[test]
    fn detects_unbalanced_arithmetic() -> Result<(), Box<dyn Error>> {
        let mut f = NamedTempFile::new()?;
        writeln!(f, "for (( i=0; i<10; i++ ; do echo $i; done")?;
        let fr = check_sh_custom(f.path(), false)?;
        assert!(!fr.valid);
        assert!(
            fr.errors
                .iter()
                .any(|e| e.contains("Unbalanced arithmetic expansion"))
        );
        Ok(())
    }

    #[test]
    fn accepts_nested_command_substitution() -> Result<(), Box<dyn Error>> {
        let mut f = NamedTempFile::new()?;
        writeln!(f, "echo $(echo $(date))")?;
        let fr = check_sh_custom(f.path(), false)?;
        assert!(fr.valid);
        Ok(())
    }
}
