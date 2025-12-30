//! Lightweight i18n / language detection module
//!
//! This module:
//! - Detects the user's preferred language (using a third-party crate when available)
//! - Allows overriding the language via `kam config` (key: `language` or `ui.language`)
//! - Provides a small lookup-based translation system (EN <-> ZH) for common UI phrases
//! - Exposes convenience helpers/macros for formatting localized strings
//!
//! Notes:
//! - The translation system is intentionally minimal: it maps a set of well-known
//!   phrases used across the CLI. If a phrase is not known, the original string
//!   is returned unchanged. This allows gradual internationalization without
//!   touching every call site immediately.
//! - Detection tries in order:
//!   1. `kam config` language configuration (local -> global)
//!   2. system locale via the `sys-locale` crate
//!   3. fallback to `LANG` environment variable
//!   4. default to English
//!
//! Usage examples:
//!
//! // initialize early in main()
//! kam::i18n::init();
//!
//! // simple translation of a literal
//! let msg = kam::i18n::tr_key("Thanks for using Kam");
//!
//! // format a translated template
//! use kam::i18n::trf;
//! let formatted = trf!("Building module: {} v{}", &module_id, &version);
//! ```
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{OnceLock, RwLock};

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    En,
    Zh,
}

// RwLock should be OK for a static global language setting.
// We default to English and allow changing it at runtime via `set_language`.
static CURRENT_LANGUAGE: RwLock<Language> = RwLock::new(Language::En);

static KEYED_EN: OnceLock<HashMap<String, String>> = OnceLock::new();
static KEYED_ZH: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Initialize i18n subsystem.
///
/// This should be called once at program start (e.g. in `main()`).
/// It will attempt to discover an overridden language via `kam config`,
/// otherwise it will fall back to system locale detection (via `sys-locale`)
/// and finally environment variable `LANG`.
pub fn init() {
    // Try to load runtime i18n overrides (KAM_I18N_DIR or ./i18n/) before other initialization.
    // This lets local deployments or packaging override translations without a rebuild.
    try_load_runtime_i18n();

    // 0. Environment variables take highest precedence. This allows quick one-off
    // overrides without changing config files.
    // Supported keys:
    // - KAM_UI_LANGUAGE : explicit UI language (e.g. "en", "zh", "zh-CN")
    // - KAM_LANG        : a secondary option for legacy or convenience
    if let Ok(val) = std::env::var("KAM_UI_LANGUAGE")
        && !val.trim().is_empty()
    {
        let _ = set_language_str(&val);
        return;
    }
    if let Ok(val) = std::env::var("KAM_LANG")
        && !val.trim().is_empty()
    {
        let _ = set_language_str(&val);
        return;
    }

    // 1. Try to read config (local or global). Prefer a 'language' or 'ui.language' key.
    // We call into config::read_language_from_config which is expected to be
    // a tiny public helper in `src/cmds/config.rs`.
    if let Some(lang_cfg) = get_language_from_config() {
        let _ = set_language_str(&lang_cfg);
        return;
    }

    // 2. Try to use sys-locale (third-party crate). If not available or None, fallback.
    #[allow(unused_mut)]
    {
        if let Some(detected) = detect_language_system() {
            let _ = set_language_str(&detected);
            return;
        }
    }

    // 3. Fallback to checking LANG env var.
    if let Ok(lang_env) = std::env::var("LANG") {
        let _ = set_language_str(&lang_env);
    }

    // 4. Otherwise default (already set to English)
}

/// Returns the current language.
pub fn current_language() -> Language {
    let lock = CURRENT_LANGUAGE.read().unwrap();
    *lock
}

/// Set language by enum
pub fn set_language(lang: Language) {
    let mut w = CURRENT_LANGUAGE.write().unwrap();
    *w = lang;
}

/// Set language by a string (e.g. "zh", "zh-CN", "en_US", "en")
/// Returns Err if the string is not recognized.
pub fn set_language_str(s: &str) -> Result<(), String> {
    let s_lc = s.trim().to_lowercase();
    let lang = if s_lc.starts_with("zh") {
        Language::Zh
    } else if s_lc.starts_with("en") {
        Language::En
    } else {
        return Err(format!("Unsupported language string: {}", s));
    };
    set_language(lang);
    Ok(())
}

/// Attempt to detect the language using the system locale. This uses the
/// third-party crate `sys-locale` if available (cfg gate), otherwise returns None.
fn detect_language_system() -> Option<String> {
    // Try the sys-locale crate first (if available).
    // If `sys-locale` wasn't added to Cargo.toml this will not compile; the user
    // may need to add it. We gracefully handle the absence of the crate by
    // using the LANG environment fallback in `init()`.
    #[cfg(feature = "sys-locale")]
    {
        if let Some(locale) = sys_locale::get_locale() {
            return Some(locale);
        }
    }

    // If `sys-locale` isn't available, try environment variable.
    if let Ok(lang) = std::env::var("LANG") {
        return Some(lang);
    }

    None
}

// --- Small translation system -------------------------------------------------

/// Translate a string key/template according to current language and return the
/// localized template string (with placeholders left intact, e.g. "{}").
///
/// If no translation mapping exists for the given string, it returns the
/// original string.
pub fn tr_key(key: &str) -> &str {
    // Helper that tries a simple, deterministic normalization: ascii <-> full-width colon.
    // The goal is to try the most-likely variant keys only (avoids overly aggressive normalization).
    fn tr_try_with_colon_variants<F>(key: &str, lookup: F) -> Option<&'static str>
    where
        F: Fn(&str) -> Option<&'static str>,
    {
        // Try exact match first.
        if let Some(v) = lookup(key) {
            return Some(v);
        }
        // If key contains full-width colon, try ASCII colon variant.
        if key.contains('：') {
            let alt = key.replace('：', ":");
            if let Some(v) = lookup(&alt) {
                return Some(v);
            }
        }
        // If key contains ASCII colon, try full-width colon variant.
        if key.contains(':') {
            let alt = key.replace(':', "：");
            if let Some(v) = lookup(&alt) {
                return Some(v);
            }
        }
        // Last defensive attempt: try trim
        let trimmed = key.trim();
        if trimmed != key
            && let Some(v) = lookup(trimmed)
        {
            return Some(v);
        }
        None
    }

    match current_language() {
        Language::En => {
            // Prefer keyed translations first (key-based system).
            if let Some(v) = keyed_en(key) {
                return v;
            }
            // Next try Fluent messages (FTL) for the current language (no args).
            if let Some(s) = format_for_lang(Language::En, key, &[] as &[&dyn Display]) {
                // Leak String to return a &'static str (intentionally persistent).
                return Box::leak(s.into_boxed_str());
            }
            // For dotted keys, enforce presence: fail-fast to force adding i18n when strict mode enabled.
            if key.contains('.') && i18n_strict_enabled() {
                panic!(
                    "Missing i18n key/message for '{}'. Add a keyed translation or an FTL message for the current language (en).",
                    key
                );
            }
            // Otherwise fall back to literal conversions (e.g., full-width colon variants).
            tr_try_with_colon_variants(key, zh_to_en).map_or(key, |en| en)
        }
        Language::Zh => {
            // Prefer keyed translations first (key-based system).
            if let Some(v) = keyed_zh(key) {
                return v;
            }
            // Try Fluent messages (FTL) for the current language.
            if let Some(s) = format_for_lang(Language::Zh, key, &[] as &[&dyn Display]) {
                return Box::leak(s.into_boxed_str());
            }
            // For dotted keys, enforce presence: fail-fast to force adding i18n when strict mode enabled.
            if key.contains('.') && i18n_strict_enabled() {
                panic!(
                    "缺少 i18n 键/消息 '{}'. 请为当前语言 (zh) 添加一个 keyed 翻译或 FTL 消息。",
                    key
                );
            }
            // Otherwise fall back to literal-based conversion (en->zh).
            tr_try_with_colon_variants(key, en_to_zh).map_or(key, |zh| zh)
        }
    }
}

/// Convenience formatting helper (callable in code). It takes a translation
/// `template_key` and format args (like `format!`) and returns the formatted
/// string in the active language.
///
/// Example:
///   let s = tr_fmt("Building module: {} v{}", &module_id, &version);
pub fn tr_fmt(template_key: &str, args: &[&dyn Display]) -> String {
    // Fluent-first formatting (hybrid migration).
    // If a Fluent message exists for the key in the current language, prefer it.
    if let Some(s) = format_for_current_lang(template_key, args) {
        return s;
    }

    // If this looks like a dotted key (i18n key) and no Fluent message was found,
    // require a keyed translation to exist; otherwise abort to force authors to
    // add the i18n entry (fail-fast) when strict mode is enabled.
    if template_key.contains('.') && i18n_strict_enabled() {
        match current_language() {
            Language::En => {
                if keyed_en(template_key).is_none() {
                    panic!(
                        "Missing i18n for key '{}' (en): no Fluent message and no keyed translation. Add one to src/locales/en-US/main.ftl or the keyed maps.",
                        template_key
                    );
                }
            }
            Language::Zh => {
                if keyed_zh(template_key).is_none() {
                    panic!(
                        "缺少 i18n 键 '{}'（zh）：没有 Fluent 消息或 keyed 翻译，请添加到 src/locales/zh-CN/main.ftl 或 keyed 地图。",
                        template_key
                    );
                }
            }
        }
    }

    let tmpl = tr_key(template_key);
    // Very simple formatting: we just pass into `format!` by building a string.
    // Since we can't expand varargs at runtime, we use an intermediate approach:
    // - We replace `{}` in the template sequentially with `Display::to_string`
    //   of args. This is intentionally simple (matches how `{}` placeholders are
    //   used in the repository).
    let mut out = String::new();
    let mut remaining = tmpl;
    for arg in args {
        if let Some(pos) = remaining.find("{}") {
            out.push_str(&remaining[..pos]);
            out.push_str(&format!("{}", arg));
            remaining = &remaining[pos + 2..];
        } else {
            // no more placeholders, append the rest and break
            out.push_str(remaining);
            remaining = "";
            break;
        }
    }
    out.push_str(remaining);
    out
}

fn i18n_strict_enabled() -> bool {
    std::env::var("KAM_I18N_STRICT")
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false)
}

/// Helper for one-off translation. Fancy macros are provided (trf) but some
/// code paths may prefer calling this directly.
fn with_single_display_arg<R, F>(arg: &dyn Display, f: F) -> R
where
    F: FnOnce(&[&dyn Display]) -> R,
{
    let arr = [arg];
    f(&arr)
}

pub fn tr_fmt_single<T: Display>(template_key: &str, arg: T) -> String {
    // Avoid heap allocation by creating a small stack array and invoking tr_fmt
    // while the array is still in scope.
    let a: &dyn Display = &arg;
    with_single_display_arg(a, |s| tr_fmt(template_key, s))
}

// A map-backed keyed translation system with a minimal fallback to the previous
// literal match-based behavior (for backwards compatibility).
//
// The translation file loader reads TOML resources embedded at compile time
// (`src/i18n/en.toml` and `src/i18n/zh.toml`) and flattens them into a simple
// string map (keys like `workspace.summary.title`). Maps are cached in static
// `OnceLock` containers so lookups are fast and thread-safe.
// TOML-based runtime overrides were removed during the migration to Fluent (.ftl).
// If TOML import/parsing is required in the future, implement it as a small,
// focused utility (script or dedicated module) outside the runtime hot path.

const fn try_load_runtime_i18n() {
    // No-op: this project now treats FTL files under `src/locales/<lang>/main.ftl`
    // as the canonical translation source. Older TOML-based override logic has
    // been removed as part of the migration.
    //
    // If a runtime override is needed, use `KAM_LOCALES_DIR` that contains
    // `<lang>/main.ftl` files. The Fluent loader will consult those at runtime.
}

fn keyed_en_map() -> &'static HashMap<String, String> {
    // TOML-based keyed maps have been deprecated in favor of Fluent (.ftl).
    // Keep an empty runtime map here so the old `keyed_en` fallback will not
    // attempt to load TOML resources.
    KEYED_EN.get_or_init(HashMap::new)
}

fn keyed_zh_map() -> &'static HashMap<String, String> {
    // TOML-based keyed maps have been deprecated in favor of Fluent (.ftl).
    // Keep an empty runtime map here; inline zh fallbacks (keyed_zh) remain.
    KEYED_ZH.get_or_init(HashMap::new)
}

