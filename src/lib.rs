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
    ($key:expr) => {
        crate::i18n::tr_key($key).to_string()
    };
    ($key:expr, $($args:expr),+ $(,)?) => {
        {
            // Build a slice of &dyn Display that can be passed to the runtime formatter.
            // We can't call `format!(template, ..)` because `format!` requires a
            // compile-time string literal for the format, so we delegate to a
            // runtime function `crate::i18n::tr_fmt` which handles formatting.
            let args_slice: &[&dyn std::fmt::Display] = &[
                $( &($args) as &dyn std::fmt::Display ),+
            ];
            crate::i18n::tr_fmt($key, args_slice)
        }
    };
}

pub mod assets;
pub mod cli;
pub mod cmds;
pub mod errors;
pub mod template;
pub mod types;
pub mod utils;
pub mod utils_ext;
pub mod i18n;
