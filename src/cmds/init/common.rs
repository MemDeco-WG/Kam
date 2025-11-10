use colored::{Color, Colorize};
use std::path::Path;

pub fn print_status(path: &Path, rel: &str, is_dir: bool, force: bool) {
    if force || !path.exists() {
        let color = if is_dir { Color::Blue } else { Color::Green };
        println!("{}", format!("+ {}", rel).color(color));
    } else {
        println!("{}", format!("~ {}", rel).color(Color::Yellow));
    }
}