fn keyed_en(key: &str) -> Option<&'static str> {
    // Prefer keyed translations from the TOML file.
    if let Some(v) = keyed_en_map().get(key) {
        return Some(v.as_str());
    }
    // Fallback: existing inline match-based translation table.
    match key {
        "workspace.summary.title" => Some("✿ Workspace Build Summary ✿"),
        "table.header.module" => Some("Module"),
        "table.header.status" => Some("Status"),
        "status.success" => Some("✓ Success"),
        "status.failed" => Some("✗ Failed: {}"),
        "table.header.stat" => Some("Statistic"),
        "table.header.value" => Some("Value"),
        "table.stat.total" => Some("Total"),
        "table.stat.succeeded" => Some("Succeeded"),
        "table.stat.failed" => Some("Failed"),
        "table.stat.total_duration" => Some("Total Duration"),
        "build.packaging_artifacts" => Some("Packaging artifacts..."),
        // About command keys (legacy fallback)
        "about.author" => Some("Author"),
        "about.email" => Some("Email"),
        "about.developer" => Some("Developer"),
        "about.description" => Some("Description"),
        "about.repository" => Some("Repository"),
        "about.info.command_informational" => {
            Some("This command is informational only; it doesn't modify files or the registry.")
        }
        "about.info.use_other_commands" => {
            Some("Use other commands (e.g., `kam init`, `kam build`) to perform actions.")
        }
        "about.thanks" => Some("Thanks for using Kam"),
        "about.enjoy" => Some("Enjoy your module tooling experience!"),
        "about.powered" => Some("Powered by the Kam CLI — Happy building!"),

        // Auto-added missing keys to satisfy build-time i18n checks:
        "init.interactive.path_to_create_project" => Some("Path to create project"),
        "config.interactive.select_target" => Some("Select target"),
        "packaging.packaging" => Some("Packaging..."),
        "secret.saved" => Some("Secret saved"),
        "config.interactive.no_key_entered" => Some("No key entered"),
        "init.interactive.cz_not_found" => Some("Commitizen (cz) not found"),
        "config.interactive.enter_ui_language" => Some("Enter UI language"),
        "install.cli_not_found" => Some("Install CLI not found"),
        "tmpl.use_import_command" => Some("Use import command"),
        "install.executing" => Some("Executing installation..."),
        "secret.interactive.title" => Some("Secrets"),
        "termux.ssh.interactive.starting" => Some("Starting Termux SSH integration..."),
        "cache.modules.removed_module_cache" => Some("Removed module cache"),
        "termux.ssh.interactive.create_new_key_prompt" => Some("Create a new SSH key?"),
        "env.docs.header" => Some("Environment Documentation"),
        "init.interactive.enter_local_template_path" => Some("Enter path to local template"),
        "init.interactive.preview_file_empty" => Some("Preview file is empty"),
        "init.interactive.recommend_github_cli" => {
            Some("It's recommended to install the GitHub CLI")
        }
        "init.interactive.template_directory_empty" => Some("Template directory is empty"),
        "secret.no_valid_cert_in_chain" => Some("No valid certificate in chain"),
        "common.file_fixed" => Some("File fixed"),
        "init.interactive.preview_header" => Some("Preview header"),
        "config.interactive.exit" => Some("Exit"),
        "validate.mmrl.repo.changelog_file" => Some("Changelog file"),
        "termux.ssh.interactive.create_new_key_missing_tool" => Some("Tool missing to create key"),
        "config.interactive.no_change" => Some("No change detected"),
        "termux.ssh.interactive.pushed_to_sdcard" => Some("Pushed to SD card"),
        "repo.invalid_input_number" => Some("Invalid number entered"),
        "hooks.skipping_hooks_for_template_packaging" => {
            Some("Skipping hooks for template packaging")
        }
        "export.config_json" => Some("Exported config JSON"),
        "Default source directory '{}' does not exist." => {
            Some("Default source directory '{}' does not exist.")
        }
        "init.interactive.module_id_mismatch" => Some("Module ID mismatch"),
        "secret.exported" => Some("Secret exported"),
        "secret.interactive.enter_file_path" => Some("Enter file path"),
        "validate.prop.author_empty_recommended" => Some("Author is empty; recommended to set one"),
        "secret.failed_retrieve_public_key" => Some("Failed to retrieve public key"),
        "check.warnings.header" => Some("Warnings"),
        "cache.modules.no_index_files" => Some("No index files found in cache"),
        "repo.selected_module" => Some("Selected module: {}"),
        "validate.prop.id_required" => Some("ID is required"),
        "install.section" => Some("Install"),
        "termux.ssh.interactive.enter_private_key_path" => Some("Enter private key path"),
        "termux.ssh.interactive.adb_push_failed" => Some("ADB push failed"),
        "sign.no_src_or_dist" => Some("No source or distribution found to sign"),
        "init.interactive.author" => Some("Author"),
        "cache.local_cached_templates" => Some("Local cached templates"),
        "init.interactive.use_sanitized_module_id" => Some("Use sanitized module ID"),
        "config.interactive.press_enter" => Some("Press Enter to continue"),
        "repo.invalid_selection_out_of_range" => Some("Selection out of range"),
        "install.package_not_found" => Some("Package not found"),
        "termux.ssh.connecting" => Some("Connecting (Termux SSH)..."),
        "init.interactive.completed_successfully" => Some("Completed successfully"),
        "secret.interactive.choose_input_method" => Some("Choose input method"),
        "init.interactive.saved_base_values" => Some("Saved base values"),
        "init.interactive.preview_end" => Some("End of preview"),
        "repo.module_detail.asset" => Some("Asset: {}"),
        "init.interactive.title" => Some("Initialize"),
        "termux.ssh.pushed_key" => Some("Pushed key"),
        "secret.no_cert_chain_in_issue" => Some("No valid certificate chain in issue"),
        "check.no_issues_found" => Some("No issues found"),
        "build.no_workspace_members" => Some("No workspace members found"),
        "init.interactive.invalid_selection_index" => Some("Invalid selection index"),
        "secret.interactive.select_option" => Some("Select an option"),
        "init.interactive.select_by_name_or_number" => Some("Select by name or number"),
        "secret.interactive.menu.add" => Some("Add secret"),
        "validate.no_issues_kam_toml_valid" => Some("No issues: kam.toml is valid"),
        "build.concurrent_building_members" => Some("Building workspace members concurrently"),
        "secret.stored_secrets" => Some("Stored secrets"),
        "secret.interactive.menu.exit" => Some("Exit menu"),
        "termux.ssh.interactive.ssh_copy_id_attempt" => Some("Attempting ssh-copy-id..."),
        "validate.passed_with_warnings" => Some("Validation passed with warnings"),
        "init.interactive.custom_value_choice" => Some("Choose a custom value"),
        "secret.interactive.menu.list" => Some("List secrets"),
        "build.invalid_glob_pattern" => Some("Invalid glob pattern"),
        "init.interactive.gh_not_found" => Some("GitHub CLI not found"),
        "secret.interactive.storage.file" => Some("Secret storage: file"),
        "termux.ssh.auto_no_pubkey" => Some("No public key found (auto)"),
        "termux.ssh.interactive.choose_key_or_create" => {
            Some("Choose an existing key or create a new one")
        }
        "validate.failed" => Some("Validation failed"),
        "secret.failed_fetch_ca" => Some("Failed to fetch CA certificate"),
        "init.interactive.enter_true_false_for" => Some("Enter true/false for {}"),
        "termux.ssh.auto_connecting" => Some("Automatically connecting to SSH"),
        "config.interactive.cancel" => Some("Cancel"),
        "Unable to determine install CLI. Please set 'root.manager' in ~/.kam/config.toml or pass --manager" => {
            Some(
                "Unable to determine install CLI. Please set 'root.manager' in ~/.kam/config.toml or pass --manager",
            )
        }
        "init.interactive.end_of_directory_preview" => Some("End of directory preview"),
        "validate.prop.version_code_should_be_positive" => Some("Version code should be positive"),
        "hooks.malformed_env_line" => Some("Malformed environment line in hooks"),
        "secret.interactive.summary_added" => Some("Summary added"),
        "config.interactive.title" => Some("Config"),
        "this.key.does.not.exist" => Some("This key does not exist"),
        "repo.download" => Some("Downloading {}..."),
        "config.interactive.aborted" => Some("Aborted"),
        "validate.prop.version_required" => Some("Version is required"),
        "repo.result_line" => Some("{} — {} {}"),
        "config.interactive.show_current_config" => Some("Show current configuration"),
        "secret.interactive.confirm_encryption_password" => Some("Confirm encryption password"),
        "repo.index_force_synced" => Some("Index force-synced: {} -> {}"),
        "init.interactive.set_module_id_to_basename" => Some("Set module ID to basename"),
        "build.skipping_no_kam_toml" => Some("Skipping build: no kam.toml"),
        "init.interactive.inferred_id_invalid" => Some("Inferred module ID is invalid"),
        "init.interactive.description" => Some("Description"),
        "env.docs.kam_tmpl_note" => Some("Note about 'kam tmpl'"),
        "secret.written_to" => Some("Secret written to {}"),
        "check.some_issues_found" => Some("Some issues were found"),
        "hooks.skipping_template_packaging" => Some("Skipping template packaging due to hooks"),
        "cache.modules.directory_empty_or_not_exists" => {
            Some("Module cache directory empty or does not exist")
        }
        "hooks.running_pre" => Some("Running pre-hooks"),
        "termux.ssh.forwarded" => Some("Port forwarded"),
        "secret.interactive.input_method.file" => Some("File input method"),
        "secret.cert_chain_imported" => Some("Certificate chain imported"),
        "cache.modules.cleaned_successfully" => Some("Module cache cleaned successfully"),
        "packaging.source_directory_not_found" => Some("Source directory not found"),
        "validate.warnings.header" => Some("Validation Warnings"),
        "build.complete" => Some("Build complete"),
        "cache.template_added" => Some("Template added to cache"),
        "repo.index_synced" => Some("Repository index synced: {}"),
        "config.interactive.error.no_subcommand" => Some("Error: no subcommand provided"),
        "init.interactive.variable_note" => Some("Variable note"),
        "init.interactive.enter_value_for" => Some("Enter value for {}"),
        "config.interactive.enter_custom_key" => Some("Enter custom key"),
        "build.failed_to_build" => Some("Failed to build"),
        "config.set_success" => Some("Configuration set successfully"),
        "termux.ssh.pubkey_missing" => Some("SSH public key missing"),
        "secret.root_ca_removed" => Some("Root CA removed"),
        "config.interactive.make_more_changes" => Some("Make more changes?"),
        "init.interactive.example" => Some("Example"),
        "tmpl.no_templates_in_cache" => Some("No templates in cache"),
        "secret.confirm_remove" => Some("Confirm removal"),
        "termux.ssh.interactive.create_new_key_failed" => Some("Failed to create new key"),
        "init.interactive.loaded_template_from_temp" => Some("Loaded template from temporary path"),
        "repo.module_detail.release" => Some("Release: {}"),
        "config.interactive.error.conflict_with_subcommand" => {
            Some("Error: conflict with subcommand")
        }
        "init.interactive.preview_exit" => Some("Preview exit"),
        "config.note_custom_keys" => Some("Note about custom keys"),
        "export.module_prop" => Some("Export module properties"),
        "init.interactive.template_variables" => Some("Template variables"),
        "config.interactive.choose_root_manager" => Some("Choose root manager"),
        "build.building_workspace_member" => Some("Building workspace member"),
        "tmpl.export.no_templates_specified" => Some("No templates specified for export"),
        "build.workspace_member_not_found" => Some("Workspace member not found"),
        "config.interactive.select_option" => Some("Select an option"),
        "init.interactive.preview_failed_read_file" => Some("Failed to read preview file"),
        "cache.directory_empty_or_not_exists" => Some("Cache directory empty or does not exist"),
        "Verifying signature..." => Some("Verifying signature..."),
        "cache.cleaned_successfully" => Some("Cache cleaned successfully"),
        "export.module_json" => Some("Export module JSON"),
        "init.interactive.enter_true_or_false" => Some("Enter true or false"),
        "termux.ssh.ssh_failed" => Some("SSH failed"),
        "termux.ssh.interactive.create_new_key_success" => Some("Created new key successfully"),
        "cli.long_about" => Some("Long about description"),
        "init.interactive.helper_script" => Some("Helper script"),
        "secret.interactive.invalid_selection" => Some("Invalid selection"),
        "repo.skipped_selection" => Some("Selection skipped"),
        "secret.fetching_cert_from_github" => Some("Fetching certificate(s) from GitHub"),
        "packaging.success_module_built" => Some("Module successfully built"),
        "secret.interactive.no_file_entered" => Some("No file entered"),
        "init.interactive.enter_custom_value_for" => Some("Enter custom value for {}"),
        "init.interactive.non_interactive_fallback" => Some("Using non-interactive fallback"),
        "init.interactive.preview_cancelled" => Some("Preview cancelled"),
        "cache.modules.detail_cache" => Some("Module detail cache"),
        "project.output_directory" => Some("Project output directory"),
        "build.skipping_failed_load_kam_toml" => Some("Skipping build: failed to load kam.toml"),
        "export.update_json" => Some("Update JSON"),
        "init.interactive.default" => Some("Default"),
        "secret.removed" => Some("Secret removed"),
        "secret.interactive.aborted" => Some("Secret action aborted"),
        "config.unset_success" => Some("Config unset successfully"),
        "init.interactive.template_preview" => Some("Template preview"),
        "secret.interactive.error.password_mismatch" => Some("Password mismatch"),
        "config.interactive.choose_action" => Some("Choose action"),
        "cache.modules.removed" => Some("Removed modules from cache"),
        "config.interactive.menu.set_ui_language" => Some("Set UI language"),
        "init.interactive.invalid_selection" => Some("Invalid selection"),
        "secret.interactive.file_not_found" => Some("File not found"),
        "termux.ssh.interactive.connecting" => Some("Connecting..."),
        "secret.interactive.summary" => Some("Summary"),
        "init.interactive.enter_value_for_index_or_value" => Some("Enter value for index or value"),
        "hooks.invalid_env_variable_name" => Some("Invalid environment variable name in hooks"),
        "export.track_json" => Some("Track JSON export"),
        "secret.interactive.no_value_entered" => Some("No value entered"),
        "config.interactive.menu.set_root_manager" => Some("Set root manager"),
        "init.interactive.value_required" => Some("Value required"),
        "init.interactive.variable" => Some("Variable"),
        "secret.interactive.menu.get" => Some("Get secret"),
        "init.interactive.choice_pull_default_templates" => Some("Pull default templates?"),
        "repo.similar_packages_header" => Some("Similar packages ({} results) for \"{}\""),
        "sign.failed_to_sign" => Some("Failed to sign"),
        "termux.ssh.ssh_exited" => Some("SSH exited"),
        "termux.ssh.forward_failed" => Some("SSH forward failed"),
        "config.builtin_keys" => Some("Built-in config keys"),
        "termux.ssh.remote_mkdir_failed" => Some("Remote mkdir failed"),
        "init.interactive.recommend_cz_install" => Some("Recommend installing commitizen (cz)"),
        "secret.interactive.choose_action" => Some("Choose action"),
        "init.interactive.confirm_proceed_create" => Some("Confirm to proceed with creation"),
        "secret.interactive.input_method.direct" => Some("Direct input method"),
        "termux.ssh.interactive.scp_fallback" => Some("Using SCP fallback"),
        "termux.ssh.interactive.scp_failed" => Some("SCP failed"),
        "termux.ssh.setup_step4" => Some("Termux SSH setup: step 4"),
        "termux.ssh.setup_note" => Some("Termux SSH setup note"),
        "secret.interactive.menu.remove" => Some("Remove secret"),
        "termux.ssh.push_failed" => Some("Push failed"),
        "init.interactive.choice_local_path" => Some("Choose local path"),
        "init.interactive.press_enter" => Some("Press Enter"),
        "init.interactive.module_id_prompt" => Some("Module ID:"),
        "config.interactive.view_builtins" => Some("View builtins"),
        "termux.ssh.setup_step2" => Some("Termux SSH setup: step 2"),
        "termux.ssh.setup_step3" => Some("Termux SSH setup: step 3"),
        "init.interactive.value_is_not_choice_prompt" => Some("Value is not one of the choices"),
        "secret.error.no_subcommand" => Some("Secret error: no subcommand provided"),
        "termux.ssh.setup_instructions" => Some("Termux SSH setup instructions"),
        "project.header" => Some("Project"),
        "repo.everything_up_to_date" => Some("Everything up to date"),
        "secret.interactive.yes" => Some("Yes"),
        "hooks.running_post" => Some("Running post-hooks"),
        "Install CLI '{}' not found on PATH. Please install it or set 'root.manager' in ~/.kam/config.toml" => {
            Some(
                "Install CLI '{}' not found on PATH. Please install it or set 'root.manager' in ~/.kam/config.toml",
            )
        }
        "env.docs.intro" => Some("Environment docs: introduction"),
        "cache.modules.no_matching_cache_file" => Some("No matching cache file found"),
        "termux.ssh.interactive.gen_hint" => Some("Key generation hint"),
        "init.interactive.save_base_values" => Some("Save base values?"),
        "repo.prompt.enter_number" => Some("Enter a number:"),
        "init.interactive.version" => Some("Version"),
        "termux.ssh.ssh_missing" => Some("SSH is missing"),
        "init.interactive.aborted" => Some("Aborted"),
        "termux.ssh.interactive.key_installed" => Some("Key installed"),
        "init.interactive.enter_path_to_local_template" => Some("Enter path to local template"),
        "packaging.using_existing_module_prop_from_hook" => {
            Some("Using existing module prop from hook")
        }
        "secret.interactive.enter_name" => Some("Enter name"),
        "secret.interactive.cancel" => Some("Cancel"),
        "validate.mmrl.repo.readme_file" => Some("README file"),
        "init.interactive.invalid_module_id" => Some("Invalid module ID"),
        "repo.module_detail.download_url" => Some("Download URL"),
        "termux.ssh.interactive.create_new_key_option" => Some("Create a new key (option)"),
        "project.build_time" => Some("Build time"),
        "secret.interactive.encryption_password" => Some("Encryption password"),
        "packaging.files" => Some("Packaging files"),
        "install.installed" => Some("Installed"),
        "init.interactive.next_steps" => Some("Next steps"),
        "validate.mmrl.repo.license_recommended" => Some("License recommended"),
        "init.interactive.preview_continue" => Some("Continue preview"),
        "termux.ssh.interactive.ssh_copy_id_failed" => Some("ssh-copy-id failed"),
        "sign.signed" => Some("Signed"),
        "termux.ssh.setup_step1" => Some("Termux SSH setup: step 1"),
        "tmpl.export.no_templates_exported" => Some("No templates exported"),
        "init.interactive.preview_another_file" => Some("Preview another file"),
        "repo.module_not_found_showing_similar" => {
            Some("Module not found; showing similar results")
        }
        "init.interactive.project_name" => Some("Project name"),
        "secret.interactive.no_name_entered" => Some("No name entered"),
        "project.package_size" => Some("Package size"),
        "init.interactive.preview_file_truncated" => Some("Preview file truncated"),
        "cli.about" => Some("About"),
        "init.interactive.preview_failed_read_dir" => Some("Failed to read preview directory"),
        "export.repo_json" => Some("Export repo JSON"),
        "cache.modules.index_files" => Some("Module index files"),
        "packaging.generating_module_prop" => Some("Generating module property"),
        "secret.interactive.confirm_before_add" => Some("Confirm before adding"),
        "config.interactive.global" => Some("Global"),
        "repo.module_detail.title" => Some("Module details: {}"),
        "secret.imported" => Some("Secret imported"),
        "secret.no_secrets_stored" => Some("No secrets stored"),
        "config.interactive.invalid_selection" => Some("Invalid selection"),
        "config.interactive.enter_value_for_key" => Some("Enter value for key"),
        "secret.public_key_exported" => Some("Public key exported"),
        "install.su_failed" => Some("Failed to gain root privileges (su failed)"),
        "validate.prop.id_invalid_characters" => Some("ID contains invalid characters"),
        "validate.mmrl.repo.license_file" => Some("License file"),
        "termux.ssh.forward_failed_err" => Some("SSH forward failed: {}"),
        "init.interactive.and_more_files" => Some("and {} more files"),
        "cache.no_templates" => Some("No templates found in cache"),
        "validate.errors.header" => Some("Errors"),
        "install.trying_su" => Some("Trying to use su..."),
        "cache.template_removed" => Some("Template removed from cache"),
        "secret.fetching_root_ca" => Some("Fetching root CA..."),
        "termux.ssh.interactive.ask_username" => Some("Enter username for SSH"),
        "secret.failed_read_ca" => Some("Failed to read CA certificate"),
        "init.interactive.directory_preview_header" => Some("Directory preview"),
        "secret.interactive.enter_value" => Some("Enter value"),
        "build.failed_workspace_members" => Some("Some workspace members failed to build"),
        "cache.modules.index_entry" => Some("Cache index entry"),
        "build.no_workspace_section" => Some("No workspace section in kam.toml"),
        "init.interactive.template_contents_showing_up_to_files" => Some("Showing up to {} files"),
        "build.building_module_version" => Some("Building module version {}"),
        "Source directory '{}' does not exist. Build might fail or produce empty module." => {
            Some("Source directory '{}' does not exist. Build might fail or produce empty module.")
        }
        "env.no_kam_vars" => Some("No KAM environment variables found"),
        "secret.interactive.select_input_method" => Some("Select input method"),
        "init.interactive.enter_yes_no" => Some("Enter yes or no"),
        "init.interactive.choose_template" => Some("Choose a template"),
        "config.interactive.local_project_not_detected" => Some("Local project not detected"),
        "validate.prop.description_required" => Some("Description is required"),
        "packaging.success_template_built" => Some("Template built successfully"),
        "secret.interactive.confirm_overwrite" => Some("Confirm overwrite?"),
        "validate.prop.name_required" => Some("Name is required"),
        "init.interactive.select_value_for" => Some("Select value for {}"),
        "config.interactive.local_project_detected" => Some("Local project detected"),
        "secret.interactive.intro" => Some("Secret: introduction"),
        "project.output_file" => Some("Output file"),
        "config.interactive.set_custom_key" => Some("Set a custom key"),
        "init.interactive.help" => Some("Help"),
        "secret.root_ca_added" => Some("Root CA added"),
        "check.errors.header" => Some("Errors"),
        "secret.interactive.error.conflict_with_subcommand" => {
            Some("Error: conflict with subcommand")
        }
        "config.example" => Some("Config example"),
        "secret.interactive.storage.keyring" => Some("Keyring storage"),
        _ => None,
    }
}

