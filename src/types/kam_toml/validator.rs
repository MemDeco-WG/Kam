use super::KamToml;
use crate::types::kam_toml::sections::module::SupportedArch;
use regex::Regex;
use std::sync::Arc;

/// Validation status for a single check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationStatus {
    Pass,
    Warn,
    Error,
}

/// One entry in the validation output
#[derive(Debug, Clone)]
pub struct ValidationEntry {
    /// dotted path to the checked field (e.g. "prop.id", "kam.supported_arch")
    pub path: String,
    /// status
    pub status: ValidationStatus,
    /// human message to explain the result
    pub message: Option<String>,
}

impl ValidationEntry {
    pub fn pass(path: impl Into<String>, message: impl Into<Option<String>>) -> Self {
        ValidationEntry { path: path.into(), status: ValidationStatus::Pass, message: message.into() }
    }
    pub fn warn(path: impl Into<String>, message: impl Into<Option<String>>) -> Self {
        ValidationEntry { path: path.into(), status: ValidationStatus::Warn, message: message.into() }
    }
    pub fn error(path: impl Into<String>, message: impl Into<Option<String>>) -> Self {
        ValidationEntry { path: path.into(), status: ValidationStatus::Error, message: message.into() }
    }
}

/// Overall report returned by the validator
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub entries: Vec<ValidationEntry>,
}

impl ValidationReport {
    pub fn new() -> Self { ValidationReport { entries: Vec::new() } }
    pub fn push(&mut self, e: ValidationEntry) { self.entries.push(e); }
    pub fn is_valid(&self) -> bool { !self.entries.iter().any(|e| e.status == ValidationStatus::Error) }
}

/// Validation rule trait. Each rule inspects the whole KamToml and returns 0..n entries.
pub trait Rule: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn validate(&self, kt: &KamToml) -> Vec<ValidationEntry>;
}

type BoxRule = Box<dyn Rule>;

/// Validator holds a set of rules and runs them in order.
pub struct Validator {
    rules: Vec<BoxRule>,
}

impl Validator {
    pub fn new() -> Self { Validator { rules: Vec::new() } }

    pub fn register<R: Rule>(mut self, r: R) -> Self { self.rules.push(Box::new(r)); self }

    pub fn register_box(mut self, r: BoxRule) -> Self { self.rules.push(r); self }

    /// Run all registered rules and collect a report
    pub fn run(&self, kt: &KamToml) -> ValidationReport {
        let mut report = ValidationReport::new();
        for r in &self.rules {
            let mut entries = r.validate(kt);
            report.entries.append(&mut entries);
        }
        report
    }

    /// Create a validator pre-populated with sensible default rules.
    pub fn with_default_rules() -> Self {
        Validator::new()
            .register(IdRule {})
            .register(VersionRule {})
            .register(SupportedArchRule::default())
            .register(ModuleTypeRule {})
    }
}

// ------------------ Built-in rules ------------------

/// Checks `prop.id` is valid using the same regex rules as KamToml::validate_id
struct IdRule {}
impl Rule for IdRule {
    fn name(&self) -> &str { "id" }
    fn validate(&self, kt: &KamToml) -> Vec<ValidationEntry> {
        let mut out = Vec::new();
        let id = &kt.prop.id;
        // reuse KamToml::validate_id logic by re-running the same regex
        let re = Regex::new(r"^[a-zA-Z][a-zA-Z0-9._-]+$").unwrap();
        if id.is_empty() {
            out.push(ValidationEntry::error("prop.id", Some("id must not be empty".to_string())));
        } else if !re.is_match(id) {
            out.push(ValidationEntry::error("prop.id", Some(format!("invalid id '{}'", id))));
        } else {
            out.push(ValidationEntry::pass("prop.id", Some("ok".to_string())));
        }
        out
    }
}

/// Checks `prop.version` has simple semver-like format x.y.z
struct VersionRule {}
impl Rule for VersionRule {
    fn name(&self) -> &str { "version" }
    fn validate(&self, kt: &KamToml) -> Vec<ValidationEntry> {
        let mut out = Vec::new();
        let v = &kt.prop.version;
        let version_re = Regex::new(r"^\d+\.\d+\.\d+$").unwrap();
        if !version_re.is_match(v) {
            out.push(ValidationEntry::error("prop.version", Some("version must be in format x.y.z".to_string())));
        } else {
            out.push(ValidationEntry::pass("prop.version", Some("ok".to_string())));
        }
        out
    }
}

/// Validate supported_arch entries by comparing their canonical string forms
#[derive(Clone)]
pub struct SupportedArchRule {
    pub valid_archs: Arc<Vec<String>>,
}

impl Default for SupportedArchRule {
    fn default() -> Self {
        SupportedArchRule { valid_archs: Arc::new(vec!["arm64-v8a".to_string(), "armeabi-v7a".to_string(), "x86".to_string(), "x86_64".to_string()]) }
    }
}

impl Rule for SupportedArchRule {
    fn name(&self) -> &str { "supported_arch" }
    fn validate(&self, kt: &KamToml) -> Vec<ValidationEntry> {
        let mut out = Vec::new();
        if let Some(archs) = &kt.kam.supported_arch {
            for arch in archs {
                let s = arch.to_string();
                if !self.valid_archs.contains(&s) {
                    out.push(ValidationEntry::error("kam.supported_arch", Some(format!("unsupported arch '{}', allowed: {:?}", s, self.valid_archs))));
                } else {
                    out.push(ValidationEntry::pass("kam.supported_arch", Some(format!("{} ok", s))));
                }
            }
        } else {
            out.push(ValidationEntry::warn("kam.supported_arch", Some("no supported_arch specified".to_string())));
        }
        out
    }
}

/// Ensure template/library sections exist when module_type requires them
struct ModuleTypeRule {}
impl Rule for ModuleTypeRule {
    fn name(&self) -> &str { "module_type" }
    fn validate(&self, kt: &KamToml) -> Vec<ValidationEntry> {
        let mut out = Vec::new();
        use crate::types::kam_toml::sections::module::ModuleType;
        match kt.kam.module_type {
            ModuleType::Template => {
                if kt.kam.tmpl.is_none() {
                    out.push(ValidationEntry::error("kam.template", Some("module_type is Template but tmpl section is missing".to_string())));
                } else { out.push(ValidationEntry::pass("kam.template", Some("ok".to_string()))); }
            }
            ModuleType::Library => {
                if kt.kam.lib.is_none() {
                    out.push(ValidationEntry::error("kam.lib", Some("module_type is Library but lib section is missing".to_string())));
                } else { out.push(ValidationEntry::pass("kam.lib", Some("ok".to_string()))); }
            }
            ModuleType::Kam | ModuleType::Repo => {
                out.push(ValidationEntry::pass("kam.module_type", Some("Kam/Repo".to_string())));
            }
        }
        out
    }
}

// Export common types
pub use ValidationReport;
pub use ValidationEntry;
pub use ValidationStatus;
pub use Validator;
pub use Rule;
