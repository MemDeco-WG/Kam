/// Color theme and helpers for the Kam CLI.
///
/// Provides a global, lazily-initialized `Theme` that can be overridden
/// via environment variables (useful for global / per-user configuration).
///
/// Supported environment variables (hex `#RRGGBB` or `RRGGBB`):
/// - `KAM_COLOR_ERROR`
/// - `KAM_COLOR_WARN`
/// - `KAM_COLOR_INFO`
/// - `KAM_COLOR_SUCCESS`
///
/// Usage:
/// - Call `kam::colors::get_theme()` to obtain the global theme. Access
///   colors like `get_theme().error`.
use colored::Color;
use std::env;
use std::sync::OnceLock;

/// CLI color theme (semantic colors)
#[derive(Debug, Clone)]
pub struct Theme {
    pub error: Color,
    pub warn: Color,
    pub info: Color,
    pub success: Color,
}

impl Theme {
    // Default is provided by the standard `Default` trait implementation below.

    /// Load theme values from a generic getter (useful for testing).
    /// The provided `get` closure receives an env-style key like `"KAM_COLOR_ERROR"`
    /// and returns an owned `String` when present.
    pub fn load_from_map<F>(mut get: F) -> Self
    where
        F: FnMut(&str) -> Option<String>,
    {
        let mut t = Self::default();

        if let Some(v) = get("KAM_COLOR_ERROR")
            && let Some((r, g, b)) = parse_hex_color(&v)
        {
            t.error = Color::TrueColor { r, g, b };
        }

        if let Some(v) = get("KAM_COLOR_WARN")
            && let Some((r, g, b)) = parse_hex_color(&v)
        {
            t.warn = Color::TrueColor { r, g, b };
        }

        if let Some(v) = get("KAM_COLOR_INFO")
            && let Some((r, g, b)) = parse_hex_color(&v)
        {
            t.info = Color::TrueColor { r, g, b };
        }

        if let Some(v) = get("KAM_COLOR_SUCCESS")
            && let Some((r, g, b)) = parse_hex_color(&v)
        {
            t.success = Color::TrueColor { r, g, b };
        }

        t
    }

    /// Load theme values from environment variables (non-cached).
    ///
    /// This function delegates to `load_from_map` to make testing easier.
    pub fn load_from_env() -> Self {
        Self::load_from_map(|k| env::var(k).ok())
    }
}

/// Implement Default trait so callers can use `Theme::default()` idiomatically.
impl Default for Theme {
    fn default() -> Self {
        Self {
            error: Color::TrueColor {
                r: 255,
                g: 145,
                b: 80,
            },
            warn: Color::Yellow,
            info: Color::Cyan,
            success: Color::Green,
        }
    }
}

/// Parse a hex color string like `#RRGGBB` or `RRGGBB`.
fn parse_hex_color(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim();
    let s = s.strip_prefix('#').unwrap_or(s);

    if s.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

static THEME: OnceLock<Theme> = OnceLock::new();

/// Obtain the global theme singleton. Environment variables are read once
/// on first access and the result is cached for the process lifetime.
pub fn get_theme() -> &'static Theme {
    THEME.get_or_init(Theme::load_from_env)
}