fn keyed_zh(key: &str) -> Option<&'static str> {
    // Prefer keyed translations from the TOML file.
    if let Some(v) = keyed_zh_map().get(key) {
        return Some(v.as_str());
    }
    // Fallback: existing inline match-based translations
    match key {
        "workspace.summary.title" => Some("✿ 工作区构建摘要 ✿"),
        "table.header.module" => Some("模块"),
        "table.header.status" => Some("状态"),
        "status.success" => Some("✓ 成功"),
        "status.failed" => Some("✗ 失败: {}"),
        "table.header.stat" => Some("统计项"),
        "table.header.value" => Some("值"),
        "table.stat.total" => Some("总计"),
        "table.stat.succeeded" => Some("成功"),
        "table.stat.failed" => Some("失败"),
        "table.stat.total_duration" => Some("总耗时"),
        "build.packaging_artifacts" => Some("正在打包制品..."),
        // About command keys (legacy fallback)
        "about.author" => Some("作者"),
        "about.email" => Some("邮箱"),
        "about.developer" => Some("开发者"),
        "about.description" => Some("描述"),
        "about.repository" => Some("仓库"),
        "about.info.command_informational" => Some("此命令仅供参考，不会修改文件或注册表。"),
        "about.info.use_other_commands" => {
            Some("请使用其他命令（例如 `kam init`, `kam build`）执行实际操作。")
        }
        "about.thanks" => Some("感谢使用 Kam"),
        "about.enjoy" => Some("祝您使用愉快！"),
        "about.powered" => Some("由 Kam CLI 提供支持 — 祝构建顺利！"),

        // Auto-added missing keys (zh fallbacks)
        "init.interactive.path_to_create_project" => Some("创建项目的路径"),
        "config.interactive.select_target" => Some("选择目标"),
        "packaging.packaging" => Some("打包中..."),
        "secret.saved" => Some("密钥已保存"),
        "config.interactive.no_key_entered" => Some("未输入密钥"),
        "init.interactive.cz_not_found" => Some("未找到 commitizen (cz)"),
        "config.interactive.enter_ui_language" => Some("请输入界面语言"),
        "install.cli_not_found" => Some("未找到安装命令行工具"),
        "tmpl.use_import_command" => Some("使用 import 命令"),
        "install.executing" => Some("正在执行安装..."),
        "secret.interactive.title" => Some("密钥"),
        "termux.ssh.interactive.starting" => Some("正在启动 Termux SSH 集成..."),
        "cache.modules.removed_module_cache" => Some("已移除模块缓存"),
        "termux.ssh.interactive.create_new_key_prompt" => Some("是否创建新 SSH 密钥？"),
        "env.docs.header" => Some("环境文档"),
        "init.interactive.enter_local_template_path" => Some("输入本地模板路径"),
        "init.interactive.preview_file_empty" => Some("预览文件为空"),
        "init.interactive.recommend_github_cli" => Some("推荐安装 GitHub CLI"),
        "init.interactive.template_directory_empty" => Some("模板目录为空"),
        "secret.no_valid_cert_in_chain" => Some("证书链中没有有效证书"),
        "common.file_fixed" => Some("文件已修复"),
        "init.interactive.preview_header" => Some("预览"),
        "config.interactive.exit" => Some("退出"),
        "validate.mmrl.repo.changelog_file" => Some("变更日志文件"),
        "termux.ssh.interactive.create_new_key_missing_tool" => Some("缺少创建密钥的工具"),
        "config.interactive.no_change" => Some("未检测到更改"),
        "termux.ssh.interactive.pushed_to_sdcard" => Some("已推送到 SD 卡"),
        "repo.invalid_input_number" => Some("无效的数字输入"),
        "hooks.skipping_hooks_for_template_packaging" => Some("为模板打包跳过 hooks"),
        "export.config_json" => Some("导出配置 JSON"),
        "Default source directory '{}' does not exist." => Some("默认源目录 '{}' 不存在。"),
        "init.interactive.module_id_mismatch" => Some("模块 ID 不匹配"),
        "secret.exported" => Some("密钥已导出"),
        "secret.interactive.enter_file_path" => Some("输入文件路径"),
        "validate.prop.author_empty_recommended" => Some("作者为空，建议填写"),
        "secret.failed_retrieve_public_key" => Some("获取公钥失败"),
        "check.warnings.header" => Some("警告"),
        "cache.modules.no_index_files" => Some("缓存中没有索引文件"),
        "repo.selected_module" => Some("已选择模块：{}"),
        "validate.prop.id_required" => Some("需要 ID"),
        "install.section" => Some("安装"),
        "termux.ssh.interactive.enter_private_key_path" => Some("输入私钥路径"),
        "termux.ssh.interactive.adb_push_failed" => Some("ADB 推送失败"),
        "sign.no_src_or_dist" => Some("未找到要签名的源码或分发包"),
        "init.interactive.author" => Some("作者"),
        "cache.local_cached_templates" => Some("本地缓存的模板"),
        "init.interactive.use_sanitized_module_id" => Some("使用已清理的模块 ID"),
        "config.interactive.press_enter" => Some("按回车继续"),
        "repo.invalid_selection_out_of_range" => Some("选择超出范围"),
        "install.package_not_found" => Some("未找到软件包"),
        "termux.ssh.connecting" => Some("正在连接 (Termux SSH)..."),
        "init.interactive.completed_successfully" => Some("已成功完成"),
        "secret.interactive.choose_input_method" => Some("选择输入方法"),
        "init.interactive.saved_base_values" => Some("已保存基础值"),
        "init.interactive.preview_end" => Some("预览结束"),
        "repo.module_detail.asset" => Some("资源：{}"),
        "init.interactive.title" => Some("初始化"),
        "termux.ssh.pushed_key" => Some("已推送密钥"),
        "secret.no_cert_chain_in_issue" => Some("发布中无证书链"),
        "check.no_issues_found" => Some("未发现问题"),
        "build.no_workspace_members" => Some("未定义工作区成员"),
        "init.interactive.invalid_selection_index" => Some("无效的选择索引"),
        "secret.interactive.select_option" => Some("选择一个选项"),
        "init.interactive.select_by_name_or_number" => Some("按名称或编号选择"),
        "secret.interactive.menu.add" => Some("添加密钥"),
        "validate.no_issues_kam_toml_valid" => Some("无问题：kam.toml 有效"),
        "build.concurrent_building_members" => Some("并发构建工作区成员"),
        "secret.stored_secrets" => Some("已存储的密钥"),
        "secret.interactive.menu.exit" => Some("退出菜单"),
        "termux.ssh.interactive.ssh_copy_id_attempt" => Some("正在尝试 ssh-copy-id..."),
        "validate.passed_with_warnings" => Some("校验通过，但有警告"),
        "init.interactive.custom_value_choice" => Some("自定义值选择"),
        "secret.interactive.menu.list" => Some("列出密钥"),
        "build.invalid_glob_pattern" => Some("无效的 glob 模式"),
        "init.interactive.gh_not_found" => Some("未找到 GitHub CLI"),
        "secret.interactive.storage.file" => Some("存储方式：文件"),
        "termux.ssh.auto_no_pubkey" => Some("未找到公钥（自动模式）"),
        "termux.ssh.interactive.choose_key_or_create" => Some("选择密钥或创建一个"),
        "validate.failed" => Some("验证失败"),
        "secret.failed_fetch_ca" => Some("获取 CA 失败"),
        "init.interactive.enter_true_false_for" => Some("为 {} 输入 true/false"),
        "termux.ssh.auto_connecting" => Some("自动连接 SSH"),
        "config.interactive.cancel" => Some("取消"),
        "Unable to determine install CLI. Please set 'root.manager' in ~/.kam/config.toml or pass --manager" => {
            Some("无法确定安装 CLI。请在 ~/.kam/config.toml 中设置 'root.manager' 或传入 --manager")
        }
        "init.interactive.end_of_directory_preview" => Some("目录预览结束"),
        "validate.prop.version_code_should_be_positive" => Some("版本号应为正数"),
        "hooks.malformed_env_line" => Some("hooks 中的环境变量行格式错误"),
        "secret.interactive.summary_added" => Some("摘要已添加"),
        "config.interactive.title" => Some("配置"),
        "this.key.does.not.exist" => Some("该键不存在"),
        "repo.download" => Some("正在下载 {}..."),
        "config.interactive.aborted" => Some("已中止"),
        "validate.prop.version_required" => Some("需要版本号"),
        "repo.result_line" => Some("{} — {} {}"),
        "config.interactive.show_current_config" => Some("显示当前配置"),
        "secret.interactive.confirm_encryption_password" => Some("确认加密密码"),
        "repo.index_force_synced" => Some("索引强制同步：{} -> {}"),
        "init.interactive.set_module_id_to_basename" => Some("将模块 ID 设置为基目录名"),
        "build.skipping_no_kam_toml" => Some("跳过构建：未找到 kam.toml"),
        "init.interactive.inferred_id_invalid" => Some("推断的模块 ID 无效"),
        "init.interactive.description" => Some("描述"),
        "env.docs.kam_tmpl_note" => Some("'kam tmpl' 说明"),
        "secret.written_to" => Some("密钥写入到 {}"),
        "check.some_issues_found" => Some("发现一些问题"),
        "hooks.skipping_template_packaging" => Some("跳过模板打包（hooks）"),
        "cache.modules.directory_empty_or_not_exists" => Some("模块缓存目录为空或不存在"),
        "hooks.running_pre" => Some("正在运行 pre-hooks"),
        "termux.ssh.forwarded" => Some("端口已转发"),
        "secret.interactive.input_method.file" => Some("文件输入方式"),
        "secret.cert_chain_imported" => Some("证书链已导入"),
        "cache.modules.cleaned_successfully" => Some("模块缓存清理成功"),
        "packaging.source_directory_not_found" => Some("未找到源目录"),
        "validate.warnings.header" => Some("校验警告"),
        "build.complete" => Some("构建完成"),
        "cache.template_added" => Some("模板已添加到缓存"),
        "repo.index_synced" => Some("仓库索引已同步：{}"),
        "config.interactive.error.no_subcommand" => Some("错误：未提供子命令"),
        "init.interactive.variable_note" => Some("变量说明"),
        "init.interactive.enter_value_for" => Some("输入 {} 的值"),
        "config.interactive.enter_custom_key" => Some("输入自定义键"),
        "build.failed_to_build" => Some("构建失败"),
        "config.set_success" => Some("配置设置成功"),
        "termux.ssh.pubkey_missing" => Some("缺少 SSH 公钥"),
        "secret.root_ca_removed" => Some("根 CA 已移除"),
        "config.interactive.make_more_changes" => Some("是否进行更多更改？"),
        "init.interactive.example" => Some("示例"),
        "tmpl.no_templates_in_cache" => Some("缓存中没有模板"),
        "secret.confirm_remove" => Some("确认删除"),
        "termux.ssh.interactive.create_new_key_failed" => Some("创建新密钥失败"),
        "init.interactive.loaded_template_from_temp" => Some("从临时目录加载模板"),
        "repo.module_detail.release" => Some("发布：{}"),
        "config.interactive.error.conflict_with_subcommand" => Some("错误：与子命令冲突"),
        "init.interactive.preview_exit" => Some("退出预览"),
        "config.note_custom_keys" => Some("关于自定义键的说明"),
        "export.module_prop" => Some("导出模块属性"),
        "init.interactive.template_variables" => Some("模板变量"),
        "config.interactive.choose_root_manager" => Some("选择根管理器"),
        "build.building_workspace_member" => Some("正在构建工作区成员"),
        "tmpl.export.no_templates_specified" => Some("未指定要导出的模板"),
        "build.workspace_member_not_found" => Some("未找到工作区成员"),
        "config.interactive.select_option" => Some("选择一个选项"),
        "init.interactive.preview_failed_read_file" => Some("读取预览文件失败"),
        "cache.directory_empty_or_not_exists" => Some("缓存目录为空或不存在"),
        "Verifying signature..." => Some("正在验证签名..."),
        "cache.cleaned_successfully" => Some("缓存清理成功"),
        "export.module_json" => Some("导出模块 JSON"),
        "init.interactive.enter_true_or_false" => Some("输入 true 或 false"),
        "termux.ssh.ssh_failed" => Some("SSH 失败"),
        "termux.ssh.interactive.create_new_key_success" => Some("创建新密钥成功"),
        "cli.long_about" => Some("长描述"),
        "init.interactive.helper_script" => Some("辅助脚本"),
        "secret.interactive.invalid_selection" => Some("无效的选择"),
        "repo.skipped_selection" => Some("跳过选择"),
        "secret.fetching_cert_from_github" => Some("正在从 GitHub 获取证书"),
        "packaging.success_module_built" => Some("模块构建成功"),
        "secret.interactive.no_file_entered" => Some("未输入文件"),
        "init.interactive.enter_custom_value_for" => Some("输入 {} 的自定义值"),
        "init.interactive.non_interactive_fallback" => Some("使用非交互回退方案"),
        "init.interactive.preview_cancelled" => Some("预览已取消"),
        "cache.modules.detail_cache" => Some("模块详情缓存"),
        "project.output_directory" => Some("输出目录"),
        "build.skipping_failed_load_kam_toml" => Some("跳过构建：加载 kam.toml 失败"),
        "export.update_json" => Some("更新 JSON"),
        "init.interactive.default" => Some("默认"),
        "secret.removed" => Some("密钥已移除"),
        "secret.interactive.aborted" => Some("密钥操作已中止"),
        "config.unset_success" => Some("配置已取消设置"),
        "init.interactive.template_preview" => Some("模板预览"),
        "secret.interactive.error.password_mismatch" => Some("密码不匹配"),
        "config.interactive.choose_action" => Some("选择操作"),
        "cache.modules.removed" => Some("已移除的模块"),
        "config.interactive.menu.set_ui_language" => Some("设置界面语言"),
        "init.interactive.invalid_selection" => Some("无效选择"),
        "secret.interactive.file_not_found" => Some("未找到文件"),
        "termux.ssh.interactive.connecting" => Some("连接中..."),
        "secret.interactive.summary" => Some("摘要"),
        "init.interactive.enter_value_for_index_or_value" => Some("输入索引或值的值"),
        "hooks.invalid_env_variable_name" => Some("无效的环境变量名"),
        "export.track_json" => Some("跟踪 JSON 导出"),
        "secret.interactive.no_value_entered" => Some("未输入值"),
        "config.interactive.menu.set_root_manager" => Some("设置根管理器"),
        "init.interactive.value_required" => Some("需要值"),
        "init.interactive.variable" => Some("变量"),
        "secret.interactive.menu.get" => Some("获取密钥"),
        "init.interactive.choice_pull_default_templates" => Some("是否拉取默认模板？"),
        "repo.similar_packages_header" => Some("类似软件包（{} 条结果），查询 \"{}\""),
        "sign.failed_to_sign" => Some("签名失败"),
        "termux.ssh.ssh_exited" => Some("SSH 已退出"),
        "termux.ssh.forward_failed" => Some("SSH 转发失败"),
        "config.builtin_keys" => Some("内置配置键"),
        "termux.ssh.remote_mkdir_failed" => Some("远程创建目录失败"),
        "init.interactive.recommend_cz_install" => Some("建议安装 commitizen (cz)"),
        "secret.interactive.choose_action" => Some("选择操作"),
        "init.interactive.confirm_proceed_create" => Some("确认继续创建"),
        "secret.interactive.input_method.direct" => Some("直接输入模式"),
        "termux.ssh.interactive.scp_fallback" => Some("使用 SCP 回退"),
        "termux.ssh.interactive.scp_failed" => Some("SCP 失败"),
        "termux.ssh.setup_step4" => Some("Termux SSH 设置：第 4 步"),
        "termux.ssh.setup_note" => Some("Termux SSH 设置说明"),
        "secret.interactive.menu.remove" => Some("移除密钥"),
        "termux.ssh.push_failed" => Some("推送失败"),
        "init.interactive.choice_local_path" => Some("选择本地路径"),
        "init.interactive.press_enter" => Some("按回车"),
        "init.interactive.module_id_prompt" => Some("模块 ID："),
        "config.interactive.view_builtins" => Some("查看内置项"),
        "termux.ssh.setup_step2" => Some("Termux SSH 设置：第 2 步"),
        "termux.ssh.setup_step3" => Some("Termux SSH 设置：第 3 步"),
        "init.interactive.value_is_not_choice_prompt" => Some("值不是可选项"),
        "secret.error.no_subcommand" => Some("密钥错误：未提供子命令"),
        "termux.ssh.setup_instructions" => Some("Termux SSH 设置说明"),
        "project.header" => Some("项目"),
        "repo.everything_up_to_date" => Some("一切已是最新"),
        "secret.interactive.yes" => Some("是"),
        "hooks.running_post" => Some("正在运行 post-hooks"),
        "Install CLI '{}' not found on PATH. Please install it or set 'root.manager' in ~/.kam/config.toml" => {
            Some(
                "未在 PATH 中找到 Install CLI。请安装它或在 ~/.kam/config.toml 中设置 'root.manager'",
            )
        }
        "env.docs.intro" => Some("环境文档：简介"),
        "cache.modules.no_matching_cache_file" => Some("未找到匹配的缓存文件"),
        "termux.ssh.interactive.gen_hint" => Some("生成密钥提示"),
        "init.interactive.save_base_values" => Some("保存基础值？"),
        "repo.prompt.enter_number" => Some("请输入一个数字："),
        "init.interactive.version" => Some("版本"),
        "termux.ssh.ssh_missing" => Some("缺少 SSH"),
        "init.interactive.aborted" => Some("已中止"),
        "termux.ssh.interactive.key_installed" => Some("密钥已安装"),
        "init.interactive.enter_path_to_local_template" => Some("输入本地模板路径"),
        "packaging.using_existing_module_prop_from_hook" => Some("使用来自 hook 的现有模块属性"),
        "secret.interactive.enter_name" => Some("输入名称"),
        "secret.interactive.cancel" => Some("取消"),
        "validate.mmrl.repo.readme_file" => Some("README 文件"),
        "init.interactive.invalid_module_id" => Some("无效的模块 ID"),
        "repo.module_detail.download_url" => Some("下载链接"),
        "termux.ssh.interactive.create_new_key_option" => Some("创建新密钥（选项）"),
        "project.build_time" => Some("构建时间"),
        "secret.interactive.encryption_password" => Some("加密密码"),
        "packaging.files" => Some("打包文件"),
        "install.installed" => Some("已安装"),
        "secret.imported" => Some("密钥已导入"),
        "secret.no_secrets_stored" => Some("未存储任何密钥"),
        "config.interactive.invalid_selection" => Some("无效选择"),
        "config.interactive.enter_value_for_key" => Some("为键输入值"),
        "secret.public_key_exported" => Some("公钥已导出"),
        "install.su_failed" => Some("获取 root 权限失败（su 失败）"),
        "validate.prop.id_invalid_characters" => Some("ID 包含无效字符"),
        "validate.mmrl.repo.license_file" => Some("许可证文件"),
        "termux.ssh.forward_failed_err" => Some("SSH 转发失败：{}"),
        "init.interactive.and_more_files" => Some("还有 {} 个文件"),
        "cache.no_templates" => Some("缓存中无模板"),
        "validate.errors.header" => Some("错误"),
        "install.trying_su" => Some("尝试使用 su..."),
        "cache.template_removed" => Some("已从缓存中移除模板"),
        "secret.fetching_root_ca" => Some("正在获取根 CA..."),
        "termux.ssh.interactive.ask_username" => Some("请输入 SSH 用户名"),
        "secret.failed_read_ca" => Some("读取 CA 失败"),
        "init.interactive.directory_preview_header" => Some("目录预览"),
        "secret.interactive.enter_value" => Some("输入值"),
        "build.failed_workspace_members" => Some("部分工作区成员构建失败"),
        "cache.modules.index_entry" => Some("缓存索引项"),
        "build.no_workspace_section" => Some("kam.toml 中没有 workspace 节"),
        "init.interactive.template_contents_showing_up_to_files" => {
            Some("显示最多 {} 个文件的内容")
        }
        "build.building_module_version" => Some("正在构建模块版本 {}"),
        "Source directory '{}' does not exist. Build might fail or produce empty module." => {
            Some("源目录 '{}' 不存在。构建可能失败或产生空模块。")
        }
        "env.no_kam_vars" => Some("未发现 KAM 环境变量"),
        "secret.interactive.select_input_method" => Some("选择输入方法"),
        "init.interactive.enter_yes_no" => Some("输入 yes 或 no"),
        "init.interactive.choose_template" => Some("选择一个模板"),
        "config.interactive.local_project_not_detected" => Some("未检测到本地项目"),
        "validate.prop.description_required" => Some("需要描述"),
        "packaging.success_template_built" => Some("模板构建成功"),
        "secret.interactive.confirm_overwrite" => Some("确认覆盖？"),
        "validate.prop.name_required" => Some("需要名称"),
        "init.interactive.select_value_for" => Some("为 {} 选择值"),
        "config.interactive.local_project_detected" => Some("检测到本地项目"),
        "secret.interactive.intro" => Some("密钥简介"),
        "project.output_file" => Some("输出文件"),
        "config.interactive.set_custom_key" => Some("设置自定义键"),
        "init.interactive.help" => Some("帮助"),
        "secret.root_ca_added" => Some("已添加根 CA"),
        "check.errors.header" => Some("错误"),
        "secret.interactive.error.conflict_with_subcommand" => Some("错误：与子命令冲突"),
        "config.example" => Some("配置示例"),
        "secret.interactive.storage.keyring" => Some("密钥环存储"),
        "init.interactive.next_steps" => Some("下一步"),
        _ => None,
    }
}

