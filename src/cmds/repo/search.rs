use super::cache::read_local_index;
use super::{SearchEntry, repo_search};
use crate::errors::KamError;
use crate::utils::Utils;
use std::io::{Write, stdin, stdout};

/// # Errors
/// Returns `KamError` on network, I/O, or JSON parsing failures.
pub fn search_local(query: &str, base_url: &str) -> Result<(), KamError> {
    let scored = scored_results(query, base_url, 0.60)?;
    if scored.is_empty() {
        Utils::warn(trf!("repo.no_results_for", query));
        return Ok(());
    }

    for (i, (score, entry)) in scored.iter().take(50).enumerate() {
        print_search_result(base_url, i, *score, entry, SearchPrintMode::Simple);
    }
    Ok(())
}

/// # Errors
/// Returns `KamError` on network, I/O, or parsing failures.
pub(crate) fn search_local_interactive(
    query: &str,
    base_url: &str,
) -> Result<Option<String>, KamError> {
    let scored = scored_results(query, base_url, 0.30)?;
    if scored.is_empty() {
        Utils::warn(trf!("repo.no_results_for", query));
        return Ok(None);
    }

    println!(
        "{}",
        trf!(
            "repo.similar_packages_header",
            scored.len().to_string(),
            query
        )
    );
    println!();
    for (i, (score, entry)) in scored.iter().take(20).enumerate() {
        print_search_result(base_url, i, *score, entry, SearchPrintMode::Numbered);
    }

    print!("{}", crate::i18n::tr("repo.prompt.enter_number"));
    stdout().flush().map_err(KamError::Io)?;
    let mut input = String::new();
    stdin().read_line(&mut input).map_err(KamError::Io)?;
    Ok(parse_selection(input.trim(), &scored))
}

fn scored_results<'a>(
    query: &str,
    base_url: &str,
    threshold: f64,
) -> Result<Vec<(f64, SearchEntryRef<'a>)>, KamError> {
    let entries = read_local_index(base_url)?;
    let q = query.to_lowercase().trim().to_string();
    if q.is_empty() {
        Utils::warn(crate::i18n::tr("repo.search.empty_query"));
        return Ok(Vec::new());
    }

    let mut scored: Vec<_> = entries
        .into_iter()
        .map(|entry| {
            let score = repo_search::score_search_entry(&entry, &q);
            (score, SearchEntryRef::Owned(entry))
        })
        .filter(|(score, _)| *score >= threshold)
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(scored)
}

enum SearchEntryRef<'a> {
    Owned(SearchEntry),
    #[allow(dead_code)]
    Borrowed(&'a SearchEntry),
}

impl SearchEntryRef<'_> {
    fn get(&self) -> &SearchEntry {
        match self {
            Self::Owned(entry) => entry,
            Self::Borrowed(entry) => entry,
        }
    }
}

#[derive(Copy, Clone)]
enum SearchPrintMode {
    Simple,
    Numbered,
}

fn print_search_result(
    base_url: &str,
    index: usize,
    score: f64,
    entry: &SearchEntryRef<'_>,
    mode: SearchPrintMode,
) {
    let entry = entry.get();
    let desc = entry.description.as_deref().unwrap_or("");
    let score_suffix = if (score - 1.0).abs() > f64::EPSILON {
        trf!("repo.score_format", format!("{score:.2}"))
    } else {
        String::new()
    };
    match mode {
        SearchPrintMode::Simple => println!(
            "{}",
            trf!("repo.result_line_simple", entry.name, desc, score_suffix)
        ),
        SearchPrintMode::Numbered => println!(
            "{}",
            trf!(
                "repo.result_line",
                (index + 1).to_string(),
                entry.name,
                desc,
                score_suffix
            )
        ),
    }
    print_entry_metadata(base_url, entry);
    println!();
}

fn print_entry_metadata(base_url: &str, entry: &SearchEntry) {
    if let Some(summary) = &entry.summary {
        println!("    {summary}");
    }
    if let Some(authors) = &entry.authors {
        println!("    {}: {authors}", crate::i18n::tr("repo.authors"));
    }
    if let Some(url) = &entry.url {
        let pretty = resolve_entry_url(base_url, url);
        println!("    {}: {pretty}", crate::i18n::tr("repo.url"));
    }
}

fn parse_selection(input: &str, scored: &[(f64, SearchEntryRef<'_>)]) -> Option<String> {
    if input.is_empty() {
        return None;
    }
    match input.parse::<usize>() {
        Ok(num) if num > 0 && num <= scored.len() => Some(scored[num - 1].1.get().name.clone()),
        Ok(_) => {
            Utils::warn(crate::i18n::tr("repo.invalid_selection_out_of_range"));
            None
        }
        Err(_) => {
            Utils::warn(crate::i18n::tr("repo.invalid_input_number"));
            None
        }
    }
}

fn resolve_entry_url(base_url: &str, url: &str) -> String {
    let u = url.trim();
    if u.starts_with("http://") || u.starts_with("https://") {
        return u.to_string();
    }
    if u.starts_with('/') {
        return format!("{}{}", base_url.trim_end_matches('/'), u);
    }
    format!("{}/{}", base_url.trim_end_matches('/'), u)
}
