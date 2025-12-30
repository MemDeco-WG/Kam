use once_cell::sync::Lazy;
use regex::Regex;
use std::io::{self, Write};

static URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://(www\.)?[-a-zA-Z0-9@:%._\+~#=]{2,256}(\.[a-z]{2,4})?\b([-a-zA-Z0-9@:%_\+.~#?&//=]*)")
        .unwrap_or_else(|e| panic!("Failed to compile URL regex: {e}"))
});

/// Prompt the user with `msg` and return `true` for yes, `false` for no.
/// On I/O failure this function prints an error message and exits with code 1.
pub fn confirm(msg: &str) -> bool {
    loop {
        print!("{}", msg);
        let mut input = String::new();
        if let Err(e) = io::stdout().flush() {
            eprintln!("Failed to flush stdout: {e}");
            std::process::exit(1);
        }
        if let Err(e) = io::stdin().read_line(&mut input) {
            eprintln!("Failed to read input: {e}");
            std::process::exit(1);
        }
        let trimmed = input.trim().to_lowercase();
        match trimmed.as_str() {
            "yes" | "y" => {
                return true;
            }
            "no" | "n" => {
                return false;
            }
            _ => {
                // Invalid input; re-prompt.
                continue;
            }
        }
    }
}

pub fn is_url(url: &str) -> bool {
    URL_RE.is_match(url)
}
