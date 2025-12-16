use regex::Regex;
use std::io::{self, Write};

pub fn confirm(msg: &str) -> bool {
    loop {
        print!("{}", msg);
        let mut input = String::new();
        io::stdout().flush().unwrap();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read input");
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
    let url_regex: &str = r"https?://(www\.)?[-a-zA-Z0-9@:%._\+~#=]{2,256}(\.[a-z]{2,4})?\b([-a-zA-Z0-9@:%_\+.~#?&//=]*)";
    Regex::new(url_regex).unwrap().is_match(url)
}