fn en_to_zh(s: &str) -> Option<&'static str> {
    match s {
        "This command is informational only; it doesn't modify files or the registry." => {
            Some("此命令仅供参考，不会修改文件或注册表。")
        }
        "Use other commands (e.g., `kam init`, `kam build`) to perform actions." => {
            Some("请使用其他命令（例如 `kam init`, `kam build`）执行实际操作。")
        }
        "Thanks for using Kam" => Some("感谢使用 Kam"),
        "Enjoy your module tooling experience!" => Some("祝您使用愉快！"),
        "Powered by the Kam CLI — Happy building!" => Some("由 Kam CLI 提供支持 — 祝构建顺利！"),
        "workspace member {} not found" => Some("工作区成员 {} 未找到"),
        "Skipping {}: no kam.toml found" => Some("跳过 {}：未找到 kam.toml"),
        "Building workspace member: {}" => Some("构建工作区成员：{}"),
        "Failed to get current dir: {}" => Some("无法获取当前目录：{}"),
        "Failed to change to {}: {}" => Some("切换到目录 {} 失败: {}"),
        "Failed to build {}: {}" => Some("构建 {} 失败：{}"),
        "Skipping {}: failed to load kam.toml: {}" => Some("跳过 {}：加载 kam.toml 失败: {}"),
        "Failed to restore cwd: {}" => Some("恢复当前工作目录失败：{}"),
        "Invalid glob pattern '{}': {}" => Some("无效的 glob 模式 '{}': {}"),
        "✿ Workspace Build Summary ✿" => Some("✿ 工作区构建摘要 ✿"),
        "✗ Failed: {}" => Some("✗ 失败: {}"),
        "✓ Successfully built module: {}" => Some("✓ 成功构建模块: {}"),
        "Building module: {} v{}" => Some("构建模块：{} v{}"),
        "Skipping build hooks for template packaging" => Some("跳过用于模板打包的构建钩子"),
        "Packaging artifacts..." => Some("正在打包制品..."),
        "Source directory not found: {}" => Some("源目录未找到: {}"),
        "Generating module.prop" => Some("正在生成 module.prop"),
        "Using existing module.prop (from pre-build hook)" => {
            Some("使用现有的 module.prop（来自预构建钩子）")
        }
        // Additional mapping for common messages (Set/Unset, secrets, interactive prompts, preview/next steps)
        "Set {} = {} in {}" => Some("设置 {} = {} 到 {}"),
        "Unset {} in {}" => Some("{} 已在 {} 中移除"),
        "No secrets stored." => Some("未存储任何机密。"),
        "No trusted Root CAs." => Some("没有受信任的根证书颁发机构。"),
        "(Non-interactive mode detected: falling back to text input.)" => {
            Some("(检测到非交互模式：退回到文本输入。)")
        }
        "You may press Enter to accept a default value shown in brackets" => {
            Some("您可以按 Enter 接受方括号中显示的默认值")
        }
        "Variable: {}" => Some("变量：{}"),
        "{} - Note: {}" => Some("{} - 备注：{}"),
        "Preview cancelled." => Some("预览已取消。"),
        "Interactive initialization completed successfully." => Some("交互式初始化已成功完成。"),
        "Next steps:" => Some("下一步："),
        "✓ Successfully built template: {}" => Some("✓ 成功构建模板: {}"),
        // Cache-related strings
        "No templates found in local cache." => Some("本地缓存中未找到模板。"),
        "Local Cached Templates" => Some("本地缓存模板"),
        "Cache cleaned successfully" => Some("缓存清理成功"),
        "Cache directory is already empty or does not exist." => Some("缓存目录已经为空或不存在。"),
        "Template '{}' added to cache from {}" => Some("模板 '{}' 已从 {} 添加到缓存"),
        "Template '{}' removed from cache" => Some("模板 '{}' 已从缓存中移除"),
        // Secrets / Trust strings
        "Secret '{}' saved" => Some("密钥 '{}' 已保存"),
        "Secret '{}' written to {}" => Some("密钥 '{}' 已写入到 {}"),
        "Secret '{}' removed" => Some("密钥 '{}' 已移除"),
        "Secret '{}' exported to {}" => Some("密钥 '{}' 已导出到 {}"),
        "Secret '{}' imported" => Some("密钥 '{}' 已导入"),
        "Public key for secret '{}' exported to {}" => Some("密钥 '{}' 的公钥已导出到 {}"),
        "Certificate chain '{}' imported successfully" => Some("证书链 '{}' 已成功导入"),
        "Root CA '{}' added to trust store" => Some("根证书 '{}' 已添加到信任存储"),
        "Root CA '{}' removed from trust store" => Some("根证书 '{}' 已从信任存储移除"),
        "Stored Secrets" => Some("存储的密钥"),
        "Trusted Root CAs" => Some("受信任的根证书颁发机构"),
        // Table labels
        "Item" => Some("配置项"),
        "Value" => Some("值"),
        "Module" => Some("模块"),
        "Total" => Some("总计"),
        "Succeeded" => Some("成功"),
        "Failed" => Some("失败"),
        "Build Time" => Some("构建时间"),
        "Total Duration" => Some("总耗时"),
        // CA fetching error messages
        "Failed to fetch CA: {}" => Some("获取 CA 失败: {}"),
        "Failed to read CA: {}" => Some("读取 CA 失败: {}"),
        // Public key / signer errors
        "Failed to derive public key: {}" => Some("推导公钥失败: {}"),
        "Failed to parse private key: {}" => Some("解析私钥失败: {}"),
        "Failed to extract public key: {}" => Some("提取公钥失败: {}"),
        "Failed to create signer: {}" => Some("创建签名器失败: {}"),
        "Failed to update signer: {}" => Some("更新签名器失败: {}"),
        "Failed to sign: {}" => Some("签名失败: {}"),
        "Failed to base64 decode signature: {}" => Some("Base64 解码签名失败: {}"),
        "Failed to parse public key PEM from {}: {}" => Some("从 {} 解析公钥 PEM 失败: {}"),
        "Failed to parse public key from certificate: {}" => Some("从证书解析公钥失败: {}"),
        "Failed to create verifier: {}" => Some("创建验证器失败: {}"),
        "Failed to update verifier: {}" => Some("更新验证器失败: {}"),
        "Verification error: {}" => Some("验证错误: {}"),
        "Verification FAILED for '{}'" => Some("验证失败：{}"),
        "Verification successful" => Some("验证成功"),
        "Verified" => Some("已验证"),
        "Source file not found: {}" => Some("源文件未找到: {}"),
        "Signature file not found: {}" => Some("签名文件未找到: {}"),
        "No trusted Root CAs found. Add one with: kam secret trust --add-root <ca.pem> --ca-name <name>" => {
            Some(
                "未找到受信任的根 CA。请使用：kam secret trust --add-root <ca.pem> --ca-name <name> 来添加",
            )
        }
        "Verifying certificate chain..." => Some("正在验证证书链..."),
        "Certificate chain verified successfully" => Some("证书链验证成功"),
        "Loading cached certificate '{}'..." => Some("正在加载缓存证书 '{}'..."),
        "Fetching certificate from GitHub issue {}..." => Some("从 GitHub issue {} 获取证书..."),
        // Validate related
        "kam.toml not found at {}" => Some("在 {} 未找到 kam.toml"),
        "Validating {}..." => Some("正在验证 {}..."),
        "Failed to parse kam.toml: {}" => Some("解析 kam.toml 失败：{}"),
        "No issues found. kam.toml is valid." => Some("未发现问题。kam.toml 有效。"),
        "Errors:" => Some("错误："),
        "Warnings:" => Some("警告："),
        "Validation failed. Please fix the errors above." => Some("验证失败。请修复上述错误。"),
        "Validation passed with warnings." => Some("验证通过，但有警告。"),
        // Export related
        "Exported module.prop to {}" => Some("已导出 module.prop 到 {}"),
        "Exported module.json to {}" => Some("已导出 module.json 到 {}"),
        "Exported repo.json to {}" => Some("已导出 repo.json 到 {}"),
        "Exported track.json to {}" => Some("已导出 track.json 到 {}"),
        "Exported config.json to {}" => Some("已导出 config.json 到 {}"),
        "Exported update.json to {}" => Some("已导出 update.json 到 {}"),
        // Check related
        "Some issues found." => Some("发现一些问题。"),
        "No issues found." => Some("未发现问题。"),
        // Version related
        "Bumped version: {} -> {}" => Some("版本已更新：{} -> {}"),
        "Version unchanged: {}" => Some("版本未更改：{}"),
        "Updated versionCode: {} -> {}" => Some("已更新 versionCode：{} -> {}"),
        "Current version: {}" => Some("当前版本：{}"),
        "Current versionCode: {}" => Some("当前 versionCode：{}"),
        // Init/Interactive related
        "Please enter 'y' or 'n'." => Some("请输入 'y' 或 'n'。"),
        "Choose a template to use" => Some("选择要使用的模板"),
        "Enter local template path or archive file (leave empty to download default templates)" => {
            Some("输入本地模板路径或归档文件（留空以下载默认模板）")
        }
        "Enter path to local template (file or dir)" => Some("输入本地模板路径（文件或目录）"),
        "Enter custom value for {}" => Some("输入 {} 的自定义值"),
        "Enter value for {} (index or value)" => Some("输入 {} 的值（索引或值）"),
        "Enter true/false for {} (default: {})" => Some("为 {} 输入 true/false（默认：{}）"),
        "Please enter 'true' or 'false'" => Some("请输入 'true' 或 'false'"),
        "Enter value for {}" => Some("输入 {} 的值"),
        "Failed to parse existing config: {}" => Some("解析现有配置失败：{}"),
        "Failed to serialize config: {}" => Some("序列化配置失败：{}"),
        "Failed to prepare initialization defaults: {}" => Some("准备初始化默认值失败：{}"),
        "Saved base configuration to ~/.kam/config.toml" => {
            Some("已保存基础配置到 ~/.kam/config.toml")
        }
        "Summary" => Some("摘要"),
        "Path" => Some("路径"),
        "Template variables:" => Some("模板变量："),
        "Recommend: {}" => Some("推荐：{}"),
        "Or you may run the interactive helper script: {}" => {
            Some("或者您可以运行交互式辅助脚本：{}")
        }
        "Recommended: {}" => Some("推荐：{}"),
        "(Template directory empty)" => Some("（模板目录为空）"),
        "Interactive Kam Init" => Some("交互式 Kam 初始化"),
        // Template pull related
        "Downloading templates from: {}" => Some("正在从 {} 下载模板"),
        "Importing downloaded templates..." => Some("正在导入已下载的模板..."),
        "Templates downloaded and imported successfully" => Some("模板已成功下载并导入"),
        "Could not determine file size. Progress bar will be disabled." => {
            Some("无法确定文件大小。进度条将被禁用。")
        }
        // Sign related
        "Signed '{}' -> {}" => Some("已签名 '{}' -> {}"),
        "Signing failed for {}: {}" => Some("签名失败 {}: {}"),
        "Either specify 'src' or --dist/--all to sign artifacts" => {
            Some("请指定 'src' 或 --dist/--all 来签名制品")
        }
        // Verify related
        "Loading certificate chain from {}..." => Some("正在从 {} 加载证书链..."),
        "Calculating hash for '{}'..." => Some("正在计算 '{}' 的哈希值..."),
        // Build hooks related
        "✿ Running {} hooks from {} ({} script(s)) ✿" => {
            Some("✿ 正在运行来自 {} 的 {} 钩子（{} 个脚本）✿")
        }
        "[{} {}/{}] {}" => Some("[{} {}/{}] {}"),
        "Running pre-build hooks" => Some("正在运行预构建钩子"),
        "Running post-build hooks" => Some("正在运行后构建钩子"),
        "Build complete" => Some("构建完成"),
        // About related
        "Author" => Some("作者"),
        "Email" => Some("邮箱"),
        "Developer" => Some("开发者"),
        "Description" => Some("描述"),
        "Repository" => Some("仓库"),
        // Status labels
        "状态" => Some("状态"),
        "统计项" => Some("统计项"),
        "✓ 成功" => Some("✓ 成功"),
        _ => None,
    }
}

