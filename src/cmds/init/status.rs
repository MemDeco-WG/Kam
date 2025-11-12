<file_path>
Kam\src\cmds\init\status.rs
</file_path>

<edit_description>
Create a new status.rs module with an improved print_status function supporting more status types
</edit_description>

use colored::{Color, Colorize};

/// Status types for file operations
pub enum StatusType {
    /// Adding a new file or directory
    Add,
    /// Skipping an existing file or directory
    Skip,
    /// Copying from one path to another
    Copy(String, String),
    /// Linking from one path to another
    Link(String, String),
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
        StatusType::Skip => {
            println!("{}", format!("~ {}", rel).color(Color::Yellow));
        }
        StatusType::Copy(from, to) => {
            println!("{}", format!("{} -> {}", from, to).color(Color::Cyan));
        }
        StatusType::Link(from, to) => {
            println!("{}", format!("{} &-> {}", from, to).color(Color::Magenta));
        }
        StatusType::Delete => {
            println!("{}", format!("- {}", rel).color(Color::Red));
        }
    }
}
