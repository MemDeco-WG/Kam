/*!
Search scoring helpers extracted from `cmds::repo`.

This module provides small, well-tested helpers used to compute a fuzzy
relevance score between a search `query` and candidate text fields
(name/description/summary/authors).

It intentionally mirrors the lightweight logic used in the original
`repo.rs` implementation:

- Exact substring match -> score = 1.0
- Token coverage (fraction of query tokens present in the haystack) ->
  token_ratio ∈ [0.0, 1.0]
- Fuzzy similarity = 1.0 - (levenshtein / max_len) where max_len is the
  greater number of characters in the two strings
- Final score = max(token_ratio, sim_max) where sim_max is the maximum
  similarity over the candidate fields

The function `score_entry` returns the computed score (caller decides the
threshold to use, e.g. 0.60 for search or 0.30 for interactive selection).
*/

use super::SearchEntry;

/// Compute character-based Levenshtein distance (cost: insert/delete/replace = 1).
/// Implemented in a compact, allocation-friendly manner.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let n = a_chars.len();
    let m = b_chars.len();

    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }

    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur: Vec<usize> = vec![0; m + 1];

    for (i, ca) in a_chars.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b_chars.iter().enumerate() {
            // Use a concise boolean-to-int idiom instead of an if-expression.
            let cost = usize::from(ca != cb);
            cur[j + 1] = std::cmp::min(std::cmp::min(cur[j] + 1, prev[j + 1] + 1), prev[j] + cost);
        }
        prev.copy_from_slice(&cur);
    }

    prev[m]
}

/// Normalized similarity in [0.0, 1.0] derived from Levenshtein distance.
///
/// - identical strings => 1.0
/// - empty vs non-empty => < 1.0 (or 0.0 if one side is non-empty and distance==len)
/// # Notes
/// - This function casts `usize` values to `f64` for normalization. While a
///   cast from `usize` to `f64` can theoretically lose precision on some targets,
///   for the intended use (comparing relatively short string lengths and producing
///   a normalized similarity score) the precision loss is acceptable.
/// - We explicitly allow `clippy::cast_precision_loss` for this function with that justification.
#[allow(clippy::cast_precision_loss)]
pub fn similarity(a: &str, b: &str) -> f64 {
    let a_trim = a.trim();
    let b_trim = b.trim();

    if a_trim.is_empty() && b_trim.is_empty() {
        return 1.0;
    }
    let max_len = a_trim.chars().count().max(b_trim.chars().count());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein(a_trim, b_trim) as f64;
    1.0 - (dist / (max_len as f64))
}

/// Score a candidate entry (name/description/summary/authors) against the provided query.
///
/// Returns a floating point score in [0.0, 1.0]. The caller is expected to
/// decide the acceptance threshold (for example: 0.60 for `search_remote`,
/// 0.30 for interactive selection).
///
/// This function mirrors the scoring logic used in the original `repo.rs`.
#[allow(clippy::cast_precision_loss)]
pub fn score_entry(
    name: &str,
    description: Option<&str>,
    summary: Option<&str>,
    authors: Option<&str>,
    query: &str,
) -> f64 {
    let q = query.to_lowercase().trim().to_string();
    if q.is_empty() {
        return 0.0;
    }

    let name_l = name.to_lowercase();
    let desc = description.unwrap_or("").to_lowercase();
    let sum = summary.unwrap_or("").to_lowercase();
    let auth = authors.unwrap_or("").to_lowercase();

    let hay = format!("{name_l} {desc} {sum} {auth}");

    // Exact substring match -> highest relevance
    if hay.contains(&q) {
        return 1.0;
    }

    // Token coverage: fraction of query tokens present in haystack
    let tokens: Vec<&str> = q.split_whitespace().collect();
    let matched_tokens = tokens.iter().filter(|t| hay.contains(*t)).count();
    let token_ratio = if tokens.is_empty() {
        0.0
    } else {
        matched_tokens as f64 / tokens.len() as f64
    };

    // Fuzzy similarity against individual fields; use the best match
    let sim_name = similarity(&q, &name_l);
    let sim_desc = similarity(&q, &desc);
    let sim_sum = similarity(&q, &sum);
    let sim_auth = similarity(&q, &auth);
    let sim_max = sim_name.max(sim_desc).max(sim_sum).max(sim_auth);

    token_ratio.max(sim_max)
}

/// Convenience helper to compute score from a `SearchEntry`.
pub fn score_search_entry(entry: &SearchEntry, query: &str) -> f64 {
    score_entry(
        &entry.name,
        entry.description.as_deref(),
        entry.summary.as_deref(),
        entry.authors.as_deref(),
        query,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-12;

    #[test]
    fn test_levenshtein_basic() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("flaw", "lawn"), 2);
    }

    #[test]
    fn test_similarity_basic() {
        assert!((similarity("", "") - 1.0).abs() < EPS);
        assert!((similarity("kitten", "kitten") - 1.0).abs() < EPS);
        // Known example: kitten vs sitting -> 1 - 3/7 ≈ 0.571428...
        let sim = similarity("kitten", "sitting");
        assert!(sim > 0.5 && sim < 0.7);
    }

    #[test]
    fn test_score_exact_substring() {
        let s = score_entry("AlphaModule", None, None, None, "alpha");
        assert!((s - 1.0).abs() < EPS);

        let s2 = score_entry("foo", Some("The Alpha project"), None, None, "alpha");
        assert!((s2 - 1.0).abs() < EPS);
    }

    #[test]
    fn test_score_token_coverage() {
        // token coverage should be 1.0 when all tokens are present
        let s = score_entry("foo", Some("bar baz"), None, None, "foo baz");
        assert!((s - 1.0).abs() < EPS);

        // partial coverage: 1 of 2 tokens matched -> 0.5
        let s2 = score_entry("foo", Some("bar"), None, None, "foo baz");
        assert!((s2 - 0.5).abs() < EPS);
    }

    #[test]
    fn test_score_fuzzy_similarity() {
        // small typo should still be high similarity (>= 0.6)
        let s = score_entry("kitten", None, None, None, "kittn");
        assert!(s >= 0.6);

        // very different -> low score
        let s2 = score_entry("alpha", Some("beta gamma"), None, None, "zzz");
        assert!(s2 < 0.3);
    }

    #[test]
    fn test_score_search_entry_helper() {
        let entry = SearchEntry {
            name: "AlphaPackage".to_string(),
            description: Some("A thing".to_string()),
            summary: None,
            authors: Some("Some Author".to_string()),
            url: None,
        };
        let s = score_search_entry(&entry, "alpha");
        assert!((s - 1.0).abs() < EPS);
    }
}
