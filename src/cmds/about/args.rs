use clap::Args;

/// Arguments for the about command
///
/// This command is informational only and intentionally does not perform any
/// side effects beyond printing a stylized about box.
#[derive(Args, Debug)]
pub struct AboutArgs {}