#[cfg(test)]
/// Attempt to initialize the global theme for tests.
///
/// This test-only function tries to set the crate-global theme and returns
/// `Ok(())` when successful or `Err(theme)` if the theme had been previously
/// initialized. It is `pub(crate)` so it can be used by tests within the crate,
/// but it is not exposed to library consumers.
///
/// Note: `OnceLock` semantics mean the global theme can only be set once per
/// process; callers should account for this (tests that rely on this should
/// run serially or ensure they initialize the theme early).
pub(crate) fn init_theme_for_tests(theme: Theme) -> Result<(), Theme> {
    THEME.set(theme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use colored::Color;
    use serial_test::serial;

    #[test]
    fn parse_hex_ok() {
        let c = parse_hex_color("#FF9150").expect("parsed");
        assert_eq!(c, (255, 145, 80));
    }

    #[test]
    fn parse_hex_no_hash_ok() {
        let c = parse_hex_color("FF9150").expect("parsed");
        assert_eq!(c, (255, 145, 80));
    }

    #[test]
    fn parse_hex_whitespace_case_ok() {
        let c = parse_hex_color("  #fF9150 ").expect("parsed");
        assert_eq!(c, (255, 145, 80));
    }

    #[test]
    fn parse_bad_len() {
        assert!(parse_hex_color("#FFF").is_none());
        assert!(parse_hex_color("GGHHII").is_none()); // invalid hex
    }

    #[test]
    fn load_from_map_override() {
        let t = Theme::load_from_map(|k| {
            if k == "KAM_COLOR_ERROR" {
                Some("#010203".to_string())
            } else {
                None
            }
        });
        match t.error {
            Color::TrueColor { r, g, b } => assert_eq!((r, g, b), (1, 2, 3)),
            _ => panic!("expected TrueColor"),
        }
    }

    #[test]
    fn load_from_map_invalid_values_ignored() {
        let t = Theme::load_from_map(|k| {
            if k == "KAM_COLOR_ERROR" {
                Some("ZZZZZZ".to_string())
            } else {
                None
            }
        });
        // invalid value should leave the error color as default
        match t.error {
            Color::TrueColor { r, g, b } => assert_eq!((r, g, b), (255, 145, 80)),
            _ => panic!("expected TrueColor"),
        }
    }

    #[test]
    fn load_from_map_all_override() {
        let t = Theme::load_from_map(|k| match k {
            "KAM_COLOR_ERROR" => Some("#010203".to_string()),
            "KAM_COLOR_WARN" => Some("#040506".to_string()),
            "KAM_COLOR_INFO" => Some("#070809".to_string()),
            "KAM_COLOR_SUCCESS" => Some("#0A0B0C".to_string()),
            _ => None,
        });

        match t.error {
            Color::TrueColor { r, g, b } => assert_eq!((r, g, b), (1, 2, 3)),
            _ => panic!("expected TrueColor"),
        }
        match t.warn {
            Color::TrueColor { r, g, b } => assert_eq!((r, g, b), (4, 5, 6)),
            _ => panic!("expected TrueColor"),
        }
        match t.info {
            Color::TrueColor { r, g, b } => assert_eq!((r, g, b), (7, 8, 9)),
            _ => panic!("expected TrueColor"),
        }
        match t.success {
            Color::TrueColor { r, g, b } => assert_eq!((r, g, b), (10, 11, 12)),
            _ => panic!("expected TrueColor"),
        }
    }

    #[test]
    fn default_theme_values() {
        let t = Theme::default();
        match t.error {
            Color::TrueColor { r, g, b } => assert_eq!((r, g, b), (255, 145, 80)),
            _ => panic!("expected TrueColor"),
        }
        assert!(matches!(t.warn, Color::Yellow));
        assert!(matches!(t.info, Color::Cyan));
        assert!(matches!(t.success, Color::Green));
    }

    #[test]
    #[serial]
    fn init_theme_for_tests_sets_global_theme() {
        // Attempt to initialize the global theme for tests. If initialization fails
        // because the theme was already initialized elsewhere, accept that but make
        // sure a theme is available (no panic).
        let desired = Theme::load_from_map(|k| {
            if k == "KAM_COLOR_ERROR" {
                Some("#0f0f0f".to_string())
            } else {
                None
            }
        });

        let res = init_theme_for_tests(desired.clone());
        if res.is_ok() {
            let global_theme = get_theme();
            match global_theme.error {
                Color::TrueColor { r, g, b } => assert_eq!((r, g, b), (15, 15, 15)),
                _ => panic!("expected TrueColor"),
            }
        } else {
            // Already initialized by another test/crate: ensure get_theme returns a Theme
            let _ = get_theme();
        }
    }
}
