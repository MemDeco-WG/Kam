// kam library
//
// Export a small formatting translation macro at crate root so it can be used
// from any module without needing to import it. The macro delegates to the
// `i18n` module's translation templates and then formats the result, which
// keeps translation responsibilities centralized.
//
// NOTE:
// - We define this macro here (crate root) to ensure it's always visible even
//   when other modules are compiled before the i18n module's implementation.
// - This macro intentionally mirrors the behaviour in the `i18n` module:
//   it gets the localized template via `crate::i18n::tr_key(...)` and then
//   applies `format!` with the provided arguments.
// - If you also keep a macro with the same name inside `i18n.rs`, you may
//   want to remove or avoid duplication there. The crate root version is a
//   guaranteed, early-available definition.
// kam library

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
        crate::i18n::tr_fmt($key, &__trf_refs)
    }};
}

pub mod assets;
pub mod cli;
pub mod cmds;
pub mod errors;
pub mod i18n;
pub mod template;
pub mod types;
pub mod utils;
pub mod utils_ext;