fn zh_to_en(s: &str) -> Option<&'static str> {
    match s {
        "此命令仅供参考，不会修改文件或注册表。" => {
            Some("This command is informational only; it doesn't modify files or the registry.")
        }
        "请使用其他命令（例如 `kam init`, `kam build`）执行实际操作。" => {
            Some("Use other commands (e.g., `kam init`, `kam build`) to perform actions.")
        }
        "感谢使用 Kam" => Some("Thanks for using Kam"),
        "祝您使用愉快！" => Some("Enjoy your module tooling experience!"),
        "由 Kam CLI 提供支持 — 祝构建顺利！" => {
            Some("Powered by the Kam CLI — Happy building!")
        }
        "工作区成员 {} 未找到" => Some("workspace member {} not found"),
        "跳过 {}：未找到 kam.toml" => Some("Skipping {}: no kam.toml found"),
        "构建工作区成员：{}" => Some("Building workspace member: {}"),
        "无法获取当前目录：{}" => Some("Failed to get current dir: {}"),
        "切换到目录 {} 失败: {}" => Some("Failed to change to {}: {}"),
        "构建 {} 失败：{}" => Some("Failed to build {}: {}"),
        "跳过 {}：加载 kam.toml 失败: {}" => {
            Some("Skipping {}: failed to load kam.toml: {}")
        }
        "恢复当前工作目录失败：{}" => Some("Failed to restore cwd: {}"),
        "无效的 glob 模式 '{}': {}" => Some("Invalid glob pattern '{}': {}"),
        "✿ 工作区构建摘要 ✿" => Some("✿ Workspace Build Summary ✿"),
        "✗ 失败: {}" => Some("✗ Failed: {}"),
        "✓ 成功构建模块: {}" => Some("✓ Successfully built module: {}"),
        "构建模块：{} v{}" => Some("Building module: {} v{}"),
        "跳过用于模板打包的构建钩子" => {
            Some("Skipping build hooks for template packaging")
        }
        "正在打包制品..." => Some("Packaging artifacts..."),
        "源目录未找到: {}" => Some("Source directory not found: {}"),
        "正在生成 module.prop" => Some("Generating module.prop"),
        "使用现有的 module.prop（来自预构建钩子）" => {
            Some("Using existing module.prop (from pre-build hook)")
        }
        // Additional mapping for common messages (reverse direction: zh -> en)
        "设置 {} = {} 到 {}" => Some("Set {} = {} in {}"),
        "{} 已在 {} 中移除" => Some("Unset {} in {}"),
        "未存储任何机密。" => Some("No secrets stored."),
        "没有受信任的根证书颁发机构。" => Some("No trusted Root CAs."),
        "(检测到非交互模式：退回到文本输入。)" => {
            Some("(Non-interactive mode detected: falling back to text input.)")
        }
        "您可以按 Enter 接受方括号中显示的默认值" => {
            Some("You may press Enter to accept a default value shown in brackets")
        }
        "变量：{}" => Some("Variable: {}"),
        "{} - 备注：{}" => Some("{} - Note: {}"),
        "预览已取消。" => Some("Preview cancelled."),
        "交互式初始化已成功完成。" => {
            Some("Interactive initialization completed successfully.")
        }
        "下一步：" => Some("Next steps:"),
        "✓ 成功构建模板: {}" => Some("✓ Successfully built template: {}"),
        // Cache-related strings
        "本地缓存中未找到模板。" => Some("No templates found in local cache."),
        "本地缓存模板" => Some("Local Cached Templates"),
        "缓存清理成功" => Some("Cache cleaned successfully"),
        "缓存目录已经为空或不存在。" => {
            Some("Cache directory is already empty or does not exist.")
        }
        "模板 '{}' 已从 {} 添加到缓存" => Some("Template '{}' added to cache from {}"),
        "模板 '{}' 已从缓存中移除" => Some("Template '{}' removed from cache"),
        // Secrets / Trust strings (zh -> en)
        "密钥 '{}' 已保存" => Some("Secret '{}' saved"),
        "密钥 '{}' 已写入到 {}" => Some("Secret '{}' written to {}"),
        "密钥 '{}' 已移除" => Some("Secret '{}' removed"),
        "密钥 '{}' 已导出到 {}" => Some("Secret '{}' exported to {}"),
        "密钥 '{}' 已导入" => Some("Secret '{}' imported"),
        "密钥 '{}' 的公钥已导出到 {}" => Some("Public key for secret '{}' exported to {}"),
        "证书链 '{}' 已成功导入" => Some("Certificate chain '{}' imported successfully"),
        "根证书 '{}' 已添加到信任存储" => Some("Root CA '{}' added to trust store"),
        "根证书 '{}' 已从信任存储移除" => Some("Root CA '{}' removed from trust store"),
        "存储的密钥" => Some("Stored Secrets"),
        "受信任的根证书颁发机构" => Some("Trusted Root CAs"),
        // Table labels
        "配置项" => Some("Item"),
        "值" => Some("Value"),
        "模块" => Some("Module"),
        "总计" => Some("Total"),
        "成功" => Some("Succeeded"),
        "失败" => Some("Failed"),
        "构建时间" => Some("Build Time"),
        "总耗时" => Some("Total Duration"),
        // CA fetching error messages (zh -> en)
        "获取 CA 失败: {}" => Some("Failed to fetch CA: {}"),
        "读取 CA 失败: {}" => Some("Failed to read CA: {}"),
        // Public key / signer errors (zh -> en)
        "推导公钥失败: {}" => Some("Failed to derive public key: {}"),
        "解析私钥失败: {}" => Some("Failed to parse private key: {}"),
        "提取公钥失败: {}" => Some("Failed to extract public key: {}"),
        "创建签名器失败: {}" => Some("Failed to create signer: {}"),
        "更新签名器失败: {}" => Some("Failed to update signer: {}"),
        "签名失败: {}" => Some("Failed to sign: {}"),
        "Base64 解码签名失败: {}" => Some("Failed to base64 decode signature: {}"),
        "从 {} 解析公钥 PEM 失败: {}" => Some("Failed to parse public key PEM from {}: {}"),
        "从证书解析公钥失败: {}" => {
            Some("Failed to parse public key from certificate: {}")
        }
        "创建验证器失败: {}" => Some("Failed to create verifier: {}"),
        "更新验证器失败: {}" => Some("Failed to update verifier: {}"),
        "验证错误: {}" => Some("Verification error: {}"),
        "验证失败：{}" => Some("Verification FAILED for '{}'"),
        "验证成功" => Some("Verification successful"),
        "已验证" => Some("Verified"),
        "源文件未找到: {}" => Some("Source file not found: {}"),
        "签名文件未找到: {}" => Some("Signature file not found: {}"),
        "未找到受信任的根 CA。请使用：kam secret trust --add-root <ca.pem> --ca-name <name> 来添加" => {
            Some(
                "No trusted Root CAs found. Add one with: kam secret trust --add-root <ca.pem> --ca-name <name>",
            )
        }
        "正在验证证书链..." => Some("Verifying certificate chain..."),
        "证书链验证成功" => Some("Certificate chain verified successfully"),
        "正在加载缓存证书 '{}'..." => Some("Loading cached certificate '{}'..."),
        "从 GitHub issue {} 获取证书..." => {
            Some("Fetching certificate from GitHub issue {}...")
        }
        // Validate related (zh -> en)
        "在 {} 未找到 kam.toml" => Some("kam.toml not found at {}"),
        "正在验证 {}..." => Some("Validating {}..."),
        "解析 kam.toml 失败: {}" => Some("Failed to parse kam.toml: {}"),
        "未发现问题。kam.toml 有效。" => Some("No issues found. kam.toml is valid."),
        "错误：" => Some("Errors:"),
        "警告：" => Some("Warnings:"),
        "验证失败。请修复上述错误。" => {
            Some("Validation failed. Please fix the errors above.")
        }
        "验证通过，但有警告。" => Some("Validation passed with warnings."),
        // Export related (zh -> en)
        "已导出 module.prop 到 {}" => Some("Exported module.prop to {}"),
        "已导出 module.json 到 {}" => Some("Exported module.json to {}"),
        "已导出 repo.json 到 {}" => Some("Exported repo.json to {}"),
        "已导出 track.json 到 {}" => Some("Exported track.json to {}"),
        "已导出 config.json 到 {}" => Some("Exported config.json to {}"),
        "已导出 update.json 到 {}" => Some("Exported update.json to {}"),
        // Check related (zh -> en)
        "发现一些问题。" => Some("Some issues found."),
        "未发现问题。" => Some("No issues found."),
        // Version related (zh -> en)
        "版本已更新：{} -> {}" => Some("Bumped version: {} -> {}"),
        "版本未更改：{}" => Some("Version unchanged: {}"),
        "已更新 versionCode：{} -> {}" => Some("Updated versionCode: {} -> {}"),
        "当前版本：{}" => Some("Current version: {}"),
        "当前 versionCode：{}" => Some("Current versionCode: {}"),
        // Init/Interactive related (zh -> en)
        "请输入 'y' 或 'n'。" => Some("Please enter 'y' or 'n'."),
        "选择要使用的模板" => Some("Choose a template to use"),
        "输入本地模板路径或归档文件（留空以下载默认模板）" => Some(
            "Enter local template path or archive file (leave empty to download default templates)",
        ),
        "输入本地模板路径（文件或目录）" => {
            Some("Enter path to local template (file or dir)")
        }
        "输入 {} 的自定义值" => Some("Enter custom value for {}"),
        "输入 {} 的值（索引或值）" => Some("Enter value for {} (index or value)"),
        "为 {} 输入 true/false（默认：{}）" => {
            Some("Enter true/false for {} (default: {})")
        }
        "请输入 'true' 或 'false'" => Some("Please enter 'true' or 'false'"),
        "输入 {} 的值" => Some("Enter value for {}"),
        "解析现有配置失败：{}" => Some("Failed to parse existing config: {}"),
        "序列化配置失败：{}" => Some("Failed to serialize config: {}"),
        "准备初始化默认值失败：{}" => {
            Some("Failed to prepare initialization defaults: {}")
        }
        "已保存基础配置到 ~/.kam/config.toml" => {
            Some("Saved base configuration to ~/.kam/config.toml")
        }
        "摘要" => Some("Summary"),
        "路径" => Some("Path"),
        "模板变量：" => Some("Template variables:"),
        "推荐：{}" => Some("Recommend: {}"),
        "或者您可以运行交互式辅助脚本：{}" => {
            Some("Or you may run the interactive helper script: {}")
        }
        "（模板目录为空）" => Some("(Template directory empty)"),
        "交互式 Kam 初始化" => Some("Interactive Kam Init"),
        // Template pull related (zh -> en)
        "正在从 {} 下载模板" => Some("Downloading templates from: {}"),
        "正在导入已下载的模板..." => Some("Importing downloaded templates..."),
        "模板已成功下载并导入" => Some("Templates downloaded and imported successfully"),
        "无法确定文件大小。进度条将被禁用。" => {
            Some("Could not determine file size. Progress bar will be disabled.")
        }
        // Sign related (zh -> en)
        "已签名 '{}' -> {}" => Some("Signed '{}' -> {}"),
        "签名失败 {}: {}" => Some("Signing failed for {}: {}"),
        "请指定 'src' 或 --dist/--all 来签名制品" => {
            Some("Either specify 'src' or --dist/--all to sign artifacts")
        }
        // Verify related (zh -> en)
        "正在从 {} 加载证书链..." => Some("Loading certificate chain from {}..."),
        "正在计算 '{}' 的哈希值..." => Some("Calculating hash for '{}'..."),
        // Build hooks related (zh -> en)
        "✿ 正在运行来自 {} 的 {} 钩子（{} 个脚本）✿" => {
            Some("✿ Running {} hooks from {} ({} script(s)) ✿")
        }
        "[{} {}/{}] {}" => Some("[{} {}/{}] {}"),
        "正在运行预构建钩子" => Some("Running pre-build hooks"),
        "正在运行后构建钩子" => Some("Running post-build hooks"),
        "构建完成" => Some("Build complete"),
        // Config command related (zh -> en)
        "内置配置项：" => Some("Built-in configuration keys:"),
        "示例：" => Some("Example:"),
        "注意：您也可以设置自定义键。使用 'kam config list' 查看所有已配置的值。" => {
            Some(
                "Note: You can also set custom keys. Use 'kam config list' to see all configured values.",
            )
        }
        "UI 语言偏好（优先于 'language'）" => {
            Some("UI language preference (preferred over 'language')")
        }
        "UI 语言偏好（备用，建议使用 'ui.language'）" => {
            Some("UI language preference (fallback, use 'ui.language' instead)")
        }
        "新项目的默认作者名称（在初始化时保存）" => {
            Some("Default author name for new projects (saved during init)")
        }
        "新项目的默认模块名称（在初始化时保存）" => {
            Some("Default module name for new projects (saved during init)")
        }
        "新项目的默认版本（在初始化时保存）" => {
            Some("Default version for new projects (saved during init)")
        }
        // Config command related (en -> zh)
        "Built-in configuration keys:" => Some("内置配置项："),
        "Example:" => Some("示例："),
        "Note: You can also set custom keys. Use 'kam config list' to see all configured values." => {
            Some("注意：您也可以设置自定义键。使用 'kam config list' 查看所有已配置的值。")
        }
        "UI language preference (preferred over 'language')" => {
            Some("UI 语言偏好（优先于 'language'）")
        }
        "UI language preference (fallback, use 'ui.language' instead)" => {
            Some("UI 语言偏好（备用，建议使用 'ui.language'）")
        }
        "Default author name for new projects (saved during init)" => {
            Some("新项目的默认作者名称（在初始化时保存）")
        }
        "Default module name for new projects (saved during init)" => {
            Some("新项目的默认模块名称（在初始化时保存）")
        }
        "Default version for new projects (saved during init)" => {
            Some("新项目的默认版本（在初始化时保存）")
        }
        // About related (zh -> en)
        "作者" => Some("Author"),
        "邮箱" => Some("Email"),
        "开发者" => Some("Developer"),
        "描述" => Some("Description"),
        "仓库" => Some("Repository"),
        // Status labels (zh -> en)
        "状态" => Some("状态"),
        "统计项" => Some("统计项"),
        "✓ 成功" => Some("✓ 成功"),
        _ => None,
    }
}

