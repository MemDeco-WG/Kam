use serde::{Deserialize, Serialize};
use std::path::Path;

/// Representation of a single package entry in `kam.lock`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LockPackage {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
}

impl LockPackage {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        LockPackage {
            name: name.into(),
            version: version.into(),
            source: None,
            checksum: None,
            dependencies: Vec::new(),
        }
    }
}

/// Top-level representation of a `kam.lock` file.
///
/// Mirrors the Cargo.lock style where packages are represented as `[[package]]` tables
/// and a top-level `version = <number>` is present.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct KamLock {
    /// Lockfile schema version (e.g. 1, 2, 3...); mirrors Cargo.lock's `version`.
    pub version: u32,

    /// Vec of package entries. This is serialized as `[[package]]` in TOML.
    #[serde(rename = "package")]
    #[serde(default)]
    pub packages: Vec<LockPackage>,
}

impl KamLock {
    pub fn new(version: u32) -> Self {
        KamLock {
            version,
            packages: Vec::new(),
        }
    }

    /// Load a `KamLock` from a path containing TOML content.
    pub fn load_from_path(path: &Path) -> crate::errors::Result<Self> {
        let s = std::fs::read_to_string(path)?;
        let kl: KamLock = toml::from_str(&s)?;
        Ok(kl)
    }

    /// Write the `KamLock` to the given path as TOML.
    pub fn write_to_path(&self, path: &Path) -> crate::errors::Result<()> {
        let s = toml::to_string(self)?;
        std::fs::write(path, s)?;
        Ok(())
    }

    /// Find a package by name.
    pub fn find_package(&self, name: &str) -> Option<&LockPackage> {
        self.packages.iter().find(|p| p.name == name)
    }
}
