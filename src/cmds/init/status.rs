use colored::{Color, Colorize};

/// Status types for file operations
pub enum StatusType {
    /// Adding a new file or directory
    Add,
    /// Updating an existing file or directory
    Update,
    /// Skipping an existing file or directory
    Skip,
    /// Copying from one path to another
    Copy(String, String),
    /// Creating a symlink
    Symlink(String, String),
    /// Deleting a file or directory
    Delete,
}

/// Print status message for file operations
pub fn print_status(status: StatusType, rel: &str, is_dir: bool) {
    match status {
        StatusType::Add => {
            let color = if is_dir { Color::Blue } else { Color::Green };
            println!("{}", format!("+ {}", rel).color(color));
        }
        StatusType::Update => {
            println!("{}", format!("~ {}", rel).color(Color::Yellow));
        }
        StatusType::Skip => {
            println!("{}", format!("~ {}", rel).color(Color::Yellow));
        }
        StatusType::Copy(from, to) => {
            println!("{}", format!("{} -> {}", from, to).color(Color::Cyan));
        }
        StatusType::Symlink(target, link) => {
            println!(
                "{}",
                format!("{} --> {}", link, target).color(Color::Magenta)
            );
        }
        StatusType::Delete => {
            println!("{}", format!("- {}", rel).color(Color::Red));
        }
    }
}
