/*
Kam/src/cmds/about/handler.rs
Use comfy_table for rendering About output, dynamic values, and remove any hardcoded ASCII art.
*/

use comfy_table::{presets::UTF8_FULL, Attribute, Cell, ContentArrangement, Row, Table};
use regex::Regex;

use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

use super::args::AboutArgs;

/// Try to extract an email address from a string:
/// - prefer `<email@domain>` pattern,
/// - otherwise fallback to a generic email regex match.
fn extract_email(text: &str) -> Option<String> {
    // Look for `<...>` email first
    if let Some(start) = text.find('<') {
        if let Some(end_rel) = text[start..].find('>') {
            let candidate = text[start + 1..start + end_rel].trim();
            if !candidate.is_empty() {
                return Some(candidate.to_string());
            }
        }
    }

    // Fallback: simple email regex
    if let Ok(re) = Regex::new(r"([a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+)") {
        if let Some(cap) = re.captures(text) {
            return Some(cap[1].to_string());
        }
    }

    None
}

/// Pick the first non-empty value from candidates and return as a String
fn pick_first<'a>(candidates: Vec<Option<&'a str>>, fallback: &'a str) -> String {
    for c in candidates {
        if let Some(v) = c {
            let vtrim = v.trim();
            if !vtrim.is_empty() {
                return vtrim.to_string();
            }
        }
    }
    fallback.to_string()
}

/// Execute the `kam about` subcommand.
pub fn run(_args: AboutArgs) -> Result<(), KamError> {
    // Use the current directory's kam.toml when present to prefer project-local metadata
    let cwd = std::env::current_dir().map_err(KamError::Io)?;
    let kam_toml = KamToml::load_from_dir(&cwd).ok();

    // Prefer values from kam.toml, otherwise fall back to Cargo metadata (via env), then to a sensible default.
    let name = pick_first(
        vec![
            kam_toml.as_ref().map(|k| k.prop.name.as_str()),
            option_env!("CARGO_PKG_NAME"),
        ],
        "kam",
    );

    let version = pick_first(
        vec![
            kam_toml.as_ref().map(|k| k.prop.version.as_str()),
            option_env!("CARGO_PKG_VERSION"),
        ],
        "unknown",
    );

    let description = pick_first(
        vec![
            kam_toml.as_ref().map(|k| k.prop.description.as_str()),
            option_env!("CARGO_PKG_DESCRIPTION"),
        ],
        "",
    );

    // Author: prefer kam.toml prop.author, otherwise CARGO_PKG_AUTHORS
    let author_raw = pick_first(
        vec![
            kam_toml.as_ref().map(|k| k.prop.author.as_str()),
            option_env!("CARGO_PKG_AUTHORS"),
        ],
        "LIghJUNction",
    );

    // For email, try (in order) to extract from author string, then KAM_AUTHOR_EMAIL env var,
    // then try the Cargo metadata authors string, otherwise none
    let mut email = extract_email(&author_raw);
    if email.is_none() {
        if let Ok(val) = std::env::var("KAM_AUTHOR_EMAIL") {
            if !val.trim().is_empty() {
                email = Some(val);
            }
        }
    }
    if email.is_none() {
        if let Some(pkg_authors) = option_env!("CARGO_PKG_AUTHORS") {
            email = extract_email(pkg_authors);
        }
    }

    // Developer / Homepage site
    let dev_site = "https://developers.kernelsu.org/";

    // Make a tidy table using comfy_table
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    // Header: style name + version a bit
    table.set_header(vec![
        Cell::new(format!("{} v{}", name, version))
            .add_attribute(Attribute::Bold)
            .fg(comfy_table::Color::Cyan),
        Cell::new(""),
    ]);

    // Rows:
    table.add_row(Row::from(vec![
        Cell::new("Author").add_attribute(Attribute::Bold),
        Cell::new(author_raw.clone()),
    ]));

    if let Some(email) = email {
        table.add_row(Row::from(vec![
            Cell::new("Email").add_attribute(Attribute::Bold),
            Cell::new(email),
        ]));
    }

    table.add_row(Row::from(vec![
        Cell::new("Developer").add_attribute(Attribute::Bold),
        Cell::new(dev_site),
    ]));

    if !description.is_empty() {
        table.add_row(Row::from(vec![
            Cell::new("Description").add_attribute(Attribute::Bold),
            Cell::new(description),
        ]));
    }

    // Optional repository info if available from Cargo metadata
    if let Some(repo) = option_env!("CARGO_PKG_REPOSITORY") {
        if !repo.trim().is_empty() {
            table.add_row(Row::from(vec![
                Cell::new("Repository").add_attribute(Attribute::Bold),
                Cell::new(repo),
            ]));
        }
    }

    // Print a small centered banner and the table for a nice output style
    Utils::banner(&format!("{} v{}", name, version));
    println!("{}", table);
    println!(); // visual spacing

    Utils::info("This command is informational only; it doesn't modify files or the registry.");
    Utils::info("Use other commands (e.g., `kam init`, `kam build`) to perform actions.");
    println!();

    Utils::section("Thanks for using Kam");
    Utils::info("Enjoy your module tooling experience!");
    Utils::success("Powered by the Kam CLI — Happy building!");

    Ok(())
}
