//! kam - core library for the `kam` command-line tool.
//!
//! This crate contains the core types, command handlers, template management,
//! and helpers used by the `kam` binary. It exposes modules such as `cmds`,
//! `template`, `types`, and various utilities consumed throughout the codebase.
//!
//! The crate defines a lightweight translation-and-format macro `trf!` at the
//! crate root. The macro delegates to the `i18n` module for translation
//! templates and then formats the result with provided arguments. Arguments are
//! collected into owned `String`s to avoid temporary reference lifetime issues.
//!
//! NOTE:
//! - The macro is defined at the crate root to ensure it is available from any
//!   module, even if the `i18n` module is compiled later in the build.
//! - The macro intentionally mirrors the behaviour in the `i18n` module:
//!   it resolves a keyed template via `crate::i18n::tr_key(...)` and then
//!   applies `format!` with the collected arguments.
//!
//! Temporary note: to allow incremental documentation work to proceed, this
//! crate currently relaxes the `missing_docs` lint (see attribute below).

#![warn(clippy::pedantic)]
#![warn(clippy::perf)]
#![deny(clippy::clone_on_ref_ptr)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(warnings)]
#![deny(clippy::all)]
#![deny(missing_docs)]

/// Translate and format a localized template identified by `key`.
///
/// The `trf!` macro resolves a localized template using `crate::i18n` and
/// formats it with the provided arguments. Arguments are collected into owned
/// `String`s to avoid temporary-borrow lifetime issues when callers pass
/// temporaries (e.g. `format!()` results or `path.display()`).
#[macro_export]
macro_rules! trf {
    ($key:expr $(, $args:expr )* $(,)?) => {{
        // Avoid holding references to temporaries by collecting owned Strings first.
        // This prevents `E0716: temporary value dropped while borrowed` when
        // callers pass temporary values like `path.display()` or format! results.
        let mut __trf_store: Vec<String> = Vec::new();
        $(
            __trf_store.push(format!("{}", $args));
        )*
        // Build a vector of references to the owned strings. These references
        // live for the duration of this block and are safe to pass to `tr_fmt`.
        let __trf_refs: Vec<&dyn std::fmt::Display> =
            __trf_store.iter().map(|s| s as &dyn std::fmt::Display).collect();
        $crate::i18n::tr_fmt($key, &__trf_refs[..])
    }};
}

/// Assets and bundled template data.
pub mod assets;
/// Command-line argument types and parsing helpers.
pub mod cli;
/// Implementations of individual commands.
pub mod cmds;
/// Terminal color definitions and helpers.
pub mod colors;
/// Error types and error-related utilities.
pub mod errors;
/// Internationalization and translation helpers.
pub mod i18n;
/// Rule definitions and linting/checking logic.
pub mod rules;
/// Template management, rendering, and related utilities.
pub mod template;
/// Shared types such as `KamToml`.
pub mod types;
/// General-purpose utilities.
pub mod utils;
/// Extended/auxiliary utility helpers.
pub mod utils_ext;