// Fluent formatting helpers (loaded per-call; no global FluentBundle in statics).
// These helpers attempt to format a dotted-key using Fluent (hyphenated id).
// Behavior:
//  - Check runtime override dir (KAM_LOCALES_DIR/<lang>/main.ftl) first (if present).
//  - Otherwise include compiled-in FTL under `src/locales/<lang>/main.ftl`.
// Notes:
//  - We build a transient FluentBundle per call to avoid placing non-Send types
//    inside static variables (avoids Sync/Send issues when embedding bundles).
//  - Positional `{}` args are exposed to Fluent as `$arg0`, `$arg1`, ... and
//    `count` is set from the first argument if it parses as an integer.

fn dotted_to_fluent_id(dotted: &str) -> String {
    dotted.replace(['.', '_'], "-")
}

fn bundle_from_ftl_str(lang_code: &str, ftl_str: &str) -> Option<FluentBundle<FluentResource>> {
    let res = FluentResource::try_new(ftl_str.to_owned()).ok()?;
    let langid: LanguageIdentifier = lang_code.parse().ok()?;
    // Create a local bundle for the requested language
    let mut bundle = FluentBundle::new(vec![langid]);
    // Add the resource by value so the bundle owns it (avoids borrow/lifetime issues).
    let _ = bundle.add_resource(res);
    Some(bundle)
}

