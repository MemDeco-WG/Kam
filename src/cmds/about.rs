//! About command module root
//!
//! This module exposes the `about` subcommand and routes to the argument
//! definition and handler implementation. The command is purely informational
//! and prints author and contact information in a stylized manner.

pub mod args;
pub mod handler;

pub use args::*;
pub use handler::run;
