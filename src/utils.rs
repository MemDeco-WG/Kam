use std::path::Path;
use colored::{Color, Colorize};

pub struct Utils;

pub enum PrintOp {
    Create { is_dir: bool },
    Update,
    Delete,
    Copy { from: String, to: String },
    Symlink { target: String, link_type: LinkType },
}

pub enum LinkType {
    Soft,
    Hard,
}

impl Utils {
    pub fn print_status(path: &Path, rel: &str, op: PrintOp, force: bool) {
        if force || !path.exists() || matches!(op, PrintOp::Delete) {
            match op {
                PrintOp::Create { is_dir } => {
                    let color = if is_dir { Color::Blue } else { Color::Green };
                    println!("{}", format!("+ {}", rel).color(color));
                }
                PrintOp::Update => {
                    println!("{}", format!("~ {}", rel).color(Color::Yellow));
                }
                PrintOp::Delete => {
                    println!("{}", format!("- {}", rel).color(Color::Red));
                }
                PrintOp::Copy { from, to } => {
                    println!("{}", format!("{} -> {} (copy)", from, to).color(Color::Cyan));
                }
                PrintOp::Symlink { target, link_type } => {
                    let symbol = match link_type {
                        LinkType::Soft => "-->",
                        LinkType::Hard => "==>",
                    };
                    println!("{}", format!("{} {} {} (symlink)", rel, symbol, target).color(Color::Magenta));
                }
            }
        } else {
            // For existing files without force, perhaps do nothing or print update
            println!("{}", format!("~ {}", rel).color(Color::Yellow));
        }
    }
}