fn build_bundle_for_locale(lang_code: &str) -> Option<FluentBundle<FluentResource>> {
    // 1) Runtime override (KAM_LOCALES_DIR/<lang>/main.ftl)
    if let Ok(dir) = std::env::var("KAM_LOCALES_DIR") {
        let candidate = std::path::Path::new(&dir).join(lang_code).join("main.ftl");
        if candidate.exists()
            && let Ok(contents) = std::fs::read_to_string(candidate)
            && let Some(bundle) = bundle_from_ftl_str(lang_code, &contents)
        {
            return Some(bundle);
        }
    }

    // 2) Fallback to compiled-in locales under src/locales/<lang>/main.ftl
    match lang_code {
        "en-US" => bundle_from_ftl_str("en-US", include_str!("locales/en-US/main.ftl")),
        "zh-CN" => bundle_from_ftl_str("zh-CN", include_str!("locales/zh-CN/main.ftl")),
        _ => None,
    }
}

/// Format a dotted key (e.g., `termux.ssh.forwarded`) for a specific language.
/// Returns `Some(String)` if a Fluent translation exists and formatting succeeded.
pub fn format_for_lang(lang: Language, key: &str, args: &[&dyn Display]) -> Option<String> {
    let id = dotted_to_fluent_id(key);
    let lang_code = match lang {
        Language::En => "en-US",
        Language::Zh => "zh-CN",
    };

    let bundle = build_bundle_for_locale(lang_code)?;
    let message = bundle.get_message(&id)?;
    let value = message.value()?;

    // Build FluentArgs: arg0, arg1, ... (and count if first arg is numeric)
    let mut f_args = FluentArgs::new();
    for (i, a) in args.iter().enumerate() {
        let s = format!("{}", a);
        // Create owned key + owned string values so nothing is borrowed from a temporary.
        let key = format!("arg{}", i);
        if let Ok(n) = s.parse::<i64>() {
            f_args.set(key, FluentValue::from(n));
        } else {
            f_args.set(key, FluentValue::from(s));
        }
    }
    if !args.is_empty() {
        let first = format!("{}", args[0]);
        if let Ok(n) = first.parse::<i64>() {
            f_args.set("count", FluentValue::from(n));
        }
    }

    let mut errs = vec![];
    let formatted = bundle.format_pattern(value, Some(&f_args), &mut errs);
    Some(formatted.to_string())
}

/// Convenience wrapper that formats for the *current* language, if available.
pub fn format_for_current_lang(key: &str, args: &[&dyn Display]) -> Option<String> {
    format_for_lang(current_language(), key, args)
}

/// Check existence of a Fluent message id for a given language/key
pub fn has_message(lang: Language, key: &str) -> bool {
    let id = dotted_to_fluent_id(key);
    let lang_code = match lang {
        Language::En => "en-US",
        Language::Zh => "zh-CN",
    };
    build_bundle_for_locale(lang_code).is_some_and(|b| b.get_message(&id).is_some())
}

// --- Public helpers for config-based language retrieval ----------------------

/// Tries to read `language` or `ui.language` from the effective `kam` config
/// file (local if in a project, otherwise global). To avoid code duplication,
/// the config module provides a small public helper. If that helper is not
/// available, this function falls back to a best-effort attempt to parse the
/// default config path.
fn get_language_from_config() -> Option<String> {
    // Prefer calling into `cmds::config::read_language_from_config()` if it exists.
    // To keep backward compatibility in case it isn't exported, we do this in a
    // guarded fashion.
    // Note: This function expects `cmds::config::read_language_from_config()` to be
    // public and return `Option<String>`. If you changed `config.rs` keep an eye on
    // compilation errors and export a tiny helper there.
    std::panic::catch_unwind(|| {
        // Use fully-qualified absolute path to call function in case `cfg` differs:
        crate::cmds::config::read_language_from_config()
    })
    .unwrap_or_default()
}

// --- Macros -----------------------------------------------------------------
// Macro-style formatting that performs lookup of the template first and then
// formats it with the provided arguments.
//
// Usage:
//   trf!("Building module: {} v{}", &module_id, &version)
// trf macro has been moved to crate root (lib.rs) to ensure visibility across modules.
// This duplicate definition was removed from i18n.rs to avoid conflicts.

// Also provide a simple `tr` function to translate a single key.
pub fn tr(s: &str) -> String {
    tr_key(s).to_string()
}

// --- Unit tests (optional) --------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn mk_temp_dir(prefix: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = env::temp_dir().join(format!("kam_test_{}_{}", prefix, now));
        // create dir (ignore error if already exists)
        let _ = fs::create_dir_all(&dir);
        dir
    }
    #[test]
    #[serial]
    fn test_translate_en_to_zh() {
        set_language(Language::Zh);
        assert_eq!(tr_key("Thanks for using Kam"), "感谢使用 Kam");
    }

    #[test]
    #[serial]
    fn test_translate_zh_to_en() {
        set_language(Language::En);
        assert_eq!(tr_key("感谢使用 Kam"), "Thanks for using Kam");
    }

    #[test]
    #[serial]
    fn test_repo_i18n_keys_present() {
        // Ensure commonly used repo keys exist in at least one of the keyed maps
        // (either English or Chinese). This avoids fragile tests when runtime
        // overlays or partial translations are present; we accept presence in
        // either map as success.
        let keys = vec![
            "repo.result_line_simple",
            "repo.score_format",
            "repo.no_results_for",
            "repo.authors",
            "repo.url",
            "repo.version",
            "repo.no_downloadable_zip_asset",
            "repo.confirm_download",
            "repo.skipped_download",
            "repo.saved",
            "repo.failed_to_download",
            "repo.search.empty_query",
        ];

        // Collect keys missing in keyed maps and Fluent messages for clearer debug output.
        let mut missing: Vec<&str> = Vec::new();
        for k in &keys {
            let en_has = keyed_en_map().contains_key(*k);
            let zh_has = keyed_zh_map().contains_key(*k);
            // Also accept presence in Fluent translations (FTL) for either language.
            let ftl_en = has_message(Language::En, *k);
            let ftl_zh = has_message(Language::Zh, *k);
            if !en_has && !zh_has && !ftl_en && !ftl_zh {
                missing.push(*k);
            }
        }

        if !missing.is_empty() {
            // Print diagnostic information to help debug missing translations.
            eprintln!("Missing repo i18n keys: {:?}", missing);

            let en_repo_keys: Vec<_> = keyed_en_map()
                .keys()
                .filter(|k| k.starts_with("repo."))
                .collect();
            let zh_repo_keys: Vec<_> = keyed_zh_map()
                .keys()
                .filter(|k| k.starts_with("repo."))
                .collect();

            // Also show which of the expected keys are present as Fluent messages.
            let mut en_ftl_found: Vec<&str> = vec![];
            let mut zh_ftl_found: Vec<&str> = vec![];
            for k in &keys {
                if has_message(Language::En, k) {
                    en_ftl_found.push(*k);
                }
                if has_message(Language::Zh, k) {
                    zh_ftl_found.push(*k);
                }
            }

            eprintln!("EN repo keys ({}): {:?}", en_repo_keys.len(), en_repo_keys);
            eprintln!("ZH repo keys ({}): {:?}", zh_repo_keys.len(), zh_repo_keys);
            eprintln!(
                "EN FTL repo keys ({}): {:?}",
                en_ftl_found.len(),
                en_ftl_found
            );
            eprintln!(
                "ZH FTL repo keys ({}): {:?}",
                zh_ftl_found.len(),
                zh_ftl_found
            );
        }

        assert!(
            missing.is_empty(),
            "Missing i18n keys in en/zh maps or FTL messages: {:?}",
            missing
        );
    }

    #[test]
    #[serial]
    fn test_ftl_key_coverage() {
        use regex::Regex;
        use std::fs;

        // Ensure the en-US FTL exists and all its message IDs are present in zh-CN as well.
        let en_path = "src/locales/en-US/main.ftl";
        let zh_path = "src/locales/zh-CN/main.ftl";

        let en_s = fs::read_to_string(en_path).expect("Failed to read en-US FTL file");
        let zh_s = fs::read_to_string(zh_path).expect("Failed to read zh-CN FTL file");

        let id_re = Regex::new(r"^([A-Za-z0-9_-]+)\s*=").unwrap();

        let mut en_keys = Vec::new();
        for line in en_s.lines() {
            if let Some(c) = id_re.captures(line) {
                en_keys.push(c[1].to_string());
            }
        }

        let mut zh_keys = std::collections::HashSet::new();
        for line in zh_s.lines() {
            if let Some(c) = id_re.captures(line) {
                zh_keys.insert(c[1].to_string());
            }
        }

        let missing: Vec<_> = en_keys
            .into_iter()
            .filter(|k| !zh_keys.contains(k))
            .collect();
        assert!(missing.is_empty(), "Missing keys in zh-CN: {:?}", missing);
    }

    #[test]
    #[serial]
    fn test_trf_macro_behaviour() {
        set_language(Language::Zh);
        // Using the macro, with English key
        let s = trf!("Building module: {} v{}", "mod", "1.0");
        assert!(s.contains("构建模块") || s.contains("构建模块："));
    }

    #[test]
    #[serial]
    fn test_tr_fmt_single_stack_helper() {
        // Ensure stack-allocated helper path works and formats a single arg correctly.
        set_language(Language::Zh);
        let s = tr_fmt_single("status.failed", "foo");
        assert_eq!(s, "✗ 失败: foo");
        set_language(Language::En);
        let s2 = tr_fmt_single("status.failed", "bar");
        assert_eq!(s2, "✗ Failed: bar");
        // Reset language
        set_language(Language::En);
    }

    #[test]
    #[serial]
    fn test_tr_fmt_panics_on_missing_key() {
        // tr_fmt should panic for missing dotted keyed messages (both en & zh)
        let orig = std::env::var("KAM_I18N_STRICT").ok();
        unsafe {
            std::env::set_var("KAM_I18N_STRICT", "1");
        }

        // Generate a runtime-unique key so static analysis (build-time checks) won't
        // treat the literal test key as a missing i18n and fail early.
        let uniq = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let key = format!("this.key.does.not.exist.{}", uniq);

        set_language(Language::En);
        let r = std::panic::catch_unwind(|| {
            tr_fmt(&key, &[] as &[&dyn std::fmt::Display]);
        });
        assert!(
            r.is_err(),
            "Expected tr_fmt to panic on missing keyed i18n (en)"
        );

        set_language(Language::Zh);
        let r2 = std::panic::catch_unwind(|| {
            tr_fmt(&key, &[] as &[&dyn std::fmt::Display]);
        });
        assert!(
            r2.is_err(),
            "Expected tr_fmt to panic on missing keyed i18n (zh)"
        );

        // Restore env & reset language
        if let Some(v) = orig {
            unsafe {
                std::env::set_var("KAM_I18N_STRICT", v);
            }
        } else {
            unsafe {
                std::env::remove_var("KAM_I18N_STRICT");
            }
        }
        set_language(Language::En);
    }

    #[test]
    #[serial]
    fn test_tr_key_panics_on_missing_key() {
        // tr_key should panic when dotted keyed messages are missing
        let orig = std::env::var("KAM_I18N_STRICT").ok();
        unsafe {
            std::env::set_var("KAM_I18N_STRICT", "1");
        }

        // Generate a runtime-unique key to avoid triggering build-time checks that
        // look for static literal missing keys.
        let uniq = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let key = format!("this.key.also.missing.{}", uniq);

        set_language(Language::En);
        let r = std::panic::catch_unwind(|| {
            let _ = tr_key(&key);
        });
        assert!(
            r.is_err(),
            "Expected tr_key to panic on missing keyed i18n (en)"
        );

        set_language(Language::Zh);
        let r2 = std::panic::catch_unwind(|| {
            let _ = tr_key(&key);
        });
        assert!(
            r2.is_err(),
            "Expected tr_key to panic on missing keyed i18n (zh)"
        );

        // Restore env & language
        if let Some(v) = orig {
            unsafe {
                std::env::set_var("KAM_I18N_STRICT", v);
            }
        } else {
            unsafe {
                std::env::remove_var("KAM_I18N_STRICT");
            }
        }
        set_language(Language::En);
    }

    #[test]
    #[serial]
    fn test_tr_fmt_non_strict_allows_missing() {
        // Ensure default (non-strict) does not panic for missing dotted keyed messages.
        let orig = std::env::var("KAM_I18N_STRICT").ok();
        // Ensure strict mode unset
        unsafe {
            std::env::remove_var("KAM_I18N_STRICT");
        }
        set_language(Language::En);
        let r = std::panic::catch_unwind(|| {
            tr_fmt("this.key.does.not.exist", &[] as &[&dyn std::fmt::Display]);
        });
        assert!(r.is_ok(), "Expected tr_fmt not to panic in non-strict mode");
        // Restore env & language
        if let Some(v) = orig {
            unsafe {
                std::env::set_var("KAM_I18N_STRICT", v);
            }
        } else {
            unsafe {
                std::env::remove_var("KAM_I18N_STRICT");
            }
        }
        set_language(Language::En);
    }

    #[test]
    #[serial]
    fn test_init_env_var_precedence() {
        // Save environment variables to restore later
        let orig_kam_ui = env::var("KAM_UI_LANGUAGE").ok();
        let orig_kam_lang = env::var("KAM_LANG").ok();
        let orig_lang = env::var("LANG").ok();
        let orig_cwd = env::current_dir().unwrap();

        // Ensure no config overrides
        unsafe {
            env::remove_var("KAM_UI_LANGUAGE");
        }
        unsafe {
            env::remove_var("KAM_LANG");
        }
        // set KAM_UI_LANGUAGE to zh
        unsafe {
            env::set_var("KAM_UI_LANGUAGE", "zh");
        }

        // start from a known language
        set_language(Language::En);
        // initialize i18n
        init();

        // env override should take precedence
        assert_eq!(current_language(), Language::Zh);

        // restore environment
        if let Some(k) = orig_kam_ui {
            unsafe {
                env::set_var("KAM_UI_LANGUAGE", k);
            }
        } else {
            unsafe {
                env::remove_var("KAM_UI_LANGUAGE");
            }
        }
        if let Some(k2) = orig_kam_lang {
            unsafe {
                env::set_var("KAM_LANG", k2);
            }
        } else {
            unsafe {
                env::remove_var("KAM_LANG");
            }
        }
        if let Some(l) = orig_lang {
            unsafe {
                env::set_var("LANG", l);
            }
        } else {
            unsafe {
                env::remove_var("LANG");
            }
        }
        let _ = env::set_current_dir(orig_cwd);

        // reset global language
        set_language(Language::En);
    }

    #[test]
    #[serial]
    fn test_init_local_config_precedence() {
        let orig_home = env::var("HOME").ok();
        let orig_kam_ui = env::var("KAM_UI_LANGUAGE").ok();
        let orig_kam_lang = env::var("KAM_LANG").ok();
        let orig_lang = env::var("LANG").ok();
        let orig_cwd = env::current_dir().unwrap();

        // Create a temporary project with a local .kam/config.toml that sets zh
        let tmp = mk_temp_dir("i18n_local");
        fs::create_dir_all(tmp.join(".kam")).unwrap();
        fs::write(
            tmp.join(".kam").join("config.toml"),
            r#"ui.language = "zh""#,
        )
        .unwrap();
        fs::write(tmp.join("kam.toml"), "name = \"i18n-test\"").unwrap();

        // ensure env overrides won't take precedence
        unsafe {
            env::remove_var("KAM_UI_LANGUAGE");
        }
        unsafe {
            env::remove_var("KAM_LANG");
        }

        // jump to project dir so local config is used
        env::set_current_dir(&tmp).unwrap();
        // start with a different language
        set_language(Language::En);
        init();

        // local should be preferred over global (if any)
        assert_eq!(current_language(), Language::Zh);

        // restore environment
        if let Some(h) = orig_home {
            unsafe {
                env::set_var("HOME", h);
            }
        } else {
            unsafe {
                env::remove_var("HOME");
            }
        }
        if let Some(k) = orig_kam_ui {
            unsafe {
                env::set_var("KAM_UI_LANGUAGE", k);
            }
        } else {
            unsafe {
                env::remove_var("KAM_UI_LANGUAGE");
            }
        }
        if let Some(k2) = orig_kam_lang {
            unsafe {
                env::set_var("KAM_LANG", k2);
            }
        } else {
            unsafe {
                env::remove_var("KAM_LANG");
            }
        }
        if let Some(l) = orig_lang {
            unsafe {
                env::set_var("LANG", l);
            }
        } else {
            unsafe {
                env::remove_var("LANG");
            }
        }
        let _ = env::set_current_dir(orig_cwd);

        // cleanup
        let _ = fs::remove_dir_all(tmp);

        // reset global language
        set_language(Language::En);
    }

    #[test]
    #[serial]
    fn test_init_global_config_fallback() {
        let orig_home = env::var("HOME").ok();
        let orig_kam_ui = env::var("KAM_UI_LANGUAGE").ok();
        let orig_kam_lang = env::var("KAM_LANG").ok();
        let orig_lang = env::var("LANG").ok();
        let orig_cwd = env::current_dir().unwrap();

        // Create a fake HOME with global config
        let htmp = mk_temp_dir("i18n_home");
        fs::create_dir_all(htmp.join(".kam")).unwrap();
        fs::write(
            htmp.join(".kam").join("config.toml"),
            r#"ui.language = "en""#,
        )
        .unwrap();

        // ensure env overrides won't take precedence
        unsafe {
            env::remove_var("KAM_UI_LANGUAGE");
        }
        unsafe {
            env::remove_var("KAM_LANG");
        }
        // attempt to set HOME
        unsafe {
            env::set_var("HOME", htmp.to_str().unwrap());
        }

        // Because `dirs` may cache the home directory at process startup, confirm it changed.
        // If not, skip to avoid mutating the real user's home directory.
        if dirs::home_dir().as_deref() != Some(htmp.as_path()) {
            // restore and cleanup, then skip by returning early
            if let Some(h) = orig_home {
                unsafe {
                    env::set_var("HOME", h);
                }
            } else {
                unsafe {
                    env::remove_var("HOME");
                }
            }
            if let Some(k) = orig_kam_ui {
                unsafe {
                    env::set_var("KAM_UI_LANGUAGE", k);
                }
            } else {
                unsafe {
                    env::remove_var("KAM_UI_LANGUAGE");
                }
            }
            if let Some(k2) = orig_kam_lang {
                unsafe {
                    env::set_var("KAM_LANG", k2);
                }
            } else {
                unsafe {
                    env::remove_var("KAM_LANG");
                }
            }
            if let Some(l) = orig_lang {
                unsafe {
                    env::set_var("LANG", l);
                }
            } else {
                unsafe {
                    env::remove_var("LANG");
                }
            }
            let _ = env::set_current_dir(orig_cwd);
            let _ = fs::remove_dir_all(&htmp);
            return;
        }

        // ensure we're not inside a project (so local won't override)
        env::set_current_dir(env::temp_dir()).unwrap();
        // start with a different language
        set_language(Language::Zh);
        init();

        // global should be used
        assert_eq!(current_language(), Language::En);

        // restore environment and cleanup
        if let Some(h) = orig_home {
            unsafe {
                env::set_var("HOME", h);
            }
        } else {
            unsafe {
                env::remove_var("HOME");
            }
        }
        if let Some(k) = orig_kam_ui {
            unsafe {
                env::set_var("KAM_UI_LANGUAGE", k);
            }
        } else {
            unsafe {
                env::remove_var("KAM_UI_LANGUAGE");
            }
        }
        if let Some(k2) = orig_kam_lang {
            unsafe {
                env::set_var("KAM_LANG", k2);
            }
        } else {
            unsafe {
                env::remove_var("KAM_LANG");
            }
        }
        if let Some(l) = orig_lang {
            unsafe {
                env::set_var("LANG", l);
            }
        } else {
            unsafe {
                env::remove_var("LANG");
            }
        }
        let _ = env::set_current_dir(orig_cwd);
        let _ = fs::remove_dir_all(&htmp);

        // reset global language
        set_language(Language::En);
    }

    #[test]
    #[serial]
    fn test_keyed_translations_work() {
        // Validate keyed translations (basic)
        // Ensure English mapping returns the English phrase
        set_language(Language::En);
        assert_eq!(
            tr_key("workspace.summary.title"),
            "✿ Workspace Build Summary ✿"
        );
        // English authorship: check a table header key
        assert_eq!(tr_key("table.header.module"), "Module");

        // Keys added from the external TOML file should also be visible:
        assert_eq!(tr_key("about.author"), "Author");
        assert_eq!(
            tr_key("build.packaging_artifacts"),
            "Packaging artifacts..."
        );

        // Validate Chinese mapping returns the Chinese phrase
        set_language(Language::Zh);
        assert_eq!(tr_key("workspace.summary.title"), "✿ 工作区构建摘要 ✿");
        assert_eq!(tr_key("table.header.module"), "模块");

        // Validate keys from the TOML file in Chinese
        assert_eq!(tr_key("about.author"), "作者");
        assert_eq!(tr_key("build.packaging_artifacts"), "正在打包制品...");

        // Reset
        set_language(Language::En);
    }
}
